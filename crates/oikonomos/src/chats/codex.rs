use std::{
    collections::{BTreeMap, HashMap},
    io,
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
use tokio_tungstenite::{WebSocketStream, client_async, tungstenite::Message};

use super::{Chat, CommandStatus, Context, Driver, Event, Item, Notice, Session, Tokens, ToolCall, ToolKind, relative, seconds, unified};
use crate::{
    config::{Agent, AgentSettings, codex_policy},
    hooks::Status,
    usage::{Reading, Window},
};

const CLOSING: Duration = Duration::from_secs(5);
const HEARTBEAT: Duration = Duration::from_secs(5);
const RECONNECTING: Duration = Duration::from_secs(30);

const SOURCES: [&str; 5] = ["cli", "vscode", "exec", "appServer", "unknown"];

pub struct Codex {
    requests: UnboundedSender<Request>,
}

enum Request {
    Send(String),
    Interrupt,
    Close,
}

impl Driver for Codex {
    fn send(&self, text: &str) {
        let _ = self.requests.send(Request::Send(text.to_owned()));
    }

    fn interrupt(&self) {
        let _ = self.requests.send(Request::Interrupt);
    }

    fn close(&self) {
        let _ = self.requests.send(Request::Close);
    }
}

pub async fn start(
    cwd: &Path,
    env: &BTreeMap<String, String>,
    settings: &AgentSettings,
) -> io::Result<(Codex, UnboundedReceiver<Event>)> {
    let (mut connection, proxy) = connect(cwd, env).await?;
    let started = connection.call("thread/start", thread(cwd, settings)).await?;
    let thread = started["thread"]["id"].as_str().ok_or_else(|| io::Error::other("thread/start gave no thread"))?;
    let (events, received) = mpsc::unbounded_channel();
    let _ = events.send(Event::Session(thread.to_owned()));
    Ok((run(connection, proxy, Transcript::new(thread, cwd, events), cwd, env), received))
}

pub async fn resume(chat: &Chat, env: &BTreeMap<String, String>) -> io::Result<(Codex, UnboundedReceiver<Event>)> {
    let thread = chat.agent_session.as_deref().ok_or_else(|| io::Error::other("the chat has no thread"))?;
    let (events, received) = mpsc::unbounded_channel();
    let mut transcript = Transcript::new(thread, &chat.cwd, events);
    transcript.status = chat.status;
    let (connection, proxy) = rejoin(&chat.cwd, env, &mut transcript).await?;
    Ok((run(connection, proxy, transcript, &chat.cwd, env), received))
}

async fn rejoin(
    cwd: &Path,
    env: &BTreeMap<String, String>,
    transcript: &mut Transcript,
) -> io::Result<(Connection, Child)> {
    let (mut connection, proxy) = connect(cwd, env).await?;
    let resumed = connection.call("thread/resume", json!({ "threadId": transcript.thread })).await?;
    transcript.resumed(&resumed["thread"]["turns"]);
    Ok((connection, proxy))
}

async fn reconnect(
    cwd: &Path,
    env: &BTreeMap<String, String>,
    transcript: &mut Transcript,
) -> Option<(Connection, Child)> {
    let deadline = Instant::now() + RECONNECTING;
    loop {
        match rejoin(cwd, env, transcript).await {
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
            if let (Some(id), Some(preview), Some(updated)) =
                (thread["id"].as_str(), thread["preview"].as_str(), thread["updatedAt"].as_u64())
                && !preview.is_empty()
            {
                sessions.push(Session::new(Agent::Codex, id, preview, updated * 1000));
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

async fn connect(cwd: &Path, env: &BTreeMap<String, String>) -> io::Result<(Connection, Child)> {
    let daemon = codex(cwd, env).args(["app-server", "daemon", "start"]).stdout(Stdio::null()).status().await?;
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
    let (socket, _) = client_async("ws://localhost/", stdio).await.map_err(io::Error::other)?;
    let mut connection = Connection { socket, next: 0 };
    let client = json!({ "name": "oikonomos", "title": "Oikonomos", "version": env!("CARGO_PKG_VERSION") });
    let initialize = json!({ "clientInfo": client, "capabilities": { "experimentalApi": true } });
    connection.call("initialize", initialize).await?;
    connection.write(json!({ "method": "initialized" })).await?;
    Ok((connection, proxy))
}

fn run(
    mut connection: Connection,
    mut proxy: Child,
    mut transcript: Transcript,
    cwd: &Path,
    env: &BTreeMap<String, String>,
) -> Codex {
    let (requests, mut pending) = mpsc::unbounded_channel();
    let (cwd, env) = (cwd.to_owned(), env.clone());
    tokio::spawn(async move {
        // The proxy outlives a daemon that has gone until it next relays something.
        let mut heartbeat = interval(HEARTBEAT);
        loop {
            tokio::select! {
                request = pending.recv() => match request {
                    Some(Request::Send(text)) => {
                        let input = json!([{ "type": "text", "text": text, "text_elements": [] }]);
                        let _ = connection.send(&mut transcript, input).await;
                    }
                    Some(Request::Interrupt) => {
                        let _ = connection.interrupt(&mut transcript).await;
                    }
                    Some(Request::Close) | None => {
                        let _ = timeout(CLOSING, connection.close(&mut transcript)).await;
                        break;
                    }
                },
                message = connection.next() => match message {
                    Some(message) => {
                        transcript.read(&message);
                        if let Some(input) = transcript.unsteered.take() {
                            let _ = connection.start(&mut transcript, input).await;
                        }
                    }
                    None => {
                        let _ = proxy.kill().await;
                        match reconnect(&cwd, &env, &mut transcript).await {
                            Some(rejoined) => (connection, proxy) = rejoined,
                            None => return,
                        }
                    }
                },
                _ = heartbeat.tick() => {
                    let _ = connection.ping().await;
                }
            }
        }
        let _ = proxy.kill().await;
    });
    Codex { requests }
}

/// Reads the account's windows from the daemon, which answers without a thread.
pub async fn usage(env: &BTreeMap<String, String>) -> io::Result<Vec<Reading>> {
    let (mut connection, mut proxy) = connect(&crate::home(), env).await?;
    let read = connection.call("account/rateLimits/read", json!({ "excludeResetCreditDetails": true })).await;
    let _ = proxy.kill().await;
    Ok(readings(&read?["rateLimits"]))
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

fn thread(cwd: &Path, settings: &AgentSettings) -> Value {
    let mut params = json!({ "cwd": cwd });
    if let Some(model) = &settings.model {
        params["model"] = json!(model);
    }
    if let Some(effort) = &settings.effort {
        params["config"] = json!({ "model_reasoning_effort": effort });
    }
    if let Some((approval, sandbox)) = settings.mode.as_deref().and_then(codex_policy) {
        params["approvalPolicy"] = json!(approval);
        params["sandbox"] = json!(sandbox);
    }
    params
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
        let id = self.request(method, params).await?;
        loop {
            let message = self.next().await.ok_or_else(|| io::Error::other(format!("codex hung up during {method}")))?;
            if response(&message) == Some(id) {
                return match message.get("error") {
                    Some(error) => Err(io::Error::other(format!("{method}: {error}"))),
                    None => Ok(message["result"].clone()),
                };
            }
        }
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
        let id = self.request("turn/start", json!({ "threadId": transcript.thread, "input": input })).await?;
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
    thread: String,
    cwd: PathBuf,
    events: UnboundedSender<Event>,
    len: usize,
    starting: Option<u64>,
    turn: Option<String>,
    streaming: HashMap<String, (usize, String)>,
    tools: Option<(usize, Vec<ToolCall>)>,
    commands: HashMap<String, usize>,
    previous: u64,
    steering: HashMap<u64, Value>,
    /// A steer that reached the daemon after its turn had ended, to start a turn with instead.
    unsteered: Option<Value>,
    status: Status,
}

impl Transcript {
    fn new(thread: &str, cwd: &Path, events: UnboundedSender<Event>) -> Self {
        Self {
            thread: thread.to_owned(),
            cwd: cwd.to_owned(),
            events,
            len: 0,
            starting: None,
            turn: None,
            streaming: HashMap::new(),
            tools: None,
            commands: HashMap::new(),
            previous: 0,
            steering: HashMap::new(),
            unsteered: None,
            status: Status::Done,
        }
    }

    fn report(&mut self, status: Status) {
        self.status = status;
        let _ = self.events.send(Event::Status(status));
    }

    fn push(&mut self, item: Item) -> usize {
        let index = self.len;
        self.len += 1;
        self.tools = None;
        let _ = self.events.send(Event::Item(index, item));
        index
    }

    fn stream(&mut self, id: &str, text: &str) {
        let index = self.push(Item::Text { text: text.to_owned(), streaming: true });
        self.streaming.insert(id.to_owned(), (index, text.to_owned()));
    }

    fn resumed(&mut self, turns: &Value) {
        let turns = turns.as_array().map(Vec::as_slice).unwrap_or_default();
        let _ = self.events.send(Event::Truncate(0));
        self.len = 0;
        self.tools = None;
        self.starting = None;
        self.streaming.clear();
        self.commands.clear();
        self.steering.clear();
        self.unsteered = None;
        for item in turns.iter().flat_map(|turn| turn["items"].as_array().into_iter().flatten()) {
            match item["type"].as_str() {
                Some("userMessage") => {
                    self.push(user(item));
                }
                _ => self.completed(item, 0),
            }
        }
        let last = turns.last();
        let running = last.filter(|turn| turn["status"] == "inProgress");
        self.turn = running.and_then(|turn| turn["id"].as_str()).map(str::to_owned);
        if self.status != Status::Done && running.is_none() && !last.is_some_and(|turn| turn["status"] == "completed") {
            self.push(Item::interrupted());
        }
        self.report(if self.turn.is_some() { Status::Working } else { Status::Done });
    }

    fn read(&mut self, message: &Value) {
        if self.starting.is_some() && response(message) == self.starting {
            self.starting = None;
            match message["result"]["turn"]["id"].as_str() {
                Some(turn) => self.turn = Some(turn.to_owned()),
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
        let params = &message["params"];
        if params.get("threadId").is_some_and(|thread| thread != self.thread.as_str()) {
            return;
        }
        let item = &params["item"];
        match message["method"].as_str() {
            Some("thread/tokenUsage/updated") => {
                let usage = &params["tokenUsage"];
                let (used, window) = (usage["last"]["totalTokens"].as_u64(), usage["modelContextWindow"].as_u64());
                let context = used.zip(window).map(|(used, window)| Context { used, window });
                let (input, output) = (usage["total"]["inputTokens"].as_u64(), usage["total"]["outputTokens"].as_u64());
                let tokens = input.zip(output).map(|(input, output)| Tokens { input, output });
                let _ = self.events.send(Event::Meters(context, tokens));
            }
            Some("turn/completed") => {
                self.turn = None;
                // A message cut off by an interrupt or a steer never gets its item/completed.
                for (_, (index, text)) in self.streaming.drain() {
                    let _ = self.events.send(Event::Item(index, Item::Text { text, streaming: false }));
                }
                if params["turn"]["status"] == "failed" {
                    self.push(notice(&params["turn"]["error"]));
                }
                if params["turn"]["status"] == "interrupted" {
                    self.push(Item::interrupted());
                }
                self.report(Status::Done);
            }
            Some("item/started") if item["type"] == "userMessage" => {
                self.push(user(item));
            }
            Some("item/started") if item["type"] == "agentMessage" => {
                let (Some(id), Some(text)) = (item["id"].as_str(), item["text"].as_str()) else { return };
                self.stream(id, text);
            }
            Some("item/agentMessage/delta") => {
                let Some(id) = params["itemId"].as_str() else { return };
                let delta = params["delta"].as_str().unwrap_or_default();
                match self.streaming.get_mut(id) {
                    Some((index, text)) => {
                        text.push_str(delta);
                        let item = Item::Text { text: text.clone(), streaming: true };
                        let _ = self.events.send(Event::Item(*index, item));
                    }
                    // A resume rejoins a streaming message without its item/started or its text so far.
                    None => self.stream(id, delta),
                }
            }
            Some("item/started") if item["type"] == "commandExecution" => {
                let index = self.push(command(item));
                if let Some(id) = item["id"].as_str() {
                    self.commands.insert(id.to_owned(), index);
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

    /// Codex announces reasoning once it is done, so its duration runs from the item before.
    fn completed(&mut self, item: &Value, elapsed: u64) {
        let text = |key: &str| item[key].as_str().unwrap_or_default().to_owned();
        match item["type"].as_str() {
            Some("agentMessage") => match item["id"].as_str().and_then(|id| self.streaming.remove(id)) {
                Some((index, _)) => {
                    let _ = self.events.send(Event::Item(index, agent(item)));
                }
                None => {
                    self.push(agent(item));
                }
            },
            Some("reasoning") => {
                let parts = |key: &str| {
                    let parts: Vec<&str> = item[key].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                    Some(parts.join("\n\n")).filter(|text| !text.is_empty())
                };
                self.push(Item::Thinking { seconds: seconds(elapsed), text: parts("summary").or_else(|| parts("content")) });
            }
            Some("commandExecution") => match item["id"].as_str().and_then(|id| self.commands.remove(id)) {
                Some(index) => {
                    let _ = self.events.send(Event::Item(index, command(item)));
                }
                None => {
                    self.push(command(item));
                }
            },
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
                    self.push(Item::edit(relative(&self.cwd, path), diff));
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
                let name = format!("{}.{}", text("server"), text("tool"));
                self.call(ToolKind::Other, &name, item["arguments"].to_string());
            }
            Some("dynamicToolCall") => self.call(ToolKind::Other, &text("tool"), item["arguments"].to_string()),
            Some("imageView") => self.call(ToolKind::Read, "imageView", text("path")),
            Some("contextCompaction") => {
                self.push(Item::compacted());
            }
            _ => {}
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
    Item::Command { command, status, output: item["aggregatedOutput"].as_str().unwrap_or_default().to_owned() }
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
    let text: Vec<&str> = content.filter_map(|input| input["text"].as_str()).collect();
    Item::User { text: text.join("\n"), images: 0 }
}

fn agent(item: &Value) -> Item {
    Item::Text { text: item["text"].as_str().unwrap_or_default().to_owned(), streaming: false }
}
