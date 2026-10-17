use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File},
    io::{self, BufRead, Read},
    path::{Path, PathBuf},
    process::Stdio,
    time::UNIX_EPOCH,
};

use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::unix::OwnedWriteHalf,
    process,
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use toml::value::{Datetime, Offset};
use uuid::Uuid;

use super::{
    AgentCommand, Answer, Builtin, Capabilities, Chat, Choice, CommandStatus, Context, Driver, Event, Item, Notice, Outcome, Phase, Setting,
    Settings, Session, Subagent, Todo, TodoStatus, Tokens, ToolCall, ToolKind,
    holder::{self, Frames},
    html,
    protocol::{self, Message},
    Request, Image, Scope, AnsweredQuestions, Question, diff, seconds, unified, words,
};
use crate::{
    config::{Agent, AgentSettings},
    chats::Status,
    usage::{Reading, Window},
};

const ARGS: [&str; 7] = [
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-partial-messages",
    "--permission-prompt-tool=stdio",
];

const SDK_ENV: [(&str, &str); 2] = [("CLAUDE_CODE_ENTRYPOINT", "sdk-ts"), ("CLAUDE_CODE_SDK_READS_SESSION_STATE", "1")];

const WINDOWS: [(Window, &str); 2] = [(Window::FiveHour, "five_hour"), (Window::Weekly, "seven_day")];

const HEAD: u64 = 1 << 20;
const DENIED: &str = "The user denied this request.";

const MANAGEMENT: &[&str] = &[
    "mcp", "hooks", "memory", "permissions", "plugin", "plugins", "statusline", "theme", "export", "add-dir",
    "agents", "login", "logout", "config", "heapdump",
];

pub struct Claude {
    commands: UnboundedSender<Command>,
}

enum Command {
    Send(String, Vec<Image>),
    Retry,
    Interrupt,
    Answer(String, Answer),
    Run(Builtin),
    Close,
}

impl Driver for Claude {
    fn send(&self, text: &str, images: &[Image]) {
        let _ = self.commands.send(Command::Send(text.to_owned(), images.to_vec()));
    }

    fn retry(&self) {
        let _ = self.commands.send(Command::Retry);
    }

    fn interrupt(&self) {
        let _ = self.commands.send(Command::Interrupt);
    }

    fn answer(&self, request_id: &str, answer: Answer) {
        let _ = self.commands.send(Command::Answer(request_id.to_owned(), answer));
    }

    fn run(&self, builtin: Builtin) {
        let _ = self.commands.send(Command::Run(builtin));
    }

    fn close(&self) {
        let _ = self.commands.send(Command::Close);
    }
}

pub async fn start(
    data_dir: &Path,
    id: &str,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    settings: &AgentSettings,
    session: Option<&str>,
) -> io::Result<(Claude, UnboundedReceiver<Event>)> {
    let (frames, writer, transcript, received) = launch(data_dir, id, cwd, env, settings, session, None, None).await?;
    Ok((drive(frames, writer, transcript), received))
}

pub async fn resume(
    data_dir: &Path,
    chat: &Chat,
    env: &BTreeMap<String, String>,
) -> io::Result<(Claude, UnboundedReceiver<Event>)> {
    let _ = fs::remove_file(holder::socket_path(data_dir, &chat.id));
    let session = chat.agent_session.as_deref();
    let (frames, writer, mut transcript, received) =
        launch(data_dir, &chat.id, &chat.cwd, env, &chat.settings.chosen, session, chat.resume_at.as_deref(), None).await?;
    transcript.window = chat.context.map(|context| context.window);
    if chat.status != Status::Done {
        if chat.status == Status::Working { let _ = transcript.events.send(Event::Recoverable); }
        transcript.push(Item::interrupted());
    }
    let _ = transcript.events.send(Event::Status(Status::Done));
    Ok((drive(frames, writer, transcript), received))
}

async fn launch(
    data_dir: &Path,
    id: &str,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    settings: &AgentSettings,
    session: Option<&str>,
    at: Option<&str>,
    sender: Option<UnboundedSender<Event>>,
) -> io::Result<(Frames, OwnedWriteHalf, Transcript, UnboundedReceiver<Event>)> {
    let variables = [("CLAUDE_CODE_ENABLE_TODO_TOOLS", "1")].into_iter()
        .chain(env.iter().map(|(name, value)| (name.as_str(), value.as_str())))
        .chain(SDK_ENV)
        .chain([("CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING", "true")]);
    let tools = ["--mcp-config".to_owned(), json!({ "mcpServers": { html::SERVER: html::config() } }).to_string(),
        "--allowedTools".to_owned(), html::CLAUDE_TOOL.to_owned()];
    let command = ["claude"].iter().chain(&ARGS).map(|arg| arg.to_string()).chain(settings.claude_flags()).chain(tools);
    let resume = session.into_iter().flat_map(|session| ["--resume".to_owned(), session.to_owned()]);
    let rewind = at.into_iter().flat_map(|at| ["--resume-session-at".to_owned(), at.to_owned()]);
    holder::spawn(data_dir, id, cwd, variables, command.chain(resume).chain(rewind)).await?;
    let (frames, mut writer, _) = holder::connect(data_dir, id).await?;
    let (events, received) = mpsc::unbounded_channel();
    let mut transcript = Transcript::new(sender.unwrap_or(events), cwd);
    transcript.location = Some(Location { data_dir: data_dir.to_owned(), id: id.to_owned(), env: env.clone() });
    transcript.session = session.map(str::to_owned);
    if let Some(session) = session {
        transcript.history(env, session, at);
    }
    transcript.settings.chosen = settings.clone();
    transcript.discover(&mut writer).await?;
    Ok((frames, writer, transcript, received))
}

/// Rebuilds the transcript from the session file up to the holder's latest turn, then from the turn the holder replays.
pub async fn reattach(
    data_dir: &Path,
    chat: &Chat,
    env: &BTreeMap<String, String>,
) -> io::Result<(Claude, UnboundedReceiver<Event>)> {
    let (mut frames, mut writer, buffered) = holder::connect(data_dir, &chat.id).await?;
    let mut replay = Vec::with_capacity(buffered);
    for _ in 0..buffered {
        let frame = frames.next().await.ok_or(io::ErrorKind::UnexpectedEof)??;
        replay.extend(protocol::decode(&frame)?);
    }
    let turn = replay.iter().find_map(|message| match message {
        Message::Input(line) => user(line)?["uuid"].as_str().map(str::to_owned),
        _ => None,
    });
    let (events, received) = mpsc::unbounded_channel();
    let mut transcript = Transcript::new(events, &chat.cwd);
    transcript.location = Some(Location { data_dir: data_dir.to_owned(), id: chat.id.clone(), env: env.clone() });
    transcript.session = chat.agent_session.clone();
    transcript.replaying = true;
    transcript.window = chat.context.map(|context| context.window);
    transcript.settings.chosen = chat.settings.chosen.clone();
    if let Some(session) = &chat.agent_session {
        transcript.history(env, session, turn.as_deref().or(chat.resume_at.as_deref()));
    }
    for message in replay {
        match message {
            Message::Line(line) => {
                if let Ok(message) = serde_json::from_slice(&line) {
                    transcript.read(&message);
                }
            }
            Message::Input(line) => {
                let Ok(message) = serde_json::from_slice::<Value>(&line) else { continue };
                match message["type"].as_str() {
                    Some("user") => transcript.sent(&message),
                    Some("control_response") => {
                        transcript.resolve(message["response"]["request_id"].as_str().unwrap_or_default())
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    transcript.replaying = false;
    for request in std::mem::take(&mut transcript.pending) {
        let _ = transcript.events.send(Event::Request(request));
    }
    let _ = transcript.events.send(Event::Status(transcript.status));
    if transcript.todos_held {
        transcript.report_todos();
    }
    transcript.discover(&mut writer).await?;
    Ok((drive(frames, writer, transcript), received))
}

fn drive(mut frames: Frames, mut writer: OwnedWriteHalf, mut transcript: Transcript) -> Claude {
    let (commands, mut pending) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                command = pending.recv() => match command {
                    Some(Command::Send(text, images)) => {
                        if send(&mut writer, &mut transcript, &text, &images).await.is_err() {
                            break;
                        }
                    }
                    Some(Command::Retry) => {
                        if let Some(content) = transcript.last_input.clone() {
                            let message = json!({ "type": "user", "message": { "role": "user", "content": content },
                                "parent_tool_use_id": null, "uuid": Uuid::new_v4().to_string() });
                            transcript.sent(&message);
                            if writer.write_all(&protocol::encode(&Message::Forget)).await.is_err()
                                || writer.write_all(&input(&message)).await.is_err() { break; }
                        }
                    }
                    Some(Command::Run(Builtin::Rewind(index))) => {
                        match rewind(&mut frames, &mut writer, &mut transcript, index).await {
                            Ok(Some(restarted)) => (frames, writer, transcript) = restarted,
                            Ok(None) => {},
                            Err(error) => {
                                transcript.push(Item::Notice { notice: Notice::Error, text: format!("Could not rewind: {error}") });
                            }
                        }
                    }
                    Some(Command::Run(builtin)) => {
                        if run(&mut writer, &mut transcript, builtin).await.is_err() {
                            break;
                        }
                    }
                    Some(Command::Interrupt) => {
                        let interrupt = json!({
                            "type": "control_request",
                            "request_id": Uuid::new_v4().to_string(),
                            "request": { "subtype": "interrupt", "cancel_queued": true },
                        });
                        if transcript.status != Status::Done && writer.write_all(&input(&interrupt)).await.is_err() {
                            break;
                        }
                    }
                    Some(Command::Answer(id, answer)) => {
                        let original = transcript.inputs.remove(&id).unwrap_or_default();
                        let response = match answer {
                            Answer::Allow => json!({ "behavior": "allow" }),
                            Answer::Rules(rules) => {
                                let parts = words(original["input"]["command"].as_str().unwrap_or_default());
                                let updates = rules.iter().zip(parts).map(|(rule, part)| {
                                    let command = shlex::try_join(rule.prefix.iter().map(String::as_str)).unwrap_or_default();
                                    let content = if rule.prefix == part { command } else { format!("{command}:*") };
                                    json!({ "type": "addRules", "rules": [{ "toolName": "Bash", "ruleContent": content }],
                                        "behavior": "allow", "destination": if rule.scope == Scope::Chat { "session" } else { "localSettings" } })
                                }).collect::<Vec<_>>();
                                json!({ "behavior": "allow", "updatedPermissions": updates })
                            }
                            Answer::Answers(answers) => {
                                let mut updated = original["input"].clone();
                                let mut mapped = serde_json::Map::new();
                                for (question, answers) in original["input"]["questions"].as_array().into_iter().flatten().zip(answers) {
                                    if let Some(question) = question["question"].as_str() {
                                        mapped.insert(question.to_owned(), json!(answers.join(", ")));
                                    }
                                }
                                updated["answers"] = json!(mapped);
                                json!({ "behavior": "allow", "updatedInput": updated })
                            }
                            Answer::Deny { instead: Some(instead) } if original["tool_name"] == "AskUserQuestion" => {
                                let mut updated = original["input"].clone();
                                let answers = original["input"]["questions"].as_array().into_iter().flatten()
                                    .filter_map(|q| q["question"].as_str()).map(|q| (q.to_owned(), json!(instead))).collect::<serde_json::Map<_, _>>();
                                updated["answers"] = json!(answers);
                                json!({ "behavior": "allow", "updatedInput": updated })
                            }
                            Answer::Deny { instead: Some(instead) } => json!({ "behavior": "deny", "message": instead }),
                            Answer::Deny { instead: None } => json!({ "behavior": "deny", "message": DENIED, "interrupt": true }),
                        };
                        let response = json!({
                            "type": "control_response",
                            "response": { "subtype": "success", "request_id": id, "response": response },
                        });
                        if writer.write_all(&input(&response)).await.is_err() {
                            break;
                        }
                    }
                    Some(Command::Close) => {
                        let _ = writer.write_all(&protocol::encode(&Message::Hangup)).await;
                    }
                    None => break,
                },
                frame = frames.next() => {
                    let Some(Ok(frame)) = frame else { break };
                    if let Ok(Some(Message::Line(line))) = protocol::decode(&frame)
                        && let Ok(message) = serde_json::from_slice(&line)
                    {
                        transcript.read(&message);
                    }
                }
            }
        }
    });
    Claude { commands }
}

async fn rewind(
    frames: &mut Frames,
    writer: &mut OwnedWriteHalf,
    transcript: &mut Transcript,
    index: usize,
) -> io::Result<Option<(Frames, OwnedWriteHalf, Transcript)>> {
    if transcript.status != Status::Done { return Ok(None); }
    let Some(checkpoint) = transcript.checkpoints.get(&index).cloned() else { return Ok(None) };
    let Some(location) = transcript.location.clone() else { return Ok(None) };
    let Some(session) = transcript.session.clone() else { return Ok(None) };
    let id = Uuid::new_v4().to_string();
    let request = json!({ "type": "control_request", "request_id": id,
        "request": { "subtype": "rewind_files", "user_message_id": checkpoint } });
    writer.write_all(&input(&request)).await?;
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let frame = frames.next().await.ok_or(io::ErrorKind::UnexpectedEof)??;
            if let Some(Message::Line(line)) = protocol::decode(&frame)? {
                let message: Value = serde_json::from_slice(&line)?;
                if message["type"] == "control_response" && message["response"]["request_id"] == id {
                    let response = &message["response"];
                    if response["subtype"] != "success" || response["response"]["canRewind"] == false {
                        return Err(io::Error::other(format!("{}", response.get("error").unwrap_or(&response["response"]))));
                    }
                    return Ok(());
                }
                transcript.read(&message);
            }
        }
    }).await.map_err(io::Error::other)??;
    writer.write_all(&protocol::encode(&Message::Hangup)).await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), async { while frames.next().await.is_some() {} }).await.map_err(io::Error::other)?;
    let _ = fs::remove_file(holder::socket_path(&location.data_dir, &location.id));
    let events = transcript.events.clone();
    let _ = events.send(Event::QuestionsTruncated(index));
    let _ = events.send(Event::Truncate(0));
    let _ = events.send(Event::Reset);
    let (frames, writer, mut fresh, _) = launch(&location.data_dir, &location.id, &transcript.cwd,
        &location.env, &transcript.settings.chosen, Some(&session), Some(&checkpoint), Some(events)).await?;
    let _ = fresh.events.send(Event::ResumeAt(Some(checkpoint)));
    fresh.push(Item::Notice { notice: Notice::Rewound, text: "Conversation rewound; tracked edits restored. Bash and subagent changes are not restored.".to_owned() });
    Ok(Some((frames, writer, fresh)))
}

/// Asks a fresh `claude` for the plan's windows with the `get_usage` control request, which makes no model call.
pub async fn usage(env: &BTreeMap<String, String>) -> io::Result<Vec<Reading>> {
    let mut claude = process::Command::new("claude")
        .args(ARGS)
        .envs(env)
        .envs(SDK_ENV)
        .current_dir(crate::home())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = claude.stdin.take().unwrap();
    let id = Uuid::new_v4().to_string();
    let request = json!({
        "type": "control_request",
        "request_id": id,
        "request": { "subtype": "get_usage", "skip_behaviors": true },
    });
    stdin.write_all(format!("{request}\n").as_bytes()).await?;
    let mut lines = BufReader::new(claude.stdout.take().unwrap()).lines();
    while let Some(line) = lines.next_line().await? {
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        let response = &message["response"];
        if message["type"] != "control_response" || response["request_id"] != id {
            continue;
        }
        drop(stdin);
        claude.wait().await?;
        if response["subtype"] != "success" {
            return Err(io::Error::other(format!("get_usage: {}", response["error"])));
        }
        let limits = &response["response"]["rate_limits"];
        let readings = WINDOWS.into_iter().filter_map(|(window, name)| {
            let used = limits[name]["utilization"].as_f64()? / 100.0;
            Some(Reading::new(Agent::ClaudeCode, window, used, limits[name]["resets_at"].as_str().and_then(epoch_ms)))
        });
        return Ok(readings.collect());
    }
    Err(io::Error::other("claude ended without answering get_usage"))
}

fn epoch_ms(time: &str) -> Option<u64> {
    let time: Datetime = time.parse().ok()?;
    let (date, clock) = (time.date?, time.time?);
    let offset = match time.offset? {
        Offset::Z => 0,
        Offset::Custom { minutes } => i64::from(minutes),
    };
    let (month, day) = (i64::from(date.month), i64::from(date.day));
    let year = i64::from(date.year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let days = era * 146_097 + of_era * 365 + of_era / 4 - of_era / 100 + of_year - 719_468;
    let minutes = (days * 24 + i64::from(clock.hour)) * 60 + i64::from(clock.minute) - offset;
    let seconds = minutes * 60 + i64::from(clock.second.unwrap_or(0));
    u64::try_from(seconds * 1000 + i64::from(clock.nanosecond.unwrap_or(0) / 1_000_000)).ok()
}

async fn send(writer: &mut OwnedWriteHalf, transcript: &mut Transcript, text: &str, images: &[Image]) -> io::Result<()> {
    let mut message = json!({
        "type": "user",
        "message": { "role": "user", "content": text },
        "parent_tool_use_id": null,
        "uuid": Uuid::new_v4().to_string(),
    });
    if !images.is_empty() {
        let mut content = vec![json!({ "type": "text", "text": text })];
        content.extend(images.iter().map(|image| json!({ "type": "image", "source": {
            "type": "base64", "media_type": image.media_type, "data": image.data
        } })));
        message["message"]["content"] = json!(content);
    }
    let idle = transcript.status == Status::Done;
    if !idle {
        message["priority"] = json!("next");
    }
    transcript.sent(&message);
    if idle {
        writer.write_all(&protocol::encode(&Message::Forget)).await?;
    }
    writer.write_all(&input(&message)).await
}

async fn run(writer: &mut OwnedWriteHalf, transcript: &mut Transcript, builtin: Builtin) -> io::Result<()> {
    let (setting, request, value) = match builtin {
        Builtin::Compact => return send(writer, transcript, "/compact", &[]).await,
        Builtin::Clear => return send(writer, transcript, "/clear", &[]).await,
        Builtin::Rewind(_) => return Ok(()),
        Builtin::Model(model) => (Setting::Model, json!({ "subtype": "set_model", "model": model }), model),
        Builtin::Effort(effort) => {
            let request = json!({ "subtype": "apply_flag_settings", "settings": { "effortLevel": effort } });
            (Setting::Effort, request, effort)
        }
        Builtin::Mode(mode) => (Setting::Mode, json!({ "subtype": "set_permission_mode", "mode": mode }), mode),
    };
    *transcript.settings.get_mut(setting) = Some(value.clone());
    transcript.settings.mark_pending(setting);
    transcript.changed(setting);
    let request = transcript.request(request, Asked::Set(setting, value));
    writer.write_all(&request).await
}

fn user(line: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(line).ok().filter(|message| message["type"] == "user")
}

/// The sessions Claude keeps for `cwd`, each with the first prompt found in its file's head.
pub fn sessions(cwd: &Path, env: &BTreeMap<String, String>) -> io::Result<Vec<Session>> {
    let Some(config) = config_dir(env) else { return Ok(Vec::new()) };
    let name: String = cwd.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let entries = match fs::read_dir(config.join("projects").join(name)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        entries => entries?,
    };
    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension() != Some("jsonl".as_ref()) {
            continue;
        }
        let (Some(id), Some(prompt)) = (path.file_stem().and_then(|stem| stem.to_str()), first_prompt(&path)) else {
            continue;
        };
        let Some(updated) = entry.metadata().and_then(|metadata| metadata.modified()).ok() else { continue };
        let updated = updated.duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_millis() as u64;
        sessions.push(Session::new(Agent::ClaudeCode, id, &prompt, updated));
    }
    Ok(sessions)
}

fn first_prompt(path: &Path) -> Option<String> {
    let head = std::io::BufReader::new(File::open(path).ok()?.take(HEAD));
    let entries = head.lines().map_while(Result::ok).filter_map(|line| serde_json::from_str::<Value>(&line).ok());
    entries.filter(|entry| entry["type"] == "user" && entry["isMeta"] != true && entry["isSidechain"] != true).find_map(|entry| {
        let text = match &entry["message"]["content"] {
            Value::String(text) => text.clone(),
            Value::Array(blocks) => blocks.iter().filter_map(|block| block["text"].as_str()).collect::<Vec<_>>().join("\n"),
            _ => return None,
        };
        if text.starts_with("<command-message>") {
            let args = tagged(&text, "command-args").unwrap_or_default();
            return Some(format!("{} {args}", tagged(&text, "command-name")?).trim_end().to_owned());
        }
        // Local commands, their output, and notifications all arrive as user text wrapped in a tag.
        (!text.is_empty() && !text.starts_with('<')).then_some(text)
    })
}

fn tagged<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let start = text.find(&format!("<{tag}>"))? + tag.len() + 2;
    let end = start + text[start..].find(&format!("</{tag}>"))?;
    Some(&text[start..end])
}

fn session_file(env: &BTreeMap<String, String>, session: &str) -> Option<PathBuf> {
    let name = format!("{session}.jsonl");
    let projects = fs::read_dir(config_dir(env)?.join("projects")).ok()?;
    projects.flatten().map(|project| project.path().join(&name)).find(|path| path.is_file())
}

fn config_dir(env: &BTreeMap<String, String>) -> Option<PathBuf> {
    match env.get("CLAUDE_CONFIG_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|| std::env::home_dir().map(|home| home.join(".claude"))),
    }
}

fn request(id: &str, request: &Value) -> Request {
    let id = id.to_owned();
    let input = &request["input"];
    let text = |field: &str| input[field].as_str().unwrap_or_default().to_owned();
    match request["tool_name"].as_str() {
        Some("Bash") => {
            let parts = words(&text("command"));
            let rules = request["permission_suggestions"].as_array().into_iter().flatten()
                .flat_map(|suggestion| suggestion["rules"].as_array().into_iter().flatten())
                .filter(|rule| rule["toolName"] == "Bash")
                .filter_map(|rule| rule["ruleContent"].as_str())
                .flat_map(|rule| words(rule.trim_end_matches(":*"))).collect::<Vec<_>>();
            let suggestion = parts.iter().map(|part| rules.iter().find(|rule| part.starts_with(rule)).unwrap_or(part).clone()).collect();
            Request::Command { id, words: parts, suggestion, session_prefixes: true }
        }
        Some("AskUserQuestion") => Request::Question {
            id, questions: serde_json::from_value(input["questions"].clone()).unwrap_or_default(),
        },
        Some("ExitPlanMode") => Request::Plan { id, plan: text("plan") },
        Some("Edit") => {
            let diff = diff(&text("old_string"), &text("new_string"));
            Request::Edit { id, path: text("file_path"), diff }
        }
        Some("Write") => Request::Edit { id, path: text("file_path"), diff: diff("", &text("content")) },
        tool => {
            let name = request["display_name"].as_str().or(tool).unwrap_or("a tool");
            let title = request["title"].as_str().map_or_else(|| format!("Claude wants to use {name}"), str::to_owned);
            Request::Other { id, title, detail: serde_json::to_string_pretty(input).unwrap_or_default() }
        }
    }
}

fn recorded_answers(question: &Question, text: &str) -> Vec<String> {
    if text.is_empty() { return Vec::new(); }
    if !question.multi_select { return vec![text.to_owned()]; }
    let mut remaining = text;
    let mut answers = Vec::new();
    while !remaining.is_empty() {
        let option = question.options.iter().filter(|option| {
            !option.label.is_empty() && (remaining == option.label || remaining.strip_prefix(&option.label).is_some_and(|tail| tail.starts_with(", ")))
        }).max_by_key(|option| option.label.len());
        let Some(option) = option else {
            answers.push(remaining.to_owned());
            break;
        };
        answers.push(option.label.clone());
        remaining = remaining[option.label.len()..].strip_prefix(", ").unwrap_or_default();
    }
    answers
}

fn input(message: &Value) -> Vec<u8> {
    protocol::encode(&Message::Input(message.to_string().into_bytes()))
}

fn interrupted(content: &Value) -> bool {
    let mut blocks = content.as_array().into_iter().flatten();
    blocks.any(|block| block["text"].as_str().is_some_and(|text| text.starts_with("[Request interrupted by user")))
}

#[derive(Clone)]
struct Location {
    data_dir: PathBuf,
    id: String,
    env: BTreeMap<String, String>,
}

struct Transcript {
    last_input: Option<Value>,
    location: Option<Location>,
    session: Option<String>,
    checkpoints: HashMap<usize, String>,
    events: UnboundedSender<Event>,
    cwd: PathBuf,
    len: usize,
    streaming: HashMap<u64, (usize, String)>,
    message: Option<String>,
    phase: Phase,
    said: Vec<(usize, String)>,
    tools: Option<(usize, Vec<ToolCall>)>,
    activity: super::Activity,
    commands: HashMap<String, (usize, String)>,
    snippets: HashMap<String, Item>,
    edits: HashMap<String, bool>,
    subagents: HashMap<String, (Subagent, Transcript)>,
    todos: Vec<(String, Todo)>,
    creating: HashSet<String>,
    tasks: HashMap<String, String>,
    todos_held: bool,
    history: Option<PathBuf>,
    compacted: bool,
    status: Status,
    replaying: bool,
    pending: Vec<Request>,
    inputs: HashMap<String, Value>,
    /// The model of the main loop's latest API call, and how many tokens it was sent.
    call: Option<(String, u64)>,
    window: Option<u64>,
    steering: HashSet<String>,
    asked: HashMap<String, Asked>,
    models: Vec<Value>,
    slash_commands: Vec<AgentCommand>,
    settings: Settings,
    applied: AgentSettings,
}

enum Asked {
    Initialize,
    Settings,
    Set(Setting, String),
}

impl Transcript {
    fn new(events: UnboundedSender<Event>, cwd: &Path) -> Self {
        Self {
            last_input: None,
            location: None,
            session: None,
            checkpoints: HashMap::new(),
            events,
            cwd: cwd.to_owned(),
            len: 0,
            streaming: HashMap::new(),
            message: None,
            phase: Phase::Final,
            said: Vec::new(),
            tools: None,
            activity: super::Activity::default(),
            commands: HashMap::new(),
            snippets: HashMap::new(),
            edits: HashMap::new(),
            subagents: HashMap::new(),
            todos: Vec::new(),
            creating: HashSet::new(),
            tasks: HashMap::new(),
            todos_held: false,
            history: None,
            compacted: false,
            status: Status::Done,
            replaying: false,
            pending: Vec::new(),
            inputs: HashMap::new(),
            call: None,
            window: None,
            steering: HashSet::new(),
            asked: HashMap::new(),
            models: Vec::new(),
            slash_commands: Vec::new(),
            settings: Settings::default(),
            applied: AgentSettings::default(),
        }
    }

    /// Asks for the commands, models and settings, which a restarted server has to ask for again.
    async fn discover(&mut self, writer: &mut OwnedWriteHalf) -> io::Result<()> {
        writer.write_all(&self.request(json!({ "subtype": "initialize" }), Asked::Initialize)).await?;
        writer.write_all(&self.request(json!({ "subtype": "get_settings" }), Asked::Settings)).await
    }

    fn request(&mut self, request: Value, asked: Asked) -> Vec<u8> {
        let id = Uuid::new_v4().to_string();
        let message = json!({ "type": "control_request", "request_id": id, "request": request });
        self.asked.insert(id, asked);
        input(&message)
    }

    fn answered(&mut self, response: &Value) {
        let Some(asked) = response["request_id"].as_str().and_then(|id| self.asked.remove(id)) else { return };
        let success = response["subtype"] == "success";
        let body = &response["response"];
        match asked {
            Asked::Initialize if success => {
                self.models = body["models"].as_array().cloned().unwrap_or_default();
                let commands = body["commands"].as_array().into_iter().flatten().filter(|command| {
                    let name = command["name"].as_str().unwrap_or_default();
                    !name.starts_with("__") && !MANAGEMENT.contains(&name)
                });
                self.slash_commands = commands
                    .map(|command| AgentCommand {
                        name: command["name"].as_str().unwrap_or_default().to_owned(),
                        description: command["description"].as_str().unwrap_or_default().to_owned(),
                        hint: command["argumentHint"].as_str().unwrap_or_default().to_owned(),
                    })
                    .collect();
                let model = self.settings.chosen.model.take().map(|model| self.alias(&model));
                self.settings.chosen.model = model;
                if let Some(mode) = body["current_permission_mode"].as_str() {
                    self.settings.chosen.mode = Some(mode.to_owned());
                }
                self.declare();
                self.report_settings();
            }
            Asked::Settings if success => {
                let applied = &body["applied"];
                self.settings.chosen.model = applied["model"].as_str().map(|model| self.alias(model));
                self.settings.chosen.effort = applied["effort"].as_str().map(str::to_owned);
                self.applied = self.settings.chosen.clone();
                self.declare();
                self.report_settings();
            }
            Asked::Set(setting, value) => {
                if success {
                    let applied = match setting {
                        Setting::Model => &mut self.applied.model,
                        Setting::Effort => &mut self.applied.effort,
                        Setting::Mode => &mut self.applied.mode,
                    };
                    *applied = Some(value.clone());
                }
                let waiting = self.asked.values().any(|asked| matches!(asked, Asked::Set(other, _) if *other == setting));
                if !waiting && self.settings.get(setting).as_deref() == Some(value.as_str()) {
                    self.settings.pending.retain(|pending| *pending != setting);
                    if !success {
                        let applied = match setting {
                            Setting::Model => &self.applied.model,
                            Setting::Effort => &self.applied.effort,
                            Setting::Mode => &self.applied.mode,
                        };
                        *self.settings.get_mut(setting) = applied.clone();
                    }
                    self.changed(setting);
                }
            }
            _ => {}
        }
    }

    fn alias(&self, model: &str) -> String {
        let listed = self.models.iter().find(|listed| listed["value"] == model);
        let resolved = || {
            let mut aliases = self.models.iter().filter(|listed| listed["value"] != "default");
            aliases.find(|listed| listed["resolvedModel"] == model)
        };
        listed.or_else(resolved).and_then(|listed| listed["value"].as_str()).unwrap_or(model).to_owned()
    }

    fn changed(&self, setting: Setting) {
        if setting == Setting::Model {
            self.declare();
        }
        self.report_settings();
    }

    fn declare(&self) {
        let model = self.settings.chosen.model.as_deref().unwrap_or("default");
        let listed = self.models.iter().find(|listed| listed["value"] == model);
        let efforts = listed.and_then(|listed| listed["supportedEffortLevels"].as_array()).into_iter().flatten();
        let capabilities = Capabilities {
            rewinds_files: true,
            models: self.models.iter().filter_map(|model| model["value"].as_str()).map(str::to_owned).collect(),
            efforts: efforts.filter_map(Value::as_str).map(str::to_owned).collect(),
            modes: Agent::ClaudeCode.modes().iter().map(|mode| Choice::mode(mode)).collect(),
            commands: self.slash_commands.clone(),
        };
        let _ = self.events.send(Event::Capabilities(capabilities));
    }

    fn report_settings(&self) {
        let _ = self.events.send(Event::Settings(self.settings.clone()));
    }

    fn observe(&mut self, mode: &str) {
        if !self.settings.pending.contains(&Setting::Mode) && self.settings.chosen.mode.as_deref() != Some(mode) {
            self.settings.chosen.mode = Some(mode.to_owned());
            self.applied.mode = Some(mode.to_owned());
            self.report_settings();
        }
    }

    fn push(&mut self, item: Item) -> usize {
        let index = self.len;
        self.len += 1;
        self.tools = None;
        let _ = self.events.send(Event::Item(index, item));
        index
    }

    fn report(&mut self, status: Status) {
        if status == Status::Done { self.activity.clear(&self.events); }
        self.status = status;
        if !self.replaying {
            let _ = self.events.send(Event::Status(status));
        }
    }

    fn ask(&mut self, request: Request) {
        if self.replaying {
            self.pending.push(request);
        } else {
            let _ = self.events.send(Event::Request(request));
        }
    }

    fn resolve(&mut self, id: &str) {
        if self.replaying {
            self.pending.retain(|request| request.id() != id);
        } else {
            let _ = self.events.send(Event::Resolved(id.to_owned()));
        }
    }

    fn report_todos(&mut self) {
        self.todos_held = self.replaying;
        if !self.replaying {
            let todos = self.todos.iter().map(|(_, todo)| todo.clone()).collect();
            let _ = self.events.send(Event::Todos(todos));
        }
    }

    fn checkpoint(&mut self, message: &Value) {
        if let Some(id) = message["uuid"].as_str() { self.checkpoints.insert(self.len, id.to_owned()); }
    }

    fn sent(&mut self, message: &Value) {
        let content = &message["message"]["content"];
        if let Some(text) = content.as_str().map(str::to_owned).or_else(|| content.as_array().map(|blocks| {
            blocks.iter().filter_map(|block| block["text"].as_str()).collect::<Vec<_>>().join("\n")
        })) {
            self.last_input = Some(content.clone());
            let images = content.as_array().into_iter().flatten().filter(|block| block["type"] == "image").count();
            let _ = self.events.send(Event::ResumeAt(None));
            self.checkpoint(message);
            self.push(Item::User { text, images, template: None });
            if message.get("priority").is_some()
                && let Some(uuid) = message["uuid"].as_str()
            {
                self.steering.insert(uuid.to_owned());
            }
            self.report(Status::Working);
        }
    }

    /// Translates the session file's entries, up to the one whose uuid is `until`.
    fn history(&mut self, env: &BTreeMap<String, String>, session: &str, until: Option<&str>) {
        let Some(path) = session_file(env, session) else { return };
        let Ok(contents) = fs::read_to_string(&path) else { return };
        self.history = Some(path);
        let entries: Vec<Value> = contents.lines().filter_map(|line| serde_json::from_str(line).ok()).collect();
        let chained: HashMap<&str, &Value> = entries.iter().filter(|entry| entry["isSidechain"] != true).filter_map(|entry| Some((entry["uuid"].as_str()?, entry))).collect();
        let leaf = entries.iter().rev().find(|entry| {
            matches!(entry["type"].as_str(), Some("user" | "assistant")) && entry["isSidechain"] != true && entry.get("parentUuid").is_some()
        });
        let mut branch = HashSet::new();
        let mut current = leaf.and_then(|entry| entry["uuid"].as_str());
        let mut complete = false;
        while let Some(id) = current {
            if !branch.insert(id) { break; }
            let Some(entry) = chained.get(id) else { break };
            current = entry["parentUuid"].as_str();
            if entry.get("parentUuid") == Some(&Value::Null) { complete = true; }
        }
        let boundary = entries.iter().rposition(|entry| entry["type"] == "system" && entry["subtype"] == "compact_boundary");
        let active_tools: HashSet<&str> = entries.iter().filter(|entry| entry["uuid"].as_str().is_some_and(|id| branch.contains(id)))
            .flat_map(|entry| entry["message"]["content"].as_array().into_iter().flatten())
            .filter(|block| block["type"] == "tool_use").filter_map(|block| block["id"].as_str()).collect();
        for (index, entry) in entries.iter().enumerate() {
            if complete && boundary.is_none_or(|boundary| index > boundary) && !branch.is_empty() && matches!(entry["type"].as_str(), Some("user" | "assistant"))
                && entry["uuid"].as_str().is_some_and(|id| !branch.contains(id))
                && !entry["message"]["content"].as_array().into_iter().flatten().any(|block| {
                    block["type"] == "tool_result" && block["tool_use_id"].as_str().is_some_and(|id| active_tools.contains(id))
                }) { continue; }
            if until.is_some() && entry["uuid"].as_str() == until {
                // A compaction recorded just before the replayed turn's prompt belongs to that turn, which replays it.
                self.compacted = false;
                break;
            }
            self.recorded(&entry);
        }
        self.history = None;
        self.compact();
    }

    /// A compaction is recorded before the prompt that asked for it, so its notice waits for that prompt.
    fn recorded(&mut self, entry: &Value) {
        self.task_notification(entry);
        if entry["isSidechain"] == true || entry["isMeta"] == true || entry["isCompactSummary"] == true {
            return;
        }
        match entry["type"].as_str() {
            Some("system") if entry["subtype"] == "compact_boundary" => self.compacted = true,
            Some("user") if entry["promptSource"] != "system" => match entry["message"]["content"].as_str() {
                Some(content) => {
                    if let Some(text) = prompt(content) {
                        self.last_input = Some(entry["message"]["content"].clone());
                        self.checkpoint(entry);
                        self.push(Item::User { text, images: 0, template: None });
                        self.compact();
                    }
                }
                None => {
                    let blocks = entry["message"]["content"].as_array().into_iter().flatten();
                    let text = blocks.clone().filter_map(|block| block["text"].as_str()).collect::<Vec<_>>().join("\n");
                    let images = blocks.filter(|block| block["type"] == "image").count();
                    if !text.is_empty() || images > 0 { self.last_input = Some(entry["message"]["content"].clone()); self.checkpoint(entry); self.push(Item::User { text, images, template: None }); }
                    self.compact();
                    self.results(&entry["message"]["content"], &entry["toolUseResult"]);
                }
            },
            Some("attachment") if entry["attachment"]["type"] == "queued_command" && entry["renderedRole"] != "system" => {
                if let Some(text) = entry["attachment"]["prompt"].as_str() {
                    self.push(Item::User { text: text.to_owned(), images: 0, template: None });
                }
            }
            Some("assistant") => {
                self.compact();
                self.assistant(entry, &entry["thinkingDurationMs"], false);
            }
            _ => {}
        }
    }

    fn compact(&mut self) {
        if std::mem::take(&mut self.compacted) {
            self.push(Item::compacted());
        }
    }

    fn read(&mut self, message: &Value) {
        self.task_notification(message);
        let top = message["parent_tool_use_id"].is_null();
        match message["type"].as_str() {
            Some("system") if message["subtype"] == "task_started" => {
                if let (Some(task), Some(tool)) = (message["task_id"].as_str(), message["tool_use_id"].as_str()) {
                    self.tasks.insert(task.to_owned(), tool.to_owned());
                }
            }
            Some("system") if message["subtype"] == "compact_boundary" => {
                self.push(Item::compacted());
            }
            Some("system") => {
                if message["subtype"] == "init"
                    && let Some(session) = message["session_id"].as_str()
                {
                    self.session = Some(session.to_owned());
                    let _ = self.events.send(Event::Session(session.to_owned()));
                }
                if let Some(mode) = message["permissionMode"].as_str() {
                    self.observe(mode);
                }
            }
            Some("stream_event") if top => self.stream(&message["event"]),
            Some("assistant") if top => self.assistant(message, &message["thinking_duration_ms"], true),
            Some("user") if top => self.results(&message["message"]["content"], &message["tool_use_result"]),
            Some("assistant" | "user") => {
                if let Some(parent) = message["parent_tool_use_id"].as_str() {
                    self.route(parent, message);
                }
            }
            Some("command_lifecycle") if message["state"] != "queued" => {
                if let Some(uuid) = message["command_uuid"].as_str() {
                    self.steering.remove(uuid);
                }
            }
            Some("control_request") if message["request"]["subtype"] == "can_use_tool" => {
                if let Some(id) = message["request_id"].as_str() {
                    self.inputs.insert(id.to_owned(), message["request"].clone());
                    self.ask(request(id, &message["request"]));
                }
            }
            Some("control_cancel_request") => self.resolve(message["request_id"].as_str().unwrap_or_default()),
            Some("result") => {
                self.total(&message["modelUsage"]);
                // A steer Claude has not started yet runs as a turn of its own after this result.
                if self.steering.is_empty() {
                    self.report(Status::Done);
                }
            }
            Some("rate_limit_event") if !self.replaying => {
                let windows = &message["rate_limit_info"]["unifiedWindows"];
                let readings = WINDOWS.into_iter().filter_map(|(window, name)| {
                    let used = windows[name]["utilization"].as_f64()?;
                    let resets_at = windows[name]["resetsAt"].as_u64().map(|seconds| seconds * 1000);
                    Some(Reading::new(Agent::ClaudeCode, window, used, resets_at))
                });
                let _ = self.events.send(Event::Usage(readings.collect()));
            }
            Some("control_response") => self.answered(&message["response"]),
            Some("conversation_reset") => {
                let _ = self.events.send(Event::QuestionsTruncated(0));
                self.activity.clear(&self.events);
                self.call = None;
                self.window = None;
                self.len = 0;
                self.streaming.clear();
                self.tools = None;
                self.commands.clear();
                self.snippets.clear();
                self.edits.clear();
                self.subagents.clear();
                self.todos.clear();
                self.creating.clear();
                self.tasks.clear();
                self.todos_held = false;
                self.pending.clear();
                self.checkpoints.clear();
                self.steering.clear();
                let _ = self.events.send(Event::Reset);
                let _ = self.events.send(Event::Truncate(0));
            }
            _ => {}
        }
    }

    fn route(&mut self, parent: &str, message: &Value) -> bool {
        for (id, (subagent, transcript)) in &mut self.subagents {
            if id == parent {
                transcript.nested(message, &message["tool_use_result"]);
            } else if !transcript.route(parent, message) {
                continue;
            }
            if subagent.update() {
                let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
            }
            return true;
        }
        false
    }

    fn task_notification(&mut self, message: &Value) {
        match message["type"].as_str() {
            Some("system") if message["subtype"] == "task_updated" || message["subtype"] == "task_notification" => {
                let usage = &message["usage"];
                self.task_status(
                    message["task_id"].as_str(),
                    message["tool_use_id"].as_str(),
                    message["status"].as_str().or_else(|| message["patch"]["status"].as_str()),
                    message["summary"].as_str(),
                    [&usage["duration_ms"], &usage["tool_uses"], &usage["total_tokens"]].map(Value::as_u64),
                );
            }
            Some("user") => {
                let content = &message["message"]["content"];
                if let Some(text) = content.as_str() {
                    self.task_text(text);
                } else {
                    for block in content.as_array().into_iter().flatten() {
                        if let Some(text) = block["text"].as_str() { self.task_text(text); }
                    }
                }
            }
            Some("attachment") if message["attachment"]["type"] == "queued_command" => {
                if let Some(text) = message["attachment"]["prompt"].as_str() { self.task_text(text); }
            }
            _ => {}
        }
    }

    fn task_text(&mut self, text: &str) {
        for notification in text.split("<task-notification>").skip(1) {
            if let Some((notification, _)) = notification.split_once("</task-notification>") {
                let usage = tagged(notification, "usage").unwrap_or_default();
                let total = |tag: &str| tagged(usage, tag).and_then(|total| total.trim().parse().ok());
                self.task_status(
                    tagged(notification, "task-id"),
                    tagged(notification, "tool-use-id"),
                    tagged(notification, "status"),
                    tagged(notification, "result"),
                    [total("duration_ms"), total("tool_uses"), total("subagent_tokens").or_else(|| total("total_tokens"))],
                );
            }
        }
    }

    /// The text form of a notification carries the report as its result, and its summary only names the agent.
    fn task_status(&mut self, task: Option<&str>, tool: Option<&str>, status: Option<&str>, report: Option<&str>, totals: [Option<u64>; 3]) {
        let tool = tool.filter(|tool| self.subagents.contains_key(*tool)).or_else(|| task.and_then(|task| self.tasks.get(task).map(String::as_str)));
        if let Some(outcome) = outcome(status)
            && let Some((subagent, _)) = tool.and_then(|tool| self.subagents.get_mut(tool))
        {
            subagent.finish(Some(outcome));
            reported(subagent, report, totals);
            let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
        }
    }

    fn called(&mut self, message: &Value) {
        let usage = &message["usage"];
        let sent = ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"];
        let used = sent.iter().filter_map(|field| usage[field].as_u64()).sum();
        self.call = message["model"].as_str().map(|model| (model.to_owned(), used));
        self.meter(None);
    }

    fn total(&mut self, models: &Value) {
        let Some(models) = models.as_object().filter(|models| !models.is_empty()) else { return };
        let count = |fields: &[&str]| {
            models.values().flat_map(|usage| fields.iter().filter_map(|field| usage[field].as_u64())).sum()
        };
        let input = count(&["inputTokens", "cacheReadInputTokens", "cacheCreationInputTokens"]);
        let output = count(&["outputTokens"]);
        if let Some((model, _)) = &self.call
            && let Some(window) = models.get(model).and_then(|usage| usage["contextWindow"].as_u64())
        {
            self.window = Some(window);
        }
        self.meter(Some(Tokens { input, output }));
    }

    fn meter(&self, tokens: Option<Tokens>) {
        let context = self.call.as_ref().zip(self.window).map(|((_, used), window)| Context { used: *used, window });
        if context.is_some() || tokens.is_some() {
            let _ = self.events.send(Event::Meters(context, tokens));
        }
    }

    /// Text blocks of a `streamed` message already arrived as stream events.
    fn assistant(&mut self, message: &Value, thinking: &Value, streamed: bool) {
        self.begin(&message["message"]);
        let blocks = message["message"]["content"].as_array().into_iter().flatten();
        if let Some(error) = message["error"].as_str() {
            let text: Vec<&str> = blocks.filter_map(|block| block["text"].as_str()).collect();
            let notice = if error == "rate_limit" { Notice::Limit } else { Notice::Error };
            self.push(Item::Notice { notice, text: text.join("\n") });
            return;
        }
        for block in blocks {
            match block["type"].as_str() {
                Some("text") if !streamed => {
                    let text = block["text"].as_str().unwrap_or_default().to_owned();
                    let index = self.push(Item::Text { text: text.clone(), streaming: false, phase: self.phase });
                    self.said.push((index, text));
                }
                Some("thinking") => {
                    let text = block["thinking"].as_str().filter(|text| !text.is_empty()).map(str::to_owned);
                    self.push(Item::Thinking { seconds: seconds(thinking.as_u64().unwrap_or_default()), text });
                }
                Some("tool_use") => {
                    self.phase = Phase::Commentary;
                    for (index, text) in self.said.drain(..) {
                        let _ = self.events.send(Event::Item(index, Item::Text { text, streaming: false, phase: Phase::Commentary }));
                    }
                    self.tool(block, &message["timestamp"]);
                }
                _ => {}
            }
        }
    }

    /// Only a tool call later in the same message shows that its text was said while working.
    fn begin(&mut self, message: &Value) {
        let id = message["id"].as_str();
        if id.is_none() || self.message.as_deref() != id {
            self.message = id.map(str::to_owned);
            self.phase = Phase::Final;
            self.said.clear();
        }
    }

    fn tool(&mut self, block: &Value, timestamp: &Value) {
        let (Some(id), Some(name)) = (block["id"].as_str(), block["name"].as_str()) else { return };
        let input = &block["input"];
        let text = |key: &str| input[key].as_str().unwrap_or_default().to_owned();
        if !self.replaying {
            let label = match name {
                "Bash" => format!("Running {}", text("command")),
                "Read" => format!("Reading {}", self.relative(&text("file_path"))),
                "Grep" | "Glob" => format!("Searching {}", text("pattern")),
                "WebFetch" => format!("Fetching {}", text("url")),
                "WebSearch" => format!("Searching the web for {}", text("query")),
                "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => format!("Editing {}", self.relative(&text("file_path"))),
                "Agent" | "Task" => format!("Starting agent: {}", text("description")),
                _ => format!("Using {name}"),
            };
            self.activity.start(id, label, &self.events);
        }
        match name {
            html::CLAUDE_TOOL => {
                if let Some(item) = html::item(input) {
                    self.snippets.insert(id.to_owned(), item);
                } else {
                    self.call(ToolKind::Mcp, name, input.to_string());
                }
            }
            "Bash" => {
                let command = text("command");
                let running = Item::Command { command: command.clone(), status: CommandStatus::Running, output: String::new() };
                let index = self.push(running);
                self.commands.insert(id.to_owned(), (index, command));
            }
            "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => {
                self.edits.insert(id.to_owned(), name == "Write");
            }
            "Read" => self.call(ToolKind::Read, name, self.relative(&text("file_path"))),
            "Grep" | "Glob" => self.call(ToolKind::Search, name, text("pattern")),
            "WebFetch" => self.call(ToolKind::Fetch, name, text("url")),
            "WebSearch" => self.call(ToolKind::Web, name, text("query")),
            "Agent" | "Task" => {
                let replaying = self.replaying || self.history.is_some();
                let started_at = timestamp.as_str().filter(|_| replaying).and_then(epoch_ms).unwrap_or_else(super::now);
                let (mut subagent, events) = Subagent::new(id, text("description"), started_at);
                subagent.name = input["subagent_type"].as_str().map(str::to_owned);
                subagent.prompt = input["prompt"].as_str().map(str::to_owned);
                subagent.model = input["model"].as_str().map(str::to_owned);
                subagent.index = self.push(subagent.item());
                self.subagents.insert(id.to_owned(), (subagent, Transcript::new(events, &self.cwd)));
            }
            "TodoWrite" => {
                let todos = input["todos"].as_array().into_iter().flatten().map(|todo| {
                    let text = todo["content"].as_str().unwrap_or_default().to_owned();
                    (String::new(), Todo { text, status: todo_status(todo["status"].as_str().unwrap_or_default()) })
                });
                self.todos = todos.collect();
                self.report_todos();
            }
            "TaskCreate" => {
                self.creating.insert(id.to_owned());
            }
            "TaskUpdate" => {
                let task = input["taskId"].as_str().unwrap_or_default();
                if input["status"] == "deleted" {
                    self.todos.retain(|(id, _)| id != task);
                } else if let Some((_, todo)) = self.todos.iter_mut().find(|(id, _)| id == task) {
                    if let Some(status) = input["status"].as_str() {
                        todo.status = todo_status(status);
                    }
                    if let Some(subject) = input["subject"].as_str() {
                        todo.text = subject.to_owned();
                    }
                }
                self.report_todos();
            }
            _ if name.starts_with("mcp__") => self.call(ToolKind::Mcp, name, input.to_string()),
            _ => self.call(ToolKind::Other, name, input.to_string()),
        }
    }

    /// Claude streams only a subagent's tool calls and their results, so its other activity is left out of the session file's too.
    fn nested(&mut self, message: &Value, result: &Value) {
        let content = &message["message"]["content"];
        match message["type"].as_str() {
            Some("assistant") => {
                for block in content.as_array().into_iter().flatten().filter(|block| block["type"] == "tool_use") {
                    self.tool(block, &message["timestamp"]);
                }
            }
            Some("user") => self.results(content, result),
            _ => {}
        }
    }

    fn results(&mut self, content: &Value, result: &Value) {
        for block in content.as_array().into_iter().flatten().filter(|block| block["type"] == "tool_result") {
            let Some(id) = block["tool_use_id"].as_str() else { continue };
            if block["is_error"] != true && result["answers"].is_object()
                && let Ok(questions) = serde_json::from_value::<Vec<Question>>(result["questions"].clone())
                && !questions.is_empty()
            {
                let answers = questions.iter().map(|question| {
                    recorded_answers(question, result["answers"][&question.question].as_str().unwrap_or_default())
                }).collect();
                let record = AnsweredQuestions { source: Some(id.to_owned()), after: self.len, questions, answers };
                let _ = self.events.send(Event::QuestionsRecovered(record));
            }
            self.activity.finish(id, &self.events);
            if let Some(item) = self.snippets.remove(id) {
                if block["is_error"] != true {
                    self.push(item);
                }
            }
            if let Some(write) = self.edits.remove(id) && block["is_error"] != true {
                self.edit(result, write);
            }
            if self.creating.remove(id)
                && let (Some(task), Some(text)) = (result["task"]["id"].as_str(), result["task"]["subject"].as_str())
            {
                self.todos.push((task.to_owned(), Todo { text: text.to_owned(), status: TodoStatus::Pending }));
                self.report_todos();
            }
            if let Some((subagent, transcript)) = self.subagents.get_mut(id) {
                if let Some(task) = result["agentId"].as_str() { self.tasks.insert(task.to_owned(), id.to_owned()); }
                if let Some(history) = &self.history
                    && let Some(agent) = result["agentId"].as_str()
                {
                    let path = history.with_extension("").join("subagents").join(format!("agent-{agent}.jsonl"));
                    let contents = fs::read_to_string(path).unwrap_or_default();
                    transcript.history = Some(history.clone());
                    for entry in contents.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()) {
                        transcript.nested(&entry, &entry["toolUseResult"]);
                    }
                    transcript.history = None;
                }
                subagent.update();
                if let Some(model) = result["resolvedModel"].as_str() {
                    subagent.model = Some(model.to_owned());
                }
                if result["isAsync"] != true {
                    let texts: Vec<&str> = result["content"].as_array().into_iter().flatten().filter_map(|block| block["text"].as_str()).collect();
                    let report = Some(texts.join("\n")).filter(|report| !report.is_empty());
                    let fallback = if block["is_error"] == true { Outcome::Failed } else { Outcome::Completed };
                    subagent.finish(Some(outcome(result["status"].as_str()).unwrap_or(fallback)));
                    let totals = [&result["totalDurationMs"], &result["totalToolUseCount"], &result["totalTokens"]].map(Value::as_u64);
                    reported(subagent, report.as_deref(), totals);
                }
                let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
            }
            let Some((index, command)) = self.commands.remove(id) else {
                continue;
            };
            let status = if block["is_error"] == true { CommandStatus::Failed } else { CommandStatus::Ok };
            let output = match &block["content"] {
                Value::String(text) => text.clone(),
                blocks => {
                    let texts: Vec<&str> = blocks.as_array().into_iter().flatten().filter_map(|block| block["text"].as_str()).collect();
                    texts.join("\n")
                }
            };
            let _ = self.events.send(Event::Item(index, Item::Command { command, status, output }));
        }
        if interrupted(content) {
            self.push(Item::interrupted());
        }
    }

    /// A created file's patch is empty, and a notebook edit reports the whole notebook before and after instead.
    fn edit(&mut self, result: &Value, write: bool) {
        let text = |key: &str| result[key].as_str().unwrap_or_default();
        let diff = match result["structuredPatch"].as_array() {
            Some(_) if result["type"] == "create" => unified("", text("content")),
            Some(hunks) => hunks.iter().map(hunk).collect(),
            None => unified(text("original_file"), text("updated_file")),
        };
        let path = result["filePath"].as_str().unwrap_or(text("notebook_path"));
        if !path.is_empty() {
            self.push(Item::edit(self.relative(path), diff, write));
        }
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

    fn relative(&self, path: &str) -> String {
        super::relative(&self.cwd, path)
    }

    fn stream(&mut self, event: &Value) {
        if event["type"] == "message_start" {
            self.begin(&event["message"]);
            return self.called(&event["message"]);
        }
        let Some(block) = event["index"].as_u64() else { return };
        let activity_id = format!("block:{block}");
        if event["type"] == "content_block_start" {
            let content = &event["content_block"];
            match content["type"].as_str() {
                Some("thinking") => self.activity.start(&activity_id, "Thinking…".to_owned(), &self.events),
                Some("text") => self.activity.start(&activity_id, "Writing response…".to_owned(), &self.events),
                Some("tool_use") => {
                    if let (Some(id), Some(name)) = (content["id"].as_str(), content["name"].as_str()) {
                        self.activity.start(id, format!("Using {name}"), &self.events);
                    }
                }
                _ => {}
            }
        }
        if event["type"] == "content_block_stop" { self.activity.finish(&activity_id, &self.events); }
        match event["type"].as_str() {
            Some("content_block_start") if event["content_block"]["type"] == "text" => {
                let text = event["content_block"]["text"].as_str().unwrap_or_default().to_owned();
                let index = self.push(Item::Text { text: text.clone(), streaming: true, phase: self.phase });
                self.streaming.insert(block, (index, text));
            }
            Some("content_block_delta") if event["delta"]["type"] == "text_delta" => {
                if let Some((index, text)) = self.streaming.get_mut(&block) {
                    text.push_str(event["delta"]["text"].as_str().unwrap_or_default());
                    let item = Item::Text { text: text.clone(), streaming: true, phase: self.phase };
                    let _ = self.events.send(Event::Item(*index, item));
                }
            }
            Some("content_block_stop") => {
                if let Some((index, text)) = self.streaming.remove(&block) {
                    let _ = self.events.send(Event::Item(index, Item::Text { text: text.clone(), streaming: false, phase: self.phase }));
                    self.said.push((index, text));
                }
            }
            _ => {}
        }
    }
}

fn hunk(hunk: &Value) -> String {
    let count = |key: &str| hunk[key].as_u64().unwrap_or_default();
    let mut text = format!("@@ -{},{} +{},{} @@\n", count("oldStart"), count("oldLines"), count("newStart"), count("newLines"));
    for line in hunk["lines"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        text.push_str(line);
        text.push('\n');
    }
    text
}

fn outcome(status: Option<&str>) -> Option<Outcome> {
    match status? {
        "completed" => Some(Outcome::Completed),
        "failed" => Some(Outcome::Failed),
        "stopped" => Some(Outcome::Stopped),
        _ => None,
    }
}

fn reported(subagent: &mut Subagent, report: Option<&str>, [duration, tools, tokens]: [Option<u64>; 3]) {
    if let Some(report) = report {
        subagent.report = Some(report.to_owned());
    }
    subagent.duration = duration.or(subagent.duration);
    subagent.tools = tools.map(|tools| tools as usize).or(subagent.tools);
    subagent.tokens = tokens.or(subagent.tokens);
}

fn todo_status(status: &str) -> TodoStatus {
    match status {
        "in_progress" => TodoStatus::Active,
        "completed" => TodoStatus::Done,
        _ => TodoStatus::Pending,
    }
}

/// The prompt a session-file user entry records: slash commands are stored as tags, and their output is not a prompt.
fn prompt(content: &str) -> Option<String> {
    if content.starts_with("<local-command-") {
        return None;
    }
    if !content.starts_with("<command-") {
        return Some(content.to_owned());
    }
    let tag = |name: &str| {
        let start = content.find(&format!("<{name}>"))? + name.len() + 2;
        let end = content[start..].find(&format!("</{name}>"))?;
        Some(content[start..start + end].trim())
    };
    let name = tag("command-name")?;
    Some(match tag("command-args") {
        Some(args) if !args.is_empty() => format!("{name} {args}"),
        _ => name.to_owned(),
    })
}

#[cfg(test)]
mod subagent_tests {
    use super::*;

    #[test]
    fn background_completion_is_restored_without_hiding_active_agents() {
        for status in ["completed", "failed", "stopped"] {
            for recorded in [false, true] {
                for shape in ["system", "updated", "text", "blocks", "attachment"] {
                    let (events, mut received) = mpsc::unbounded_channel();
                    let mut transcript = Transcript::new(events, Path::new("/project"));
                    let mut entries = Vec::new();
                    for id in ["finished", "active"] {
                        entries.push(json!({ "type": "assistant", "message": { "content": [{
                            "type": "tool_use", "id": id, "name": "Agent", "input": { "description": id },
                        }] } }));
                        let mut result = json!({ "type": "user", "message": { "content": [{
                            "type": "tool_result", "tool_use_id": id, "content": "Async agent launched successfully.",
                        }] } });
                        result[if recorded { "toolUseResult" } else { "tool_use_result" }] = json!({ "isAsync": true, "agentId": format!("agent-{id}") });
                        entries.push(result);
                    }
                    let notification = format!("<task-notification>\n<task-id>agent-finished</task-id>\n<status>{status}</status>\n</task-notification>");
                    entries.push(match shape {
                        "system" => json!({ "type": "system", "subtype": "task_notification", "task_id": "agent-finished", "tool_use_id": "resumed-tool", "status": status }),
                        "updated" => json!({ "type": "system", "subtype": "task_updated", "tool_use_id": "finished", "patch": { "status": status } }),
                        "text" => json!({ "type": "user", "promptSource": "system", "message": { "content": notification } }),
                        "blocks" => json!({ "type": "user", "promptSource": "system", "message": { "content": [{ "type": "text", "text": notification }] } }),
                        _ => json!({ "type": "attachment", "isMeta": true, "renderedRole": "system", "attachment": { "type": "queued_command", "prompt": notification } }),
                    });
                    if recorded {
                        let directory = tempfile::tempdir().unwrap();
                        let project = directory.path().join("projects/test");
                        fs::create_dir_all(&project).unwrap();
                        fs::write(project.join("session.jsonl"), entries.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap();
                        let env = BTreeMap::from([("CLAUDE_CONFIG_DIR".to_owned(), directory.path().to_str().unwrap().to_owned())]);
                        transcript.history(&env, "session", None);
                    } else {
                        for entry in entries { transcript.read(&entry); }
                    }
                    let mut items = BTreeMap::new();
                    while let Ok(event) = received.try_recv() {
                        if let Event::Item(index, item) = event { items.insert(index, item); }
                    }
                    let statuses: Vec<_> = items.values().filter_map(|item| match item {
                        Item::Subagent { task, status, outcome, .. } => Some((task.as_str(), *status, *outcome)),
                        _ => None,
                    }).collect();
                    let outcome = match status {
                        "completed" => Outcome::Completed,
                        "failed" => Outcome::Failed,
                        _ => Outcome::Stopped,
                    };
                    assert_eq!(statuses, [("finished", Status::Done, Some(outcome)), ("active", Status::Working, None)], "{status}, recorded: {recorded}, {shape}");
                    assert!(!items.values().any(|item| matches!(item, Item::User { .. })));
                }
            }
        }
    }

    #[test]
    fn nested_subagents_fill_and_finish_at_any_depth() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new(events, Path::new("/project"));
        let messages = [
            json!({ "type": "assistant", "parent_tool_use_id": null, "message": { "content": [{
                "type": "tool_use", "id": "A", "name": "Agent", "input": { "description": "outer" },
            }] } }),
            json!({ "type": "assistant", "parent_tool_use_id": "A", "message": { "content": [{
                "type": "tool_use", "id": "B", "name": "Agent", "input": { "description": "inner" },
            }] } }),
            json!({ "type": "assistant", "parent_tool_use_id": "B", "message": { "content": [{
                "type": "tool_use", "id": "read", "name": "Read", "input": { "file_path": "/project/notes.txt" },
            }] } }),
            json!({ "type": "user", "parent_tool_use_id": "B", "message": { "content": [{
                "type": "tool_result", "tool_use_id": "read", "content": "notes",
            }] } }),
            json!({ "type": "user", "parent_tool_use_id": "A", "message": { "content": [{
                "type": "tool_result", "tool_use_id": "B", "content": [{ "type": "text", "text": "Found." }],
            }] }, "tool_use_result": {
                "status": "completed", "content": [{ "type": "text", "text": "Found." }],
                "totalDurationMs": 1000, "totalToolUseCount": 1, "totalTokens": 500,
            } }),
        ];
        for message in messages { transcript.read(&message); }
        let outer = std::iter::from_fn(|| received.try_recv().ok()).filter_map(|event| match event {
            Event::Item(_, Item::Subagent { id, items, .. }) if id == "A" => Some(items),
            _ => None,
        }).last().unwrap();
        let [Item::Subagent { id, status, outcome, items, .. }] = outer.as_slice() else { panic!("{outer:?}") };
        assert_eq!((id.as_str(), *status, *outcome), ("B", Status::Done, Some(Outcome::Completed)));
        let read = ToolCall { kind: ToolKind::Read, name: "Read".to_owned(), detail: "notes.txt".to_owned() };
        assert!(matches!(items.as_slice(), [Item::Tools { calls }] if calls == &[read]), "{items:?}");
    }
}

#[cfg(test)]
mod html_tests {
    use super::*;

    #[test]
    fn html_renders_only_after_success_live_and_from_history() {
        let arguments = json!({ "title": "Flow", "html": "<b>Start → Finish</b>" });
        let assistant = json!({ "type": "assistant", "message": { "content": [{
            "type": "tool_use", "id": "snippet", "name": html::CLAUDE_TOOL, "input": arguments,
        }] } });
        for recorded in [false, true] {
            for denied in [false, true] {
                let (events, mut received) = mpsc::unbounded_channel();
                let mut transcript = Transcript::new(events, Path::new("/project"));
                let result = json!({ "type": "user", "message": { "content": [{
                    "type": "tool_result", "tool_use_id": "snippet", "is_error": denied, "content": "done",
                }] } });
                if recorded { transcript.recorded(&assistant); } else { transcript.read(&assistant); }
                assert!(!std::iter::from_fn(|| received.try_recv().ok()).any(|event| matches!(event, Event::Item(..))));
                if recorded { transcript.recorded(&result); } else { transcript.read(&result); }
                let items: Vec<_> = std::iter::from_fn(|| received.try_recv().ok()).filter_map(|event| match event {
                    Event::Item(index, item) => Some((index, item)),
                    _ => None,
                }).collect();
                if denied {
                    assert!(items.is_empty());
                } else {
                    assert!(matches!(items.as_slice(), [(0, item)] if Some(item) == html::item(&arguments).as_ref()));
                }
                assert!(transcript.snippets.is_empty());
            }
        }
    }
}

#[cfg(test)]
mod phase_tests {
    use super::*;

    #[test]
    fn text_turns_to_commentary_once_its_message_calls_a_tool_live_and_from_history() {
        let entry = |id: &str, block: Value| json!({ "type": "assistant", "message": { "id": id, "content": [block] } });
        let text = |text: &str| json!({ "type": "text", "text": text });
        let tool = json!({ "type": "tool_use", "id": "read", "name": "Read", "input": { "file_path": "/project/notes.txt" } });
        let streamed = |id: &str, text: &str| {
            [
                json!({ "type": "message_start", "message": { "id": id } }),
                json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } }),
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": text } }),
                json!({ "type": "content_block_stop", "index": 0 }),
            ]
            .map(|event| json!({ "type": "stream_event", "event": event, "parent_tool_use_id": null }))
        };
        for recorded in [false, true] {
            let (events, mut received) = mpsc::unbounded_channel();
            let mut transcript = Transcript::new(events, Path::new("/project"));
            if recorded {
                for message in [entry("working", text("Reading.")), entry("working", tool.clone()), entry("answer", text("Done."))] {
                    transcript.recorded(&message);
                }
            } else {
                let messages = streamed("working", "Reading.").into_iter().chain([entry("working", tool.clone())]).chain(streamed("answer", "Done."));
                for message in messages {
                    transcript.read(&message);
                }
            }
            let mut items = BTreeMap::new();
            while let Ok(event) = received.try_recv() {
                if let Event::Item(index, item) = event {
                    items.insert(index, item);
                }
            }
            let texts: Vec<_> = items.into_values().filter_map(|item| match item {
                Item::Text { text, phase, .. } => Some((text, phase)),
                _ => None,
            }).collect();
            assert_eq!(texts, [("Reading.".to_owned(), Phase::Commentary), ("Done.".to_owned(), Phase::Final)], "recorded: {recorded}");
        }
    }
}
