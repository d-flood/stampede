use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::{
    io::Join,
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time::{Instant, interval, sleep, timeout},
};
use tokio_tungstenite::{
    WebSocketStream, client_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

use super::{
    AgentCommand, Answer, Builtin, CLOSING, Capabilities, Chat, Choice, CommandStatus, Context, Driver, Event, Item, Notice, Outcome, Phase, Setting,
    Settings, Session, Subagent, Todo, TodoStatus, Tokens, ToolCall, ToolKind, Image, Question, Scope, diff, relative, seconds, unified, words,
    html,
};
use crate::{
    config::{Agent, AgentSettings, codex_policy},
    chats::Status,
    usage::{Reading, ResetCredits, ResetOutcome, Snapshot, Window},
};

const HEARTBEAT: Duration = Duration::from_secs(5);
const RECONNECTING: Duration = Duration::from_secs(30);
const REPLAY: Duration = Duration::from_secs(5);

const SOURCES: [&str; 5] = ["cli", "vscode", "exec", "appServer", "unknown"];

pub struct Codex {
    requests: UnboundedSender<Request>,
}

enum Request {
    Send(String, Vec<Image>),
    Retry,
    Interrupt,
    Answer(String, Answer),
    Run(Builtin),
    Close,
    Delegated(Item),
    Shell(String),
}

impl Driver for Codex {
    fn send(&self, text: &str, images: &[Image]) {
        let _ = self.requests.send(Request::Send(text.to_owned(), images.to_vec()));
    }

    fn retry(&self) {
        let _ = self.requests.send(Request::Retry);
    }

    fn interrupt(&self) {
        let _ = self.requests.send(Request::Interrupt);
    }

    fn answer(&self, request_id: &str, answer: Answer) {
        let _ = self.requests.send(Request::Answer(request_id.to_owned(), answer));
    }

    fn run(&self, builtin: Builtin) {
        let _ = self.requests.send(Request::Run(builtin));
    }

    fn delegated(&self, item: Item) {
        let _ = self.requests.send(Request::Delegated(item));
    }

    fn close(&self) {
        let _ = self.requests.send(Request::Close);
    }

    fn shell(&self, command: &str) {
        let _ = self.requests.send(Request::Shell(command.to_owned()));
    }
}

pub async fn start(
    data_dir: &Path,
    chat_id: &str,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    settings: &AgentSettings,
) -> io::Result<(Codex, UnboundedReceiver<Event>)> {
    let (mut connection, proxy) = connect(cwd, env).await?;
    let started = connection.call("thread/start", thread(data_dir, chat_id, cwd, settings, env)).await?;
    let thread = started["thread"]["id"].as_str().ok_or_else(|| io::Error::other("thread/start gave no thread"))?;
    let (events, received) = mpsc::unbounded_channel();
    let _ = events.send(Event::Session(thread.to_owned()));
    let mut transcript = Transcript::new(thread, cwd, events);
    transcript.data_dir = Some(data_dir.to_owned());
    transcript.chat_id = chat_id.to_owned();
    transcript.settings = current(&started);
    Ok((run(connection, proxy, transcript, data_dir, cwd, env), received))
}

pub async fn resume(data_dir: &Path, chat: &Chat, env: &BTreeMap<String, String>) -> io::Result<(Codex, UnboundedReceiver<Event>)> {
    let thread = chat.agent_session.as_deref().ok_or_else(|| io::Error::other("the chat has no thread"))?;
    let (events, received) = mpsc::unbounded_channel();
    let mut transcript = Transcript::new(thread, &chat.cwd, events);
    transcript.data_dir = Some(data_dir.to_owned());
    transcript.chat_id = chat.id.clone();
    transcript.status = chat.status;
    transcript.settings = chat.settings.clone();
    transcript.plan_resolved = chat.plan_resolved.clone();
    let (connection, proxy) = rejoin(data_dir, &chat.cwd, env, &mut transcript).await?;
    Ok((run(connection, proxy, transcript, data_dir, &chat.cwd, env), received))
}

async fn rejoin(
    data_dir: &Path,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    transcript: &mut Transcript,
) -> io::Result<(Connection, Child)> {
    let (mut connection, proxy) = connect(cwd, env).await?;
    let params = json!({ "threadId": transcript.thread, "cwd": cwd, "excludeTurns": true, "config": tool_config(data_dir, &transcript.chat_id, env) });
    let id = connection.request("thread/resume", params).await?;
    let mut replay = Vec::new();
    let resumed = loop {
        let mut message = connection.next().await.ok_or_else(|| io::Error::other("codex hung up during thread/resume"))?;
        if response(&message) == Some(id) {
            if let Some(error) = message.get("error") {
                return Err(io::Error::other(format!("thread/resume: {error}")));
            }
            break message["result"].take();
        }
        if message.get("method").is_some() && message.get("id").is_some() {
            replay.push(message);
        }
    };
    let mut held = Vec::new();
    let turns = connection.history(&transcript.thread, &mut held).await?;
    let mut done = HashSet::new();
    finished(&turns, &mut done);
    let pending = transcript.settings.clone();
    transcript.settings = current(&resumed);
    for &setting in &pending.pending {
        *transcript.settings.get_mut(setting) = pending.get(setting).clone();
        transcript.settings.mark_pending(setting);
    }
    transcript.report_settings();
    let status = transcript.resumed(&turns);
    transcript.status = status;
    for child in transcript.subagents.keys().cloned().collect::<Vec<_>>() {
        let read = connection.hold("thread/read", json!({ "threadId": child }), &mut held).await.unwrap_or_default();
        let turns = connection.history(&child, &mut held).await.unwrap_or_default();
        finished(&turns, &mut done);
        transcript.rejoined(&child, &read["thread"], &turns);
    }
    replay.extend(held.into_iter().filter(|message| !stale(message, &done)));
    for message in replay {
        transcript.read(&message);
    }
    if resumed["thread"]["status"]["activeFlags"].as_array().is_some_and(|flags| !flags.is_empty())
        && transcript.asked.is_empty()
    {
        let _ = timeout(REPLAY, async {
            while let Some(message) = connection.next().await {
                transcript.read(&message);
                if !transcript.asked.is_empty() || transcript.turn.is_none() {
                    break;
                }
            }
        })
        .await;
    }
    let _ = timeout(REPLAY, connection.shells(transcript)).await;
    transcript.report(transcript.status);
    if transcript.turn.is_none() { transcript.propose(); }
    Ok((connection, proxy))
}

/// Adds the ids of the turns and items `turns` holds finished.
fn finished(turns: &[Value], done: &mut HashSet<String>) {
    for turn in turns {
        for value in std::iter::once(turn).chain(turn["items"].as_array().into_iter().flatten()) {
            if value["status"] != "inProgress" && let Some(id) = value["id"].as_str() {
                done.insert(id.to_owned());
            }
        }
    }
}

/// Whether `message`, which arrived while history was paged in, notifies of something that history already holds finished.
fn stale(message: &Value, done: &HashSet<String>) -> bool {
    let params = &message["params"];
    let ids = [&params["item"]["id"], &params["itemId"], &params["turn"]["id"]];
    message.get("id").is_none() && ids.into_iter().any(|id| id.as_str().is_some_and(|id| done.contains(id)))
}

async fn reconnect(
    data_dir: &Path,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    transcript: &mut Transcript,
) -> Option<(Connection, Child)> {
    let deadline = Instant::now() + RECONNECTING;
    loop {
        match rejoin(data_dir, cwd, env, transcript).await {
            Ok(rejoined) => return Some(rejoined),
            Err(_) if Instant::now() < deadline => sleep(Duration::from_secs(1)).await,
            Err(error) => {
                eprintln!("cannot reconnect to codex thread {}: {error}", transcript.thread);
                return None;
            }
        }
    }
}

/// This cwd's threads, non-interactive ones included.
pub async fn sessions(cwd: &Path, env: &BTreeMap<String, String>) -> io::Result<Vec<Session>> {
    let (mut connection, mut proxy) = connect(cwd, env).await?;
    let mut sessions = Vec::new();
    let mut cursor = Value::Null;
    loop {
        let params = json!({ "cwd": cwd, "sourceKinds": SOURCES, "sortKey": "updated_at", "cursor": cursor });
        let listed = connection.call("thread/list", params).await?;
        for thread in listed["data"].as_array().into_iter().flatten() {
            let title = thread["name"].as_str().filter(|title| !title.trim().is_empty())
                .or_else(|| thread["preview"].as_str().filter(|preview| !preview.trim().is_empty()));
            if let (Some(id), Some(title), Some(updated)) =
                (thread["id"].as_str(), title, thread["updatedAt"].as_u64())
            {
                sessions.push(Session::new(Agent::Codex, id, title, updated * 1000));
            }
        }
        cursor = listed["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    let _ = proxy.kill().await;
    Ok(sessions)
}

pub async fn options(cwd: &Path, env: &BTreeMap<String, String>, settings: &AgentSettings) -> io::Result<(Vec<Value>, AgentSettings)> {
    let (mut connection, mut proxy) = connect(cwd, env).await?;
    let result = async {
        let mut models = Vec::new();
        let mut cursor = Value::Null;
        loop {
            let listed = connection.call("model/list", json!({ "cursor": cursor })).await?;
            models.extend(listed["data"].as_array().into_iter().flatten().cloned());
            cursor = listed["nextCursor"].clone();
            if cursor.is_null() { break; }
        }
        let config = connection.call("config/read", json!({ "includeLayers": false, "cwd": cwd })).await?;
        let default = models.iter().find(|model| model["isDefault"] == true);
        let model = settings.model.as_deref().or(config["config"]["model"].as_str())
            .or_else(|| default.and_then(|model| model["model"].as_str()));
        let listed = models.iter().find(|listed| listed["model"].as_str() == model);
        let effort = settings.effort.as_deref().or(config["config"]["model_reasoning_effort"].as_str())
            .or_else(|| listed.and_then(|model| model["defaultReasoningEffort"].as_str()));
        let settings = AgentSettings { model: model.map(str::to_owned), effort: effort.map(str::to_owned), mode: settings.mode.clone() };
        Ok((models, settings))
    }.await;
    let _ = proxy.kill().await;
    result
}

async fn connect(cwd: &Path, env: &BTreeMap<String, String>) -> io::Result<(Connection, Child)> {
    let mut daemon = codex(cwd, env);
    if env.contains_key("STAMPEDE_SCRATCH") {
        for name in crate::storage::ENVIRONMENT { daemon.env_remove(name); }
    }
    let daemon = daemon.args(["app-server", "daemon", "start"]).stdout(Stdio::null()).status().await?;
    if !daemon.success() {
        return Err(io::Error::other(format!("codex app-server daemon start: {daemon}")));
    }
    let mut proxy = codex(cwd, env)
        .args(["app-server", "proxy"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    // The proxy relays raw bytes to the daemon's control socket, which speaks WebSocket.
    let stdio = tokio::io::join(proxy.stdout.take().unwrap(), proxy.stdin.take().unwrap());
    // A page of history holding generated images can exceed tungstenite's default frame and message limits.
    let config = WebSocketConfig::default().max_frame_size(None).max_message_size(None);
    let (socket, _) = client_async_with_config("ws://localhost/", stdio, Some(config)).await.map_err(io::Error::other)?;
    let mut connection = Connection { socket, next: 0 };
    let client = json!({ "name": "stampede", "title": "Stampede", "version": env!("CARGO_PKG_VERSION") });
    let initialize = json!({ "clientInfo": client, "capabilities": { "experimentalApi": true } });
    connection.call("initialize", initialize).await?;
    connection.write(json!({ "method": "initialized" })).await?;
    Ok((connection, proxy))
}

fn run(
    mut connection: Connection,
    mut proxy: Child,
    mut transcript: Transcript,
    data_dir: &Path,
    cwd: &Path,
    env: &BTreeMap<String, String>,
) -> Codex {
    let (requests, mut pending) = mpsc::unbounded_channel();
    let (data_dir, cwd, env) = (data_dir.to_owned(), cwd.to_owned(), env.clone());
    transcript.storage_roots = writable_roots(&data_dir, &env);
    tokio::spawn(async move {
        // The proxy outlives a daemon that has gone until it next relays something.
        let mut heartbeat = interval(HEARTBEAT);
        transcript.report_settings();
        transcript.declare();
        let lists = [("model/list", json!({}), List::Models), ("skills/list", json!({ "cwds": [cwd] }), List::Skills)];
        for (method, params, list) in lists {
            if let Ok(id) = connection.request(method, params).await {
                transcript.lists.insert(id, list);
            }
        }
        loop {
            tokio::select! {
                request = pending.recv() => match request {
                    Some(Request::Send(text, images)) => {
                        let mut input = vec![json!({ "type": "text", "text": text, "text_elements": [] })];
                        input.extend(images.iter().map(|image| json!({ "type": "image", "url": format!("data:{};base64,{}", image.media_type, image.data) })));
                        input.extend(transcript.skill(&text));
                        let _ = connection.send(&mut transcript, Value::Array(input)).await;
                    }
                    Some(Request::Delegated(item)) => transcript.delegated(item),
                    Some(Request::Shell(command)) => {
                        let _ = connection.request("thread/shellCommand", json!({ "threadId": transcript.thread, "command": command })).await;
                    }
                    Some(Request::Retry) => {
                        if let Some(input) = transcript.last_input.clone() {
                            let _ = connection.send(&mut transcript, input).await;
                        }
                    }
                    Some(Request::Interrupt) => {
                        let _ = connection.interrupt(&mut transcript).await;
                    }
                    Some(Request::Answer(id, answer)) => {
                        if transcript.plan.as_ref().is_some_and(|(plan, _)| plan == &id) {
                            transcript.plan = None;
                            transcript.plan_resolved = Some(id.clone());
                            let _ = transcript.events.send(Event::PlanResolved(id.clone()));
                            let text = match answer {
                                Answer::Allow => {
                                    transcript.choose(Setting::Mode, "default".to_owned());
                                    Some("Implement the plan.".to_owned())
                                }
                                Answer::Deny { instead } => instead,
                                _ => None,
                            };
                            if let Some(text) = text {
                                let input = json!([{ "type": "text", "text": text, "text_elements": [] }]);
                                let _ = connection.send(&mut transcript, input).await;
                            }
                            continue;
                        }
                        let Some((rpc, grant)) = transcript.resolved(&id) else { continue };
                        let questions = transcript.questions.remove(&id);
                        let question = questions.is_some();
                        let result = if let Some(questions) = questions {
                            let answers = match &answer {
                                Answer::Answers(answers) => answers.clone(),
                                Answer::Deny { instead: Some(text) } => vec![vec![text.clone()]; questions.len()],
                                _ => vec![Vec::new(); questions.len()],
                            };
                            let mapped = questions.iter().zip(answers).map(|(q, answers)| (q.id.clone(), json!({ "answers": answers }))).collect::<serde_json::Map<_, _>>();
                            json!({ "answers": mapped })
                        } else { match (&answer, grant) {
                            (Answer::Allow, Some(permissions)) => json!({ "permissions": permissions, "scope": "turn" }),
                            (Answer::Deny { .. }, Some(_)) => json!({ "permissions": {}, "scope": "turn" }),
                            (Answer::Allow, None) => json!({ "decision": "accept" }),
                            (Answer::Rules(rules), None) if rules.iter().all(|rule| rule.scope == Scope::Chat) => json!({ "decision": "acceptForSession" }),
                            (Answer::Rules(rules), None) => {
                                match persist_extra_rules(&env, rules) {
                                    Ok(()) => {
                                        json!({ "decision": { "acceptWithExecpolicyAmendment": { "execpolicy_amendment": rules[0].prefix } } })
                                    }
                                    Err(error) => {
                                        transcript.push(Item::Notice { notice: Notice::Error, text: format!("Could not save approval rules: {error}") });
                                        json!({ "decision": "decline" })
                                    }
                                }
                            },
                            (Answer::Deny { instead: None }, None) => json!({ "decision": "cancel" }),
                            _ => json!({ "decision": "decline" }),
                        }};
                        if connection.write(json!({ "id": rpc, "result": result })).await.is_err() {
                            continue;
                        }
                        if question { continue; }
                        let Answer::Deny { instead: Some(text) } = answer else { continue };
                        let input = json!([{ "type": "text", "text": text, "text_elements": [] }]);
                        let _ = connection.send(&mut transcript, input).await;
                    }
                    Some(Request::Run(Builtin::Rewind(index))) if transcript.idle() => {
                        if let Some(turn) = transcript.checkpoints.get(&index).cloned() {
                            let boundary = transcript.checkpoints.iter().filter(|(_, id)| **id == turn).map(|(index, _)| *index).min().unwrap_or(index);
                            let params = json!({ "threadId": transcript.thread, "beforeTurnId": turn });
                            transcript.report(Status::Working);
                            match connection.call("thread/revert", params).await {
                                Ok(_) => {
                                    let turns = connection.history(&transcript.thread, &mut Vec::new()).await;
                                    let _ = transcript.events.send(Event::QuestionsTruncated(boundary));
                                    transcript.status = Status::Done;
                                    transcript.resumed(turns.as_deref().unwrap_or_default());
                                    let _ = transcript.events.send(Event::Reset);
                                    if let Err(error) = turns {
                                        transcript.push(Item::Notice { notice: Notice::Error, text: error.to_string() });
                                    }
                                    transcript.push(Item::Notice { notice: Notice::Rewound, text: "Selected turn and every later turn removed. Files are left unchanged.".to_owned() });
                                }
                                Err(error) => { transcript.push(Item::Notice { notice: Notice::Error, text: error.to_string() }); }
                            }
                            transcript.report(Status::Done);
                        } else {
                            transcript.push(Item::Notice { notice: Notice::Error, text: "This message has no rewind checkpoint. Choose another user message.".to_owned() });
                        }
                    }
                    Some(Request::Run(Builtin::Compact)) if transcript.idle() => {
                        transcript.report(Status::Working);
                        let compact = json!({ "threadId": transcript.thread });
                        match connection.request("thread/compact/start", compact).await {
                            Ok(id) => transcript.compacting = Some(id),
                            Err(_) => break,
                        }
                    }
                    Some(Request::Run(Builtin::Clear)) if transcript.idle() => {
                        match connection.call("thread/start", thread(&data_dir, &transcript.chat_id, &cwd, &AgentSettings::default(), &env)).await {
                            Ok(started) => {
                                let previous = json!({ "threadId": transcript.thread });
                                let _ = connection.call("thread/unsubscribe", previous).await;
                                transcript.restart(&started);
                            }
                            Err(error) => eprintln!("codex /clear: {error}"),
                        }
                    }
                    Some(Request::Run(Builtin::Model(model))) => transcript.choose(Setting::Model, model),
                    Some(Request::Run(Builtin::Effort(effort))) => transcript.choose(Setting::Effort, effort),
                    Some(Request::Run(Builtin::Mode(mode))) => transcript.choose(Setting::Mode, mode),
                    Some(Request::Run(_)) => {}
                    Some(Request::Close) | None => {
                        let _ = timeout(CLOSING, connection.close(&mut transcript)).await;
                        break;
                    }
                },
                message = connection.next() => match message {
                    Some(message) => {
                        transcript.read(&message);
                    }
                    None => {
                        let _ = proxy.kill().await;
                        match reconnect(&data_dir, &cwd, &env, &mut transcript).await {
                            Some(rejoined) => (connection, proxy) = rejoined,
                            None => return,
                        }
                    }
                },
                _ = heartbeat.tick() => {
                    let _ = connection.ping().await;
                    let _ = timeout(HEARTBEAT, connection.shells(&mut transcript)).await;
                }
            }
            if let Some(input) = transcript.unsteered.take() {
                let _ = connection.start(&mut transcript, input).await;
            }
        }
        let _ = proxy.kill().await;
    });
    Codex { requests }
}

fn persist_extra_rules(env: &BTreeMap<String, String>, rules: &[super::Rule]) -> io::Result<()> {
    if rules.len() < 2 { return Ok(()); }
    let home = env.get("CODEX_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("CODEX_HOME").map(PathBuf::from))
        .unwrap_or_else(|| crate::home().join(".codex"));
    let directory = home.join("rules");
    fs::create_dir_all(&directory)?;
    let content = rules.iter().skip(1).map(|rule| {
        let pattern = serde_json::to_string(&rule.prefix).unwrap();
        format!("prefix_rule(pattern={pattern}, decision=\"allow\")\n")
    }).collect::<String>();
    fs::write(directory.join(format!("stampede-{}.rules", uuid::Uuid::new_v4())), content)
}

/// Reads the account's windows from the daemon, which answers without a thread.
pub async fn usage(env: &BTreeMap<String, String>) -> io::Result<Snapshot> {
    let (mut connection, mut proxy) = connect(&crate::home(), env).await?;
    let read = connection.call("account/rateLimits/read", json!({ "excludeResetCreditDetails": false })).await;
    let _ = proxy.kill().await;
    Ok(usage_snapshot(&read?))
}

pub async fn consume_reset(env: &BTreeMap<String, String>, key: &str, credit: Option<&str>) -> io::Result<(ResetOutcome, Snapshot)> {
    let (mut connection, mut proxy) = connect(&crate::home(), env).await?;
    let result = async {
        let redeemed = connection.call("account/rateLimitResetCredit/consume", json!({ "idempotencyKey": key, "creditId": credit })).await?;
        let outcome = serde_json::from_value(redeemed["outcome"].clone()).map_err(io::Error::other)?;
        let read = connection.call("account/rateLimits/read", json!({ "excludeResetCreditDetails": false })).await?;
        Ok((outcome, usage_snapshot(&read)))
    }.await;
    let _ = proxy.kill().await;
    result
}

fn usage_snapshot(read: &Value) -> Snapshot {
    let credits = &read["rateLimitResetCredits"];
    let next = credits["credits"].as_array().into_iter().flatten()
        .filter(|credit| credit["status"] == "available" && credit["resetType"] == "codexRateLimits")
        .min_by_key(|credit| credit["expiresAt"].as_u64().unwrap_or(u64::MAX));
    let codex_resets = credits["availableCount"].as_u64().map(|available_count| ResetCredits {
        available_count,
        expires_at: next.and_then(|credit| credit["expiresAt"].as_u64()).map(|seconds| seconds.saturating_mul(1000)),
        credit_id: next.and_then(|credit| credit["id"].as_str()).map(str::to_owned),
    });
    let snapshot = read["rateLimitsByLimitId"].get("codex").unwrap_or(&read["rateLimits"]);
    Snapshot { readings: readings(snapshot), codex_resets }
}

/// The windows of a rate-limit snapshot, which may come from another metered limit than Codex's own.
fn readings(snapshot: &Value) -> Vec<Reading> {
    if !(snapshot["limitId"].is_null() || snapshot["limitId"] == "codex") {
        return Vec::new();
    }
    let windows = [&snapshot["primary"], &snapshot["secondary"]].into_iter().filter_map(|window| {
        let kind = match window["windowDurationMins"].as_u64()? {
            300 => Window::FiveHour,
            10_080 => Window::Weekly,
            _ => return None,
        };
        let used = window["usedPercent"].as_f64()? / 100.0;
        Some(Reading::new(Agent::Codex, kind, used, window["resetsAt"].as_u64().map(|seconds| seconds * 1000)))
    });
    windows.collect()
}

fn thread(data_dir: &Path, chat_id: &str, cwd: &Path, settings: &AgentSettings, env: &BTreeMap<String, String>) -> Value {
    let mut params = json!({ "cwd": cwd, "config": tool_config(data_dir, chat_id, env) });
    if let Some(model) = &settings.model {
        params["model"] = json!(model);
    }
    if let Some(effort) = &settings.effort {
        params["config"]["model_reasoning_effort"] = json!(effort);
    }
    if let Some((approval, sandbox, reviewer)) = settings.mode.as_deref().and_then(codex_policy) {
        params["approvalPolicy"] = json!(approval);
        params["sandbox"] = json!(sandbox);
        params["approvalsReviewer"] = json!(reviewer);
    }
    params
}

fn tool_config(data_dir: &Path, chat_id: &str, env: &BTreeMap<String, String>) -> Value {
    let mut config = html::config(data_dir, chat_id);
    config["tool_timeout_sec"] = json!(86400);
    let environment: BTreeMap<_, _> = crate::storage::ENVIRONMENT.into_iter().filter_map(|name| env.get(name).map(|value| (name, value))).collect();
    let mut config = json!({ "tools.update_plan": { "enabled": true }, "mcp_servers.stampede_chat": config });
    if !environment.is_empty() {
        config["shell_environment_policy.set"] = json!(environment);
        config["sandbox_workspace_write.writable_roots"] = json!(writable_roots(data_dir, env));
    }
    config
}

fn writable_roots(data_dir: &Path, env: &BTreeMap<String, String>) -> Vec<String> {
    ["STAMPEDE_SCRATCH", "STAMPEDE_CACHE", "CARGO_TARGET_DIR"].into_iter()
        .filter_map(|name| env.get(name)).filter(|path| Path::new(path).is_absolute()).cloned()
        .chain([crate::canvas::root(data_dir).display().to_string()]).collect()
}

fn codex(cwd: &Path, env: &BTreeMap<String, String>) -> Command {
    let mut command = Command::new("codex");
    command.current_dir(cwd).envs(env).stdin(Stdio::null()).stderr(Stdio::null());
    command
}

struct Connection {
    socket: WebSocketStream<Join<ChildStdout, ChildStdin>>,
    next: u64,
}

impl Connection {
    async fn shells(&mut self, transcript: &mut Transcript) -> io::Result<()> {
        let mut threads = Vec::new();
        transcript.shell_threads(&mut threads);
        for (thread, candidates) in threads {
            let mut live = HashSet::new();
            let mut cursor = Value::Null;
            loop {
                let id = self.request("thread/backgroundTerminals/list", json!({ "threadId": thread, "cursor": cursor })).await?;
                let listed = loop {
                    let message = self.next().await.ok_or(io::ErrorKind::UnexpectedEof)?;
                    if response(&message) == Some(id) {
                        if let Some(error) = message.get("error") { return Err(io::Error::other(error.to_string())); }
                        break message["result"].clone();
                    }
                    transcript.read(&message);
                };
                let data = listed["data"].as_array().ok_or_else(|| io::Error::other("background terminals list gave no data"))?;
                live.extend(data.iter().filter_map(|item| item["itemId"].as_str().map(str::to_owned)));
                cursor = listed["nextCursor"].clone();
                if cursor.is_null() { break; }
            }
            transcript.reconcile_shells(&thread, &candidates, &live);
        }
        Ok(())
    }

    async fn write(&mut self, message: Value) -> io::Result<()> {
        self.socket.send(Message::text(message.to_string())).await.map_err(io::Error::other)
    }

    async fn ping(&mut self) -> io::Result<()> {
        self.socket.send(Message::Ping(Default::default())).await.map_err(io::Error::other)
    }

    async fn request(&mut self, method: &str, params: Value) -> io::Result<u64> {
        let id = self.next;
        self.next += 1;
        self.write(json!({ "method": method, "id": id, "params": params })).await?;
        Ok(id)
    }

    async fn call(&mut self, method: &str, params: Value) -> io::Result<Value> {
        self.hold(method, params, &mut Vec::new()).await
    }

    /// Like `call`, keeping whatever arrives meanwhile in `held`.
    async fn hold(&mut self, method: &str, params: Value, held: &mut Vec<Value>) -> io::Result<Value> {
        let id = self.request(method, params).await?;
        loop {
            let mut message = self.next().await.ok_or_else(|| io::Error::other(format!("codex hung up during {method}")))?;
            if response(&message) == Some(id) {
                return match message.get("error") {
                    Some(error) => Err(io::Error::other(format!("{method}: {error}"))),
                    None => Ok(message["result"].take()),
                };
            }
            held.push(message);
        }
    }

    async fn pages(&mut self, method: &str, mut params: Value, held: &mut Vec<Value>) -> io::Result<Vec<Value>> {
        let mut data = Vec::new();
        loop {
            let mut page = self.hold(method, params.clone(), held).await?;
            if let Value::Array(entries) = page["data"].take() { data.extend(entries); }
            if page["nextCursor"].is_null() { return Ok(data); }
            params["cursor"] = page["nextCursor"].take();
        }
    }

    /// A thread's turns with their items, paged in item by item since a long thread's whole history is too large for one reply.
    async fn history(&mut self, thread: &str, held: &mut Vec<Value>) -> io::Result<Vec<Value>> {
        let mut items: HashMap<String, Vec<Value>> = HashMap::new();
        for mut entry in self.pages("thread/items/list", json!({ "threadId": thread, "sortDirection": "asc" }), held).await? {
            if let Some(turn) = entry["turnId"].as_str() {
                items.entry(turn.to_owned()).or_default().push(entry["item"].take());
            }
        }
        // Listed after the items, so that every item's turn is among them.
        let params = json!({ "threadId": thread, "sortDirection": "asc", "itemsView": "notLoaded" });
        let mut turns = self.pages("thread/turns/list", params, held).await?;
        for turn in &mut turns {
            turn["items"] = Value::Array(turn["id"].as_str().and_then(|id| items.remove(id)).unwrap_or_default());
        }
        Ok(turns)
    }

    async fn next(&mut self) -> Option<Value> {
        while let Some(Ok(message)) = self.socket.next().await {
            if let Message::Text(text) = message
                && let Ok(message) = serde_json::from_str(&text)
            {
                return Some(message);
            }
        }
        None
    }

    async fn started(&mut self, transcript: &mut Transcript) -> io::Result<()> {
        while transcript.starting.is_some() {
            let message = self.next().await.ok_or(io::ErrorKind::UnexpectedEof)?;
            transcript.read(&message);
        }
        Ok(())
    }

    async fn start(&mut self, transcript: &mut Transcript, input: Value) -> io::Result<()> {
        transcript.report(Status::Working);
        let mut turn = json!({ "threadId": transcript.thread, "input": input });
        transcript.overrides(&mut turn);
        let id = self.request("turn/start", turn).await?;
        transcript.starting = Some(id);
        Ok(())
    }

    async fn send(&mut self, transcript: &mut Transcript, input: Value) -> io::Result<()> {
        self.started(transcript).await?;
        let Some(turn) = &transcript.turn else { return self.start(transcript, input).await };
        let steer = json!({ "threadId": transcript.thread, "input": input, "expectedTurnId": turn });
        let id = self.request("turn/steer", steer).await?;
        transcript.steering.insert(id, input);
        Ok(())
    }

    async fn interrupt(&mut self, transcript: &mut Transcript) -> io::Result<()> {
        self.started(transcript).await?;
        if let Some(turn) = &transcript.turn {
            self.request("turn/interrupt", json!({ "threadId": transcript.thread, "turnId": turn })).await?;
        }
        Ok(())
    }

    async fn close(&mut self, transcript: &mut Transcript) {
        if self.started(transcript).await.is_err() {
            return;
        }
        if let Some(turn) = transcript.turn.take() {
            let _ = self.call("turn/interrupt", json!({ "threadId": transcript.thread, "turnId": turn })).await;
        }
        let _ = self.call("thread/unsubscribe", json!({ "threadId": transcript.thread })).await;
    }
}

fn response(message: &Value) -> Option<u64> {
    match message.get("method") {
        Some(_) => None,
        None => message["id"].as_u64(),
    }
}

struct Transcript {
    data_dir: Option<PathBuf>,
    last_input: Option<Value>,
    thread: String,
    chat_id: String,
    cwd: PathBuf,
    storage_roots: Vec<String>,
    events: UnboundedSender<Event>,
    len: usize,
    checkpoints: HashMap<usize, String>,
    starting: Option<u64>,
    turn: Option<String>,
    streaming: HashMap<String, (usize, String, Phase)>,
    thinking: HashMap<String, (usize, BTreeMap<u64, String>)>,
    tools: Option<(usize, Vec<ToolCall>)>,
    activity: super::Activity,
    commands: HashMap<String, (usize, Item, Option<String>)>,
    subagents: HashMap<String, (Subagent, Transcript)>,
    delegated: HashMap<String, usize>,
    previous: u64,
    steering: HashMap<u64, Value>,
    /// A steer that reached the daemon after its turn had ended, to start a turn with instead.
    unsteered: Option<Value>,
    status: Status,
    asked: HashMap<String, (Value, Option<Value>, Option<String>)>,
    edits: HashMap<String, Value>,
    settings: Settings,
    models: Vec<Value>,
    skills: Vec<Value>,
    compacting: Option<u64>,
    applying: Vec<(Setting, Option<String>)>,
    lists: HashMap<u64, List>,
    questions: HashMap<String, Vec<Question>>,
    plan: Option<(String, String)>,
    plan_resolved: Option<String>,
}

enum List {
    Models,
    Skills,
}

fn enabled(skills: &Value) -> Vec<Value> {
    let listed = skills["data"].as_array().into_iter().flatten();
    let skills = listed.flat_map(|listed| listed["skills"].as_array()).flatten();
    skills.filter(|skill| skill["enabled"] != false).cloned().collect()
}

fn current(thread: &Value) -> Settings {
    let policy = |mode: &&str| {
        let (approval, sandbox, reviewer) = codex_policy(mode).unwrap();
        thread["approvalPolicy"] == approval && thread["sandbox"]["type"] == policy_type(sandbox)
            && thread["approvalsReviewer"].as_str().unwrap_or("user") == reviewer
    };
    let mode = match thread["collaborationMode"]["mode"] == "plan" {
        true => Some("plan"),
        false => Agent::Codex.modes().iter().copied().find(policy),
    };
    let chosen = AgentSettings {
        model: thread["model"].as_str().map(str::to_owned),
        effort: thread["reasoningEffort"].as_str().map(str::to_owned),
        mode: mode.map(str::to_owned),
    };
    Settings { chosen, pending: Vec::new() }
}

/// The type of a sandbox policy, as `turn/start` takes it, from the sandbox mode its `thread/start` takes.
fn policy_type(sandbox: &str) -> String {
    let mut words = sandbox.split('-');
    let first = words.next().unwrap_or_default().to_owned();
    words.fold(first, |camel, word| camel + &word[..1].to_uppercase() + &word[1..])
}

impl Transcript {
    fn shell_threads(&self, threads: &mut Vec<(String, HashSet<String>)>) {
        let candidates: HashSet<_> = self.commands.iter()
            .filter(|(_, (_, _, turn))| self.turn.is_none() || *turn != self.turn)
            .map(|(id, _)| id.clone()).collect();
        if !candidates.is_empty() { threads.push((self.thread.clone(), candidates)); }
        for (_, transcript) in self.subagents.values() { transcript.shell_threads(threads); }
    }

    fn reconcile_shells(&mut self, thread: &str, candidates: &HashSet<String>, live: &HashSet<String>) {
        if self.thread == thread {
            for id in candidates.difference(live) {
                if let Some((index, mut item, _)) = self.commands.remove(id) {
                    if let Item::Command { status, .. } | Item::Shell { status, .. } = &mut item { *status = CommandStatus::Failed; }
                    let _ = self.events.send(Event::Item(index, item));
                }
            }
        } else {
            for (subagent, transcript) in self.subagents.values_mut() {
                transcript.reconcile_shells(thread, candidates, live);
                if subagent.update() { let _ = self.events.send(Event::Item(subagent.index, subagent.item())); }
            }
        }
    }

    fn new(thread: &str, cwd: &Path, events: UnboundedSender<Event>) -> Self {
        Self {
            data_dir: None,
            last_input: None,
            thread: thread.to_owned(),
            chat_id: String::new(),
            cwd: cwd.to_owned(),
            storage_roots: Vec::new(),
            events,
            len: 0,
            checkpoints: HashMap::new(),
            starting: None,
            turn: None,
            streaming: HashMap::new(),
            thinking: HashMap::new(),
            tools: None,
            activity: super::Activity::default(),
            commands: HashMap::new(),
            subagents: HashMap::new(),
            delegated: HashMap::new(),
            previous: 0,
            steering: HashMap::new(),
            unsteered: None,
            status: Status::Done,
            asked: HashMap::new(),
            edits: HashMap::new(),
            settings: Settings::default(),
            models: Vec::new(),
            skills: Vec::new(),
            compacting: None,
            applying: Vec::new(),
            lists: HashMap::new(),
            questions: HashMap::new(),
            plan: None,
            plan_resolved: None,
        }
    }

    fn idle(&self) -> bool {
        self.starting.is_none() && self.turn.is_none() && self.compacting.is_none()
    }

    fn listed(&self) -> Option<&Value> {
        self.models.iter().find(|listed| listed["model"].as_str() == self.settings.chosen.model.as_deref())
    }

    fn efforts(&self) -> Vec<String> {
        let efforts = self.listed().and_then(|listed| listed["supportedReasoningEfforts"].as_array());
        let efforts = efforts.into_iter().flatten().filter_map(|effort| effort["reasoningEffort"].as_str());
        efforts.map(str::to_owned).collect()
    }

    fn declare(&self) {
        let commands = self.skills.iter().map(|skill| {
            let short = skill["interface"]["shortDescription"].as_str().or(skill["shortDescription"].as_str());
            AgentCommand {
                name: skill["name"].as_str().unwrap_or_default().to_owned(),
                description: short.or(skill["description"].as_str()).unwrap_or_default().to_owned(),
                hint: String::new(),
            }
        });
        let capabilities = Capabilities {
            rewinds_files: false,
            models: self.models.iter().filter_map(|model| model["model"].as_str()).map(str::to_owned).collect(),
            efforts: self.efforts(),
            modes: Agent::Codex.chat_modes().into_iter().map(Choice::mode).collect(),
            commands: commands.collect(),
        };
        let _ = self.events.send(Event::Capabilities(capabilities));
    }

    fn report_settings(&self) {
        let _ = self.events.send(Event::Settings(self.settings.clone()));
    }

    fn choose(&mut self, setting: Setting, value: String) {
        *self.settings.get_mut(setting) = Some(value);
        self.settings.mark_pending(setting);
        if setting == Setting::Model {
            let efforts = self.efforts();
            if self.settings.chosen.effort.as_ref().is_some_and(|effort| !efforts.contains(effort)) {
                let fallback = self.listed().and_then(|listed| listed["defaultReasoningEffort"].as_str());
                self.settings.chosen.effort = fallback.map(str::to_owned);
                self.settings.mark_pending(Setting::Effort);
            }
            self.declare();
        }
        self.report_settings();
    }

    fn overrides(&mut self, turn: &mut Value) {
        if self.settings.pending.is_empty() {
            return;
        }
        let Settings { chosen: AgentSettings { model, effort, mode }, pending } = &self.settings;
        turn["model"] = json!(model);
        turn["effort"] = json!(effort);
        let collaboration = if mode.as_deref() == Some("plan") { "plan" } else { "default" };
        let settings = json!({ "model": model, "reasoning_effort": effort, "developer_instructions": null });
        turn["collaborationMode"] = json!({ "mode": collaboration, "settings": settings });
        if let Some((approval, sandbox, reviewer)) = mode.as_deref().and_then(codex_policy) {
            turn["approvalPolicy"] = json!(approval);
            turn["sandboxPolicy"] = json!({ "type": policy_type(sandbox) });
            if sandbox == "workspace-write" && !self.storage_roots.is_empty() {
                turn["sandboxPolicy"]["writableRoots"] = json!(self.storage_roots);
            }
            turn["approvalsReviewer"] = json!(reviewer);
        }
        self.applying = pending.iter().map(|&setting| (setting, self.settings.get(setting).clone())).collect();
    }

    fn skill(&self, text: &str) -> Option<Value> {
        let name = text.strip_prefix('/')?.split_whitespace().next()?;
        let skill = self.skills.iter().find(|skill| skill["name"] == name)?;
        Some(json!({ "type": "skill", "name": name, "path": skill["path"] }))
    }

    /// Moves to the new thread `/clear` started, keeping the chosen settings for its first turn.
    fn restart(&mut self, started: &Value) {
        let Some(thread) = started["thread"]["id"].as_str() else { return };
        self.thread = thread.to_owned();
        self.activity.clear(&self.events);
        self.len = 0;
        self.streaming.clear();
        self.thinking.clear();
        self.tools = None;
        self.commands.clear();
        self.subagents.clear();
        self.delegated.clear();
        self.asked.clear();
        self.edits.clear();
        self.steering.clear();
        self.unsteered = None;
        self.applying.clear();
        self.checkpoints.clear();
        self.questions.clear();
        self.plan = None;
        self.previous = 0;
        let _ = self.events.send(Event::Session(self.thread.clone()));
        let _ = self.events.send(Event::Reset);
        let _ = self.events.send(Event::Truncate(0));
        let fresh = current(started);
        for setting in [Setting::Model, Setting::Effort, Setting::Mode] {
            match self.settings.get(setting) {
                None => *self.settings.get_mut(setting) = fresh.get(setting).clone(),
                chosen if chosen != fresh.get(setting) => self.settings.mark_pending(setting),
                _ => {}
            }
        }
        self.report_settings();
    }

    fn report(&mut self, status: Status) {
        if status == Status::Done { self.activity.clear(&self.events); }
        self.status = status;
        let _ = self.events.send(Event::Status(status));
    }

    fn delegated(&mut self, item: Item) {
        let Item::Subagent { id, .. } = &item else { return };
        if let Some(&index) = self.delegated.get(id) {
            let _ = self.events.send(Event::Item(index, item));
        } else {
            let id = id.clone();
            let index = self.push(item);
            self.delegated.insert(id, index);
        }
    }

    fn push(&mut self, item: Item) -> usize {
        let index = self.len;
        self.len += 1;
        self.tools = None;
        let _ = self.events.send(Event::Item(index, item));
        index
    }

    fn stream(&mut self, id: &str, text: &str, phase: Phase) {
        let index = self.push(Item::Text { text: text.to_owned(), streaming: true, phase });
        self.streaming.insert(id.to_owned(), (index, text.to_owned(), phase));
    }

    fn resumed(&mut self, turns: &[Value]) -> Status {
        self.activity.clear(&self.events);
        let _ = self.events.send(Event::Truncate(0));
        self.len = 0;
        self.tools = None;
        self.starting = None;
        self.streaming.clear();
        self.thinking.clear();
        self.commands.clear();
        self.subagents.clear();
        self.delegated.clear();
        self.steering.clear();
        self.unsteered = None;
        self.asked.clear();
        self.edits.clear();
        self.checkpoints.clear();
        self.plan = None;
        for turn in turns {
            self.turn = turn["id"].as_str().map(str::to_owned);
            for item in turn["items"].as_array().into_iter().flatten() {
                match item["type"].as_str() {
                    Some("userMessage") => {
                        self.last_input = Some(item["content"].clone());
                        self.push_user(item);
                    }
                    Some("fileChange") if item["status"] == "inProgress" => self.edited(item),
                    _ => self.completed(item, 0),
                }
            }
        }
        let last = turns.last();
        let running = last.filter(|turn| turn["status"] == "inProgress");
        self.turn = running.and_then(|turn| turn["id"].as_str()).map(str::to_owned);
        if self.status != Status::Done && running.is_none() && !last.is_some_and(|turn| turn["status"] == "completed") {
            if self.status == Status::Working { let _ = self.events.send(Event::Recoverable); }
            self.push(Item::interrupted());
        }
        if self.turn.is_some() { Status::Working } else { Status::Done }
    }

    /// Rebuilds a Subagent from its thread and its history.
    fn rejoined(&mut self, child: &str, thread: &Value, turns: &[Value]) {
        let Some((subagent, transcript)) = self.subagents.get_mut(child) else { return };
        for turn in turns {
            transcript.turn = turn["id"].as_str().map(str::to_owned);
            for item in turn["items"].as_array().into_iter().flatten() { transcript.completed(item, 0); }
        }
        transcript.turn = turns.last().filter(|turn| turn["status"] == "inProgress")
            .and_then(|turn| turn["id"].as_str()).map(str::to_owned);
        subagent.update();
        if let Some(name) = thread["agentRole"].as_str().or(thread["agentNickname"].as_str()) {
            subagent.name = Some(name.to_owned());
        }
        if subagent.model.is_none() {
            subagent.model = thread["model"].as_str().map(str::to_owned);
        }
        if let Some(created) = thread["createdAt"].as_u64() {
            subagent.started_at = created * 1000;
        }
        let durations = turns.iter().filter_map(|turn| turn["durationMs"].as_u64());
        subagent.duration = durations.reduce(|total, duration| total + duration).or(subagent.duration);
        let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
    }

    fn propose(&self) {
        if self.settings.chosen.mode.as_deref() == Some("plan")
            && let Some((id, plan)) = &self.plan
            && self.plan_resolved.as_ref() != Some(id)
        {
            self.raise(super::RequestKind::Plan { id: id.clone(), plan: plan.clone() });
        }
    }

    fn checkpoint(&mut self) {
        if let Some(turn) = &self.turn { self.checkpoints.insert(self.len, turn.clone()); }
    }

    fn push_user(&mut self, input: &Value) {
        self.checkpoint();
        let boundary = self.checkpoints.iter().filter(|(_, turn)| Some(*turn) == self.turn.as_ref()).map(|(index, _)| *index).min();
        let mut item = user(input);
        if let Item::User { rewind_from, .. } = &mut item {
            *rewind_from = boundary.filter(|index| *index < self.len);
        }
        self.push(item);
    }

    fn edited(&mut self, item: &Value) {
        if let Some(id) = item["id"].as_str() {
            self.edits.insert(id.to_owned(), item["changes"].clone());
        }
    }

    fn ask(&mut self, message: &Value, grant: Option<Value>) -> String {
        let id = key(&message["id"]);
        let child = message["params"]["threadId"].as_str().filter(|thread| self.subagents.contains_key(*thread)).map(str::to_owned);
        if let Some(child) = &child {
            self.child_status(child, Status::Waiting);
        }
        self.asked.insert(id.clone(), (message["id"].clone(), grant, child));
        id
    }

    fn raise(&self, kind: super::RequestKind) {
        let subagent = self.asked.get(kind.id()).and_then(|(_, _, child)| child.clone());
        let _ = self.events.send(Event::Request(super::Request { kind, subagent }));
    }

    fn resolved(&mut self, id: &str) -> Option<(Value, Option<Value>)> {
        let (rpc, grant, child) = self.asked.remove(id)?;
        if let Some(child) = child
            && !self.asked.values().any(|(_, _, pending)| pending.as_ref() == Some(&child))
        {
            self.child_status(&child, Status::Working);
        }
        Some((rpc, grant))
    }

    fn child_status(&mut self, thread: &str, status: Status) {
        if let Some((subagent, _)) = self.subagents.get_mut(thread)
            && subagent.status != Status::Done && subagent.status != status
        {
            subagent.status = status;
            let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
        }
    }

    fn read(&mut self, message: &Value) {
        if let Some(list) = response(message).and_then(|id| self.lists.remove(&id)) {
            match list {
                List::Models => self.models = message["result"]["data"].as_array().cloned().unwrap_or_default(),
                List::Skills => self.skills = enabled(&message["result"]),
            }
            return self.declare();
        }
        if self.starting.is_some() && response(message) == self.starting {
            self.starting = None;
            let applying = std::mem::take(&mut self.applying);
            match message["result"]["turn"]["id"].as_str() {
                Some(turn) => {
                    self.turn = Some(turn.to_owned());
                    if !applying.is_empty() {
                        for (setting, value) in applying {
                            if self.settings.get(setting) == &value {
                                self.settings.pending.retain(|pending| *pending != setting);
                            }
                        }
                        self.report_settings();
                    }
                }
                None => {
                    eprintln!("codex turn/start failed: {}", message["error"]);
                    self.report(Status::Done);
                }
            }
            return;
        }
        if let Some(input) = response(message).and_then(|id| self.steering.remove(&id)) {
            if message.get("error").is_some() {
                eprintln!("codex turn/steer failed: {}", message["error"]);
                self.unsteered = Some(input);
            }
            return;
        }
        if self.compacting.is_some() && response(message) == self.compacting {
            self.compacting = None;
            if let Some(error) = message.get("error") {
                eprintln!("codex thread/compact/start failed: {error}");
                self.report(Status::Done);
            }
            return;
        }
        let params = &message["params"];
        if let Some(thread) = params["threadId"].as_str()
            && thread != self.thread
        {
            if self.subagents.contains_key(thread) {
                if message["method"] == "item/started" && params["item"]["type"] == "fileChange" {
                    self.edited(&params["item"]);
                }
                if message["method"] == "item/completed" && let Some(id) = params["item"]["id"].as_str() {
                    self.edits.remove(id);
                }
            }
            if let Some((subagent, transcript)) = self.subagents.get_mut(thread)
            {
                transcript.read(message);
                if message["method"] == "turn/completed"
                    && let Some(duration) = params["turn"]["durationMs"].as_u64()
                {
                    subagent.duration = Some(subagent.duration.unwrap_or_default() + duration);
                }
                if subagent.update() {
                    let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
                }
            }
            let request = message.get("id").is_some() && message.get("method").is_some();
            let resolved = message["method"] == "serverRequest/resolved";
            if !self.subagents.contains_key(thread) || (!request && !resolved) {
                return;
            }
        }
        let item = &params["item"];
        if message["method"] == "item/started"
            && let Some(id) = item["id"].as_str()
        {
            let label = match item["type"].as_str() {
                Some("reasoning") => Some("Thinking…".to_owned()),
                Some("agentMessage") => Some("Writing response…".to_owned()),
                Some("commandExecution") => {
                    let (Item::Command { command, .. } | Item::Shell { command, .. }) = command(item) else { unreachable!() };
                    Some(format!("Running {command}"))
                }
                Some("webSearch") => Some("Searching the web…".to_owned()),
                Some("mcpToolCall") => Some(format!("Using {}.{}", item["server"].as_str().unwrap_or_default(), item["tool"].as_str().unwrap_or_default())),
                Some("dynamicToolCall") => Some(format!("Using {}", item["tool"].as_str().unwrap_or_default())),
                Some("fileChange") => Some("Editing files…".to_owned()),
                Some("imageView") => Some("Viewing image…".to_owned()),
                Some("collabAgentToolCall") => Some(match item["tool"].as_str() {
                    Some("wait") => "Waiting for agents…".to_owned(),
                    Some("spawnAgent") => "Starting an agent…".to_owned(),
                    _ => "Coordinating agents…".to_owned(),
                }),
                Some("contextCompaction") => Some("Compacting conversation…".to_owned()),
                _ => None,
            };
            if let Some(label) = label { self.activity.start(id, label, &self.events); }
        }
        if message["method"] == "item/completed" && let Some(id) = item["id"].as_str() {
            self.activity.finish(id, &self.events);
        }
        match message["method"].as_str() {
            Some("thread/tokenUsage/updated") => {
                let usage = &params["tokenUsage"];
                let (used, window) = (usage["last"]["totalTokens"].as_u64(), usage["modelContextWindow"].as_u64());
                let context = used.zip(window).map(|(used, window)| Context { used, window });
                let (input, output) = (usage["total"]["inputTokens"].as_u64(), usage["total"]["outputTokens"].as_u64());
                let tokens = input.zip(output).map(|(input, output)| Tokens { input, output });
                let _ = self.events.send(Event::Meters(context, tokens));
            }
            Some("item/started") if item["type"] == "fileChange" => self.edited(item),
            Some("item/commandExecution/requestApproval") => {
                let actions = params["commandActions"].as_array().into_iter().flatten();
                let actions: Vec<&str> = actions.filter_map(|action| action["command"].as_str()).collect();
                let words = match actions.as_slice() {
                    [] => words(params["command"].as_str().unwrap_or_default()),
                    actions => actions.iter().flat_map(|action| words(action)).collect(),
                };
                let id = self.ask(message, None);
                let proposed = params["proposedExecpolicyAmendment"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>();
                let suggestion = words.iter().map(|part| if !proposed.is_empty() && part.starts_with(&proposed) { proposed.clone() } else { part.clone() }).collect();
                self.raise(super::RequestKind::Command { id, words, suggestion, session_prefixes: false, allow_rules: None });
            }
            Some("item/tool/requestUserInput") => {
                let id = self.ask(message, None);
                let mut raw = params["questions"].clone();
                for question in raw.as_array_mut().into_iter().flatten() {
                    if question["options"].is_null() { question["options"] = json!([]); }
                }
                let questions: Vec<Question> = serde_json::from_value(raw).unwrap_or_default();
                self.questions.insert(id.clone(), questions.clone());
                self.raise(super::RequestKind::Question { id, questions });
            }
            Some("item/fileChange/requestApproval") => {
                let id = self.ask(message, None);
                let kind = match self.edits.get(params["itemId"].as_str().unwrap_or_default()) {
                    Some(changes) => edit(id, changes),
                    None => {
                        let detail = params["reason"].as_str().unwrap_or_default().to_owned();
                        super::RequestKind::Other { id, title: "Codex wants to edit files".into(), detail }
                    }
                };
                self.raise(kind);
            }
            Some("item/permissions/requestApproval") => {
                let permissions = &params["permissions"];
                let id = self.ask(message, Some(permissions.clone()));
                let reason = params["reason"].as_str().unwrap_or_default();
                let detail = format!("{reason}\n{}", serde_json::to_string_pretty(permissions).unwrap_or_default());
                let title = "Codex wants more permissions".into();
                self.raise(super::RequestKind::Other { id, title, detail: detail.trim().to_owned() });
            }
            Some("serverRequest/resolved") => {
                let id = key(&params["requestId"]);
                if self.resolved(&id).is_some() {
                    let _ = self.events.send(Event::Resolved(id));
                }
            }
            Some("turn/plan/updated") => {
                let steps = params["plan"].as_array().into_iter().flatten();
                let todos = steps.map(|step| {
                    let status = match step["status"].as_str() {
                        Some("inProgress") => TodoStatus::Active,
                        Some("completed") => TodoStatus::Done,
                        _ => TodoStatus::Pending,
                    };
                    Todo { text: step["step"].as_str().unwrap_or_default().to_owned(), status }
                });
                let _ = self.events.send(Event::Todos(todos.collect()));
            }
            Some("turn/started") => {
                self.turn = params["turn"]["id"].as_str().map(str::to_owned);
                self.plan = None;
                if self.status != Status::Working { self.report(Status::Working); }
            }
            Some("turn/completed") => {
                self.turn = None;
                // A message cut off by an interrupt or a steer never gets its item/completed.
                for (_, (index, text, phase)) in self.streaming.drain() {
                    let _ = self.events.send(Event::Item(index, Item::Text { text, streaming: false, phase }));
                }
                if params["turn"]["status"] == "failed" {
                    self.push(notice(&params["turn"]["error"]));
                }
                if params["turn"]["status"] == "interrupted" {
                    self.push(Item::interrupted());
                }
                self.report(Status::Done);
                if params["turn"]["status"] == "completed" { self.propose(); }
            }
            Some("item/started") if item["type"] == "userMessage" => {
                self.last_input = Some(item["content"].clone());
                self.push_user(item);
            }
            Some("item/started") if item["type"] == "agentMessage" => {
                let (Some(id), Some(text)) = (item["id"].as_str(), item["text"].as_str()) else { return };
                self.stream(id, text, phase(item));
            }
            Some("item/agentMessage/delta") => {
                let Some(id) = params["itemId"].as_str() else { return };
                let delta = params["delta"].as_str().unwrap_or_default();
                match self.streaming.get_mut(id) {
                    Some((index, text, phase)) => {
                        text.push_str(delta);
                        let item = Item::Text { text: text.clone(), streaming: true, phase: *phase };
                        let _ = self.events.send(Event::Item(*index, item));
                    }
                    // A resume rejoins a streaming message without its item/started or its text so far.
                    None => self.stream(id, delta, Phase::Final),
                }
            }
            Some("item/reasoning/summaryTextDelta") => {
                let Some(id) = params["itemId"].as_str() else { return };
                if !self.thinking.contains_key(id) {
                    let index = self.push(Item::Thinking { seconds: 0, text: None });
                    self.thinking.insert(id.to_owned(), (index, BTreeMap::new()));
                }
                let (index, parts) = self.thinking.get_mut(id).unwrap();
                parts.entry(params["summaryIndex"].as_u64().unwrap_or_default()).or_default()
                    .push_str(params["delta"].as_str().unwrap_or_default());
                let text = parts.values().cloned().collect::<Vec<_>>().join("\n\n");
                let _ = self.events.send(Event::Item(*index, Item::Thinking { seconds: 0, text: Some(text) }));
            }
            Some("item/started") if item["type"] == "commandExecution" => {
                self.command(item);
            }
            Some("item/commandExecution/outputDelta") => {
                if let Some((index, item, _)) = params["itemId"].as_str().and_then(|id| self.commands.get_mut(id)) {
                    if let Item::Command { output, .. } | Item::Shell { output, .. } = item {
                        output.push_str(params["delta"].as_str().unwrap_or_default());
                    }
                    let _ = self.events.send(Event::Item(*index, item.clone()));
                }
            }
            Some("item/completed") => {
                let at = params["completedAtMs"].as_u64().unwrap_or_default();
                let since = std::mem::replace(&mut self.previous, at);
                self.completed(item, if since == 0 { 0 } else { at.saturating_sub(since) });
            }
            Some("account/rateLimits/updated") => {
                let _ = self.events.send(Event::Usage(readings(&params["rateLimits"])));
            }
            _ => {}
        }
    }

    fn completed(&mut self, item: &Value, elapsed: u64) {
        if item["type"] == "fileChange" {
            self.edits.remove(item["id"].as_str().unwrap_or_default());
        }
        let text = |key: &str| item[key].as_str().unwrap_or_default().to_owned();
        match item["type"].as_str() {
            Some("agentMessage") => {
                if self.settings.chosen.mode.as_deref() == Some("plan")
                    && (text("text").contains("<proposed_plan>") || (item["phase"] == "final_answer" && text("text").starts_with("Plan:")))
                {
                    let plan = text("text").replace("<proposed_plan>", "").replace("</proposed_plan>", "").trim().to_owned();
                    self.plan = Some((format!("plan:{}", text("id")), plan));
                }
                match item["id"].as_str().and_then(|id| self.streaming.remove(id)) {
                Some((index, ..)) => {
                    let _ = self.events.send(Event::Item(index, agent(item)));
                }
                None => {
                    self.push(agent(item));
                }
                }
            }
            Some("plan") => {
                if self.settings.chosen.mode.as_deref() == Some("plan") {
                    self.plan = Some((format!("plan:{}", text("id")), text("text")));
                }
            }
            Some("reasoning") => {
                let parts = |key: &str| {
                    let parts: Vec<&str> = item[key].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                    Some(parts.join("\n\n")).filter(|text| !text.is_empty())
                };
                let streamed = self.thinking.remove(&text("id"));
                let text = parts("summary").or_else(|| parts("content")).or_else(|| streamed.as_ref().map(|(_, parts)| parts.values().cloned().collect::<Vec<_>>().join("\n\n")));
                let item = Item::Thinking { seconds: seconds(elapsed), text };
                if let Some((index, _)) = streamed {
                    let _ = self.events.send(Event::Item(index, item));
                } else {
                    self.push(item);
                }
            }
            Some("commandExecution") => self.command(item),
            Some("fileChange") if item["status"] == "completed" => {
                for change in item["changes"].as_array().into_iter().flatten() {
                    let (kind, diff) = (&change["kind"], change["diff"].as_str().unwrap_or_default());
                    let path = kind["move_path"].as_str().or(change["path"].as_str()).unwrap_or_default();
                    // An added or deleted file's diff is its whole content.
                    let diff = match kind["type"].as_str() {
                        Some("add") => unified("", diff),
                        Some("delete") => unified(diff, ""),
                        _ => diff.to_owned(),
                    };
                    self.push(Item::edit(relative(&self.cwd, path), diff, kind["type"] == "add"));
                }
            }
            Some("webSearch") => {
                let action = &item["action"];
                let detail = |key: &str| action[key].as_str().map_or_else(|| text("query"), str::to_owned);
                match action["type"].as_str() {
                    Some("open_page") => self.call(ToolKind::Fetch, "webSearch", detail("url")),
                    Some("find_in_page") => self.call(ToolKind::Search, "webSearch", detail("pattern")),
                    _ => self.call(ToolKind::Web, "webSearch", text("query")),
                }
            }
            Some("mcpToolCall") => {
                if item["server"] == html::SERVER && item["tool"] == super::delegation::TOOL {
                    if let Some(delegated) = super::delegation::item(&item["result"], self.data_dir.as_deref()) {
                        if let Item::Subagent { id, .. } = &delegated
                            && !self.delegated.contains_key(id)
                        {
                            self.delegated(delegated);
                        }
                    }
                    return;
                }
                if item["server"] == html::SERVER
                    && item["status"] == "completed" && item["result"]["isError"] != true
                    && let Some(snippet) = match item["tool"].as_str() {
                        Some(html::TOOL) => html::item(&item["arguments"]),
                        Some(crate::media::TOOL) => crate::media::item(&item["result"]),
                        _ => None,
                    }
                {
                    self.push(snippet);
                    return;
                }
                let name = format!("{}.{}", text("server"), text("tool"));
                self.call(ToolKind::Mcp, &name, item["arguments"].to_string());
            }
            Some("dynamicToolCall") => self.call(ToolKind::Other, &text("tool"), item["arguments"].to_string()),
            Some("imageView") => self.call(ToolKind::Read, "imageView", text("path")),
            Some("contextCompaction") => {
                self.push(Item::compacted());
            }
            Some("collabAgentToolCall") => {
                let prompt = item["prompt"].as_str().filter(|_| item["tool"] == "spawnAgent");
                for thread in item["receiverThreadIds"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    if let Some(prompt) = prompt {
                        self.spawn(thread, prompt.lines().next().unwrap_or_default());
                    }
                    if let Some((subagent, _)) = self.subagents.get_mut(thread) {
                        if let Some(prompt) = prompt {
                            subagent.prompt.get_or_insert_with(|| prompt.to_owned());
                        }
                        let status = item["agentsStates"][thread]["status"].as_str().unwrap_or_default();
                        if status == "waiting" {
                            subagent.status = Status::Waiting;
                        } else if let Some(outcome) = outcome(status) {
                            subagent.finish(Some(outcome));
                        }
                        let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
                    }
                }
            }
            Some("subAgentActivity") => {
                let Some(thread) = item["agentThreadId"].as_str() else { return };
                match item["kind"].as_str() {
                    Some("started") => {
                        let path = text("agentPath");
                        let task = path.rsplit('/').next().unwrap_or_default();
                        self.spawn(thread, task);
                        if let Some((subagent, _)) = self.subagents.get_mut(thread) {
                            if !task.is_empty() {
                                subagent.task = task.to_owned();
                            }
                            if let Some(model) = item["model"].as_str() {
                                subagent.model = Some(model.to_owned());
                            }
                            let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
                        }
                    }
                    Some(kind @ ("completed" | "interrupted")) => {
                        if let Some((subagent, _)) = self.subagents.get_mut(thread) {
                            subagent.update();
                            subagent.finish(outcome(kind));
                            let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn command(&mut self, value: &Value) {
        let id = value["id"].as_str().unwrap_or_default();
        let mut item = command(value);
        let mut turn = self.turn.clone();
        let index = match self.commands.remove(id) {
            Some((index, previous, started)) => {
                turn = started;
                if (value["aggregatedOutput"].is_null() || value["exitCode"] == -1)
                    && let (Item::Command { output, .. } | Item::Shell { output, .. }, Item::Command { output: previous, .. } | Item::Shell { output: previous, .. }) = (&mut item, previous)
                {
                    *output = previous;
                }
                let _ = self.events.send(Event::Item(index, item.clone()));
                index
            }
            None => self.push(item.clone()),
        };
        if matches!(item, Item::Command { status: CommandStatus::Running, .. } | Item::Shell { status: CommandStatus::Running, .. }) {
            self.commands.insert(id.to_owned(), (index, item, turn));
        }
    }

    fn spawn(&mut self, thread: &str, task: &str) {
        if self.subagents.contains_key(thread) {
            return;
        }
        let (mut subagent, events) = Subagent::new(thread, task.to_owned(), super::now());
        subagent.index = self.push(subagent.item());
        self.subagents.insert(thread.to_owned(), (subagent, Transcript::new(thread, &self.cwd, events)));
    }

    fn call(&mut self, kind: ToolKind, name: &str, detail: String) {
        let call = ToolCall { kind, name: name.to_owned(), detail };
        match &mut self.tools {
            Some((index, calls)) => {
                calls.push(call);
                let _ = self.events.send(Event::Item(*index, Item::Tools { calls: calls.clone() }));
            }
            None => {
                let index = self.push(Item::Tools { calls: vec![call.clone()] });
                self.tools = Some((index, vec![call]));
            }
        }
    }
}

fn command(item: &Value) -> Item {
    let line = item["command"].as_str().unwrap_or_default();
    let command = match shlex::split(line).as_deref() {
        Some([_, flag, script]) if flag == "-lc" || flag == "-c" => script.clone(),
        _ => line.to_owned(),
    };
    let status = match item["status"].as_str() {
        Some("inProgress") => CommandStatus::Running,
        Some("completed") => CommandStatus::Ok,
        _ => CommandStatus::Failed,
    };
    let output = item["aggregatedOutput"].as_str().unwrap_or_default().to_owned();
    match item["source"].as_str() {
        Some("userShell") => Item::Shell { command, status, output, pending: false },
        _ => Item::Command { command, status, output },
    }
}

fn outcome(status: &str) -> Option<Outcome> {
    match status {
        "completed" => Some(Outcome::Completed),
        "errored" => Some(Outcome::Failed),
        "shutdown" | "interrupted" | "notFound" => Some(Outcome::Stopped),
        _ => None,
    }
}

fn notice(error: &Value) -> Item {
    let message = error["message"].as_str().unwrap_or_default();
    let api: Option<Value> = serde_json::from_str(message).ok();
    let text = api.as_ref().and_then(|api| api["error"]["message"].as_str()).unwrap_or(message).to_owned();
    let limit = ["usageLimitExceeded", "rateLimitExceeded", "sessionBudgetExceeded"];
    let notice = if limit.iter().any(|limit| error["codexErrorInfo"] == *limit) { Notice::Limit } else { Notice::Error };
    Item::Notice { notice, text }
}

fn user(item: &Value) -> Item {
    let content = item["content"].as_array().into_iter().flatten();
    let images = content.clone().filter(|input| input["type"] == "image" || input["type"] == "localImage").count();
    let image_urls = content.clone().filter(|input| input["type"] == "image").filter_map(|input| input["url"].as_str().map(str::to_owned)).collect();
    let text: Vec<&str> = content.filter_map(|input| input["text"].as_str()).collect();
    Item::User { text: text.join("\n"), images, image_urls, template: None, rewind_from: None }
}

fn agent(item: &Value) -> Item {
    Item::Text { text: item["text"].as_str().unwrap_or_default().to_owned(), streaming: false, phase: phase(item) }
}

fn phase(item: &Value) -> Phase {
    if item["phase"] == "commentary" { Phase::Commentary } else { Phase::Final }
}

fn key(id: &Value) -> String {
    id.as_str().map_or_else(|| id.to_string(), str::to_owned)
}

fn edit(id: String, changes: &Value) -> super::RequestKind {
    let changes = changes.as_array().map(Vec::as_slice).unwrap_or_default();
    let paths: Vec<&str> = changes.iter().filter_map(|change| change["path"].as_str()).collect();
    let path = paths.join(", ");
    let diff: String = changes
        .iter()
        .map(|change| {
            let text = change["diff"].as_str().unwrap_or_default();
            match change["kind"]["type"].as_str() {
                Some("add") => diff("", text),
                Some("delete") => diff(text, ""),
                _ => text.to_owned(),
            }
        })
        .collect();
    super::RequestKind::Edit { id, path, diff }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subagents_publish_reasoning_and_output_before_completion() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new("thread", Path::new("/project"), events);
        transcript.spawn("child", "Review");
        for (method, params) in [
            ("item/started", json!({ "item": { "id": "thinking", "type": "reasoning", "summary": [], "content": [] } })),
            ("item/reasoning/summaryTextDelta", json!({ "itemId": "thinking", "summaryIndex": 0, "delta": "Checking the implementation." })),
            ("item/started", json!({ "item": { "id": "output", "type": "agentMessage", "text": "", "phase": "final_answer" } })),
            ("item/agentMessage/delta", json!({ "itemId": "output", "delta": "The first finding" })),
        ] {
            let mut params = params;
            params["threadId"] = json!("child");
            transcript.read(&json!({ "method": method, "params": params }));
        }
        let items = std::iter::from_fn(|| received.try_recv().ok()).filter_map(|event| match event {
            Event::Item(_, Item::Subagent { status: Status::Working, items, .. }) => Some(items),
            _ => None,
        }).last().unwrap();
        assert!(items.iter().any(|item| matches!(item, Item::Thinking { text, .. } if text.as_deref() == Some("Checking the implementation."))), "{items:?}");
        assert!(items.iter().any(|item| matches!(item, Item::Text { text, streaming: true, .. } if text == "The first finding")), "{items:?}");
    }

    #[test]
    fn reset_credits_keep_the_authoritative_count_and_choose_the_first_expiry() {
        let snapshot = usage_snapshot(&json!({ "rateLimitResetCredits": {
            "availableCount": 8,
            "credits": [
                { "id": "later", "status": "available", "resetType": "codexRateLimits", "expiresAt": 500 },
                { "id": "used", "status": "redeemed", "resetType": "codexRateLimits", "expiresAt": 100 },
                { "id": "other", "status": "available", "resetType": "unknown", "expiresAt": 50 },
                { "id": "next", "status": "available", "resetType": "codexRateLimits", "expiresAt": 300 },
                { "id": "forever", "status": "available", "resetType": "codexRateLimits", "expiresAt": null }
            ]
        }}));
        assert_eq!(snapshot.codex_resets, Some(ResetCredits { available_count: 8, expires_at: Some(300_000), credit_id: Some("next".to_owned()) }));
        for credits in [Value::Null, json!([])] {
            let snapshot = usage_snapshot(&json!({ "rateLimitResetCredits": { "availableCount": 3, "credits": credits } }));
            assert_eq!(snapshot.codex_resets, Some(ResetCredits { available_count: 3, expires_at: None, credit_id: None }));
        }
        assert!(usage_snapshot(&json!({})).codex_resets.is_none());
    }

    #[test]
    fn shell_reconciliation_preserves_foreground_commands_and_updates_subagents() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new("thread", Path::new("/project"), events);
        transcript.turn = Some("previous".to_owned());
        transcript.command(&json!({ "id": "old", "command": "pnpm dev", "status": "inProgress", "aggregatedOutput": "Ready\n" }));
        transcript.turn = Some("current".to_owned());
        transcript.command(&json!({ "id": "foreground", "command": "cargo test", "status": "inProgress" }));
        transcript.spawn("child", "Build");
        let (_, child) = transcript.subagents.get_mut("child").unwrap();
        child.command(&json!({ "id": "nested", "command": "pnpm watch", "status": "inProgress" }));
        let mut threads = Vec::new();
        transcript.shell_threads(&mut threads);
        assert_eq!(threads.len(), 2);
        for (thread, candidates) in threads {
            transcript.reconcile_shells(&thread, &candidates, &HashSet::new());
        }
        assert_eq!(transcript.commands.len(), 1);
        assert!(transcript.commands.contains_key("foreground"));
        assert!(transcript.subagents["child"].1.commands.is_empty());
        let mut items = BTreeMap::new();
        while let Ok(event) = received.try_recv() {
            if let Event::Item(index, item) = event { items.insert(index, item); }
        }
        assert!(matches!(&items[&0], Item::Command { status: CommandStatus::Failed, output, .. } if output == "Ready\n"));
        assert!(matches!(&items[&1], Item::Command { status: CommandStatus::Running, .. }));
        assert!(matches!(&items[&2], Item::Subagent { items, .. } if matches!(items[0], Item::Command { status: CommandStatus::Failed, .. })));
    }

    #[test]
    fn running_shells_stream_and_complete_in_place_after_resume() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new("thread", Path::new("/project"), events);
        let item = json!({ "type": "commandExecution", "id": "shell", "command": "pnpm dev", "status": "inProgress" });
        transcript.resumed(&[json!({ "id": "turn", "status": "completed", "items": [item] })]);
        transcript.read(&json!({ "method": "item/commandExecution/outputDelta", "params": {
            "threadId": "thread", "itemId": "shell", "delta": "Server ready\n",
        } }));
        let mut items = BTreeMap::new();
        while let Ok(event) = received.try_recv() {
            if let Event::Item(index, item) = event { items.insert(index, item); }
        }
        assert!(matches!(items.get(&0), Some(Item::Command { status: CommandStatus::Running, output, .. }) if output == "Server ready\n"));
        let mut finished = item;
        finished["status"] = json!("completed");
        transcript.completed(&finished, 0);
        while let Ok(event) = received.try_recv() {
            if let Event::Item(index, item) = event { items.insert(index, item); }
        }
        assert_eq!(items.len(), 1);
        assert!(matches!(items.get(&0), Some(Item::Command { status: CommandStatus::Ok, output, .. }) if output == "Server ready\n"));
        assert!(transcript.commands.is_empty());
    }

    #[test]
    fn user_shell_commands_are_shell_items_that_keep_their_output_when_stopped() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new("thread", Path::new("/project"), events);
        let item = json!({ "type": "commandExecution", "id": "user", "source": "userShell", "command": "/bin/bash -lc 'cargo test'", "status": "inProgress" });
        transcript.read(&json!({ "method": "item/started", "params": { "threadId": "thread", "item": item } }));
        transcript.read(&json!({ "method": "item/commandExecution/outputDelta", "params": { "threadId": "thread", "itemId": "user", "delta": "running 3 tests\n" } }));
        let mut stopped = item;
        stopped["status"] = json!("failed");
        stopped["exitCode"] = json!(-1);
        stopped["aggregatedOutput"] = json!("command aborted by user");
        transcript.completed(&stopped, 0);
        let mut items = BTreeMap::new();
        while let Ok(event) = received.try_recv() {
            if let Event::Item(index, item) = event { items.insert(index, item); }
        }
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[&0], Item::Shell { command, status: CommandStatus::Failed, output, pending: false } if command == "cargo test" && output == "running 3 tests\n"));
    }

    #[test]
    fn html_renders_live_and_from_resumed_threads() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new("thread", Path::new("/project"), events);
        let arguments = json!({ "title": "Flow", "html": "<b>Start → Finish</b>" });
        let item = json!({ "type": "mcpToolCall", "id": "snippet", "server": html::SERVER,
            "tool": html::TOOL, "arguments": arguments, "status": "completed", "result": { "content": [] } });
        transcript.completed(&item, 0);
        assert!(matches!(received.try_recv().unwrap(), Event::Item(0, snippet) if Some(&snippet) == html::item(&arguments).as_ref()));
        transcript.resumed(&[json!({ "id": "turn", "status": "completed", "items": [item] })]);
        assert!(matches!(received.try_recv().unwrap(), Event::Truncate(0)));
        assert!(matches!(received.try_recv().unwrap(), Event::Item(0, snippet) if Some(&snippet) == html::item(&arguments).as_ref()));
        let mut failed = item.clone();
        failed["status"] = json!("failed");
        transcript.completed(&failed, 0);
        assert!(matches!(received.try_recv().unwrap(), Event::Item(1, Item::Tools { .. })));
    }

    #[test]
    fn media_renders_live_and_from_resumed_threads_without_the_source_file() {
        let asset = json!({ "title": "Screen recording", "asset": "f382e7e5-ff76-42cc-b150-cb8b59c8aefa.webm" });
        for result in [json!({ "structuredContent": asset }), json!({ "content": [{ "type": "text", "text": asset.to_string() }] })] {
            let (events, mut received) = mpsc::unbounded_channel();
            let mut transcript = Transcript::new("thread", Path::new("/project"), events);
            let item = json!({ "type": "mcpToolCall", "id": "media", "server": html::SERVER,
                "tool": crate::media::TOOL, "arguments": { "path": "/removed/recording.webm", "title": "Screen recording" },
                "status": "completed", "result": result });
            transcript.completed(&item, 0);
            assert!(matches!(received.try_recv().unwrap(), Event::Item(0, media) if Some(&media) == crate::media::item(&result).as_ref()));
            transcript.resumed(&[json!({ "id": "turn", "status": "completed", "items": [item] })]);
            assert!(matches!(received.try_recv().unwrap(), Event::Truncate(0)));
            assert!(matches!(received.try_recv().unwrap(), Event::Item(0, media) if Some(&media) == crate::media::item(&result).as_ref()));
            let mut failed = item.clone();
            failed["result"]["isError"] = json!(true);
            transcript.completed(&failed, 0);
            assert!(matches!(received.try_recv().unwrap(), Event::Item(1, Item::Tools { .. })));
        }
    }

    #[test]
    fn approve_for_me_starts_with_automatic_review_inside_the_workspace() {
        let settings = AgentSettings { mode: Some("approve-for-me".to_owned()), ..AgentSettings::default() };
        let params = thread(Path::new("/data"), "chat", Path::new("/project"), &settings, &BTreeMap::new());
        assert_eq!(params["approvalPolicy"], "on-request");
        assert_eq!(params["sandbox"], "workspace-write");
        assert_eq!(params["approvalsReviewer"], "auto_review");
    }

    #[test]
    fn current_mode_distinguishes_automatic_and_manual_review() {
        for (reviewer, expected) in [(Some("auto_review"), "approve-for-me"), (Some("user"), "default"), (None, "default")] {
            let response = json!({
                "approvalPolicy": "on-request",
                "sandbox": { "type": "workspaceWrite" },
                "approvalsReviewer": reviewer,
            });
            assert_eq!(current(&response).chosen.mode.as_deref(), Some(expected));
        }
    }

    #[test]
    fn mode_changes_update_the_reviewer_and_plan_preserves_it() {
        let (events, _received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new("thread", Path::new("/project"), events);
        transcript.storage_roots = vec!["/storage/scratch".to_owned(), "/project/target".to_owned()];
        for (mode, approval, sandbox, reviewer) in [
            ("approve-for-me", "on-request", "workspaceWrite", "auto_review"),
            ("default", "on-request", "workspaceWrite", "user"),
            ("approve-for-me", "on-request", "workspaceWrite", "auto_review"),
            ("read-only", "on-request", "readOnly", "user"),
            ("approve-for-me", "on-request", "workspaceWrite", "auto_review"),
            ("full-access", "never", "dangerFullAccess", "user"),
        ] {
            transcript.choose(Setting::Mode, mode.to_owned());
            let mut turn = json!({});
            transcript.overrides(&mut turn);
            assert_eq!(turn["approvalPolicy"], approval);
            assert_eq!(turn["sandboxPolicy"]["type"], sandbox);
            if sandbox == "workspaceWrite" {
                assert_eq!(turn["sandboxPolicy"]["writableRoots"], json!(["/storage/scratch", "/project/target"]));
            } else {
                assert!(turn["sandboxPolicy"].get("writableRoots").is_none());
            }
            assert_eq!(turn["approvalsReviewer"], reviewer);
        }
        transcript.choose(Setting::Mode, "plan".to_owned());
        let mut turn = json!({});
        transcript.overrides(&mut turn);
        assert_eq!(turn["collaborationMode"]["mode"], "plan");
        assert!(turn.get("approvalsReviewer").is_none());
    }
}
