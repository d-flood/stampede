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
    Answer, Chat, CommandStatus, Context, Driver, Event, Item, Notice, Session, Tokens, ToolCall, ToolKind,
    holder::{self, Frames},
    protocol::{self, Message},
    Request, diff, seconds, unified, words,
};
use crate::{
    config::{Agent, AgentSettings},
    hooks::Status,
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

pub struct Claude {
    commands: UnboundedSender<Command>,
}

enum Command {
    Send(String),
    Interrupt,
    Answer(String, Answer),
    Close,
}

impl Driver for Claude {
    fn send(&self, text: &str) {
        let _ = self.commands.send(Command::Send(text.to_owned()));
    }

    fn interrupt(&self) {
        let _ = self.commands.send(Command::Interrupt);
    }

    fn answer(&self, request_id: &str, answer: Answer) {
        let _ = self.commands.send(Command::Answer(request_id.to_owned(), answer));
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
    let (frames, writer, transcript, received) = launch(data_dir, id, cwd, env, settings, session).await?;
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
        launch(data_dir, &chat.id, &chat.cwd, env, &chat.settings, session).await?;
    transcript.window = chat.context.map(|context| context.window);
    if chat.status != Status::Done {
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
) -> io::Result<(Frames, OwnedWriteHalf, Transcript, UnboundedReceiver<Event>)> {
    let variables = env.iter().map(|(name, value)| (name.as_str(), value.as_str())).chain(SDK_ENV);
    let command = ["claude"].iter().chain(&ARGS).map(|arg| arg.to_string()).chain(settings.flags(Agent::ClaudeCode));
    let resume = session.into_iter().flat_map(|session| ["--resume".to_owned(), session.to_owned()]);
    holder::spawn(data_dir, id, cwd, variables, command.chain(resume)).await?;
    let (frames, mut writer, _) = holder::connect(data_dir, id).await?;
    let initialize = json!({
        "type": "control_request",
        "request_id": Uuid::new_v4().to_string(),
        "request": { "subtype": "initialize" },
    });
    writer.write_all(&input(&initialize)).await?;
    let (events, received) = mpsc::unbounded_channel();
    let mut transcript = Transcript::new(events, cwd);
    if let Some(session) = session {
        transcript.history(env, session, None);
    }
    Ok((frames, writer, transcript, received))
}

/// Rebuilds the transcript from the session file up to the holder's latest turn, then from the turn the holder replays.
pub async fn reattach(
    data_dir: &Path,
    chat: &Chat,
    env: &BTreeMap<String, String>,
) -> io::Result<(Claude, UnboundedReceiver<Event>)> {
    let (mut frames, writer, buffered) = holder::connect(data_dir, &chat.id).await?;
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
    transcript.replaying = true;
    transcript.window = chat.context.map(|context| context.window);
    if let Some(session) = &chat.agent_session {
        transcript.history(env, session, turn.as_deref());
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
    Ok((drive(frames, writer, transcript), received))
}

fn drive(mut frames: Frames, mut writer: OwnedWriteHalf, mut transcript: Transcript) -> Claude {
    let (commands, mut pending) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                command = pending.recv() => match command {
                    Some(Command::Send(text)) => {
                        let mut message = json!({
                            "type": "user",
                            "message": { "role": "user", "content": text },
                            "parent_tool_use_id": null,
                            "uuid": Uuid::new_v4().to_string(),
                        });
                        let idle = transcript.status == Status::Done;
                        if !idle {
                            message["priority"] = json!("next");
                        }
                        transcript.sent(&message);
                        if idle && writer.write_all(&protocol::encode(&Message::Forget)).await.is_err() {
                            break;
                        }
                        if writer.write_all(&input(&message)).await.is_err() {
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
                        let response = match answer {
                            Answer::Allow => json!({ "behavior": "allow" }),
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
        Some("Bash") => Request::Command { id, words: words(&text("command")) },
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

fn input(message: &Value) -> Vec<u8> {
    protocol::encode(&Message::Input(message.to_string().into_bytes()))
}

fn interrupted(content: &Value) -> bool {
    let mut blocks = content.as_array().into_iter().flatten();
    blocks.any(|block| block["text"].as_str().is_some_and(|text| text.starts_with("[Request interrupted by user")))
}

struct Transcript {
    events: UnboundedSender<Event>,
    cwd: PathBuf,
    len: usize,
    streaming: HashMap<u64, (usize, String)>,
    tools: Option<(usize, Vec<ToolCall>)>,
    commands: HashMap<String, (usize, String)>,
    edits: HashSet<String>,
    compacted: bool,
    status: Status,
    replaying: bool,
    pending: Vec<Request>,
    /// The model of the main loop's latest API call, and how many tokens it was sent.
    call: Option<(String, u64)>,
    window: Option<u64>,
    steering: HashSet<String>,
}

impl Transcript {
    fn new(events: UnboundedSender<Event>, cwd: &Path) -> Self {
        Self {
            events,
            cwd: cwd.to_owned(),
            len: 0,
            streaming: HashMap::new(),
            tools: None,
            commands: HashMap::new(),
            edits: HashSet::new(),
            compacted: false,
            status: Status::Done,
            replaying: false,
            pending: Vec::new(),
            call: None,
            window: None,
            steering: HashSet::new(),
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

    fn sent(&mut self, message: &Value) {
        if let Some(text) = message["message"]["content"].as_str() {
            self.push(Item::User { text: text.to_owned(), images: 0 });
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
        let Some(contents) = session_file(env, session).and_then(|path| fs::read_to_string(path).ok()) else { return };
        for entry in contents.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()) {
            if until.is_some() && entry["uuid"].as_str() == until {
                // A compaction recorded just before the replayed turn's prompt belongs to that turn, which replays it.
                self.compacted = false;
                break;
            }
            self.recorded(&entry);
        }
        self.compact();
    }

    /// A compaction is recorded before the prompt that asked for it, so its notice waits for that prompt.
    fn recorded(&mut self, entry: &Value) {
        if entry["isSidechain"] == true || entry["isMeta"] == true || entry["isCompactSummary"] == true {
            return;
        }
        match entry["type"].as_str() {
            Some("system") if entry["subtype"] == "compact_boundary" => self.compacted = true,
            Some("user") => match entry["message"]["content"].as_str() {
                Some(content) => {
                    if let Some(text) = prompt(content) {
                        self.push(Item::User { text, images: 0 });
                        self.compact();
                    }
                }
                None => {
                    self.compact();
                    self.results(&entry["message"]["content"], &entry["toolUseResult"]);
                }
            },
            Some("attachment") if entry["attachment"]["type"] == "queued_command" => {
                if let Some(text) = entry["attachment"]["prompt"].as_str() {
                    self.push(Item::User { text: text.to_owned(), images: 0 });
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
        let top = message["parent_tool_use_id"].is_null();
        match message["type"].as_str() {
            Some("system") if message["subtype"] == "init" => {
                if let Some(session) = message["session_id"].as_str() {
                    let _ = self.events.send(Event::Session(session.to_owned()));
                }
            }
            Some("system") if message["subtype"] == "compact_boundary" => {
                self.push(Item::compacted());
            }
            Some("stream_event") if top => self.stream(&message["event"]),
            Some("assistant") if top => self.assistant(message, &message["thinking_duration_ms"], true),
            Some("user") if top => self.results(&message["message"]["content"], &message["tool_use_result"]),
            Some("command_lifecycle") if message["state"] != "queued" => {
                if let Some(uuid) = message["command_uuid"].as_str() {
                    self.steering.remove(uuid);
                }
            }
            Some("control_request") if message["request"]["subtype"] == "can_use_tool" => {
                if let Some(id) = message["request_id"].as_str() {
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
            _ => {}
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
                    self.push(Item::Text { text, streaming: false });
                }
                Some("thinking") => {
                    let text = block["thinking"].as_str().filter(|text| !text.is_empty()).map(str::to_owned);
                    self.push(Item::Thinking { seconds: seconds(thinking.as_u64().unwrap_or_default()), text });
                }
                Some("tool_use") => self.tool(block),
                _ => {}
            }
        }
    }

    fn tool(&mut self, block: &Value) {
        let (Some(id), Some(name)) = (block["id"].as_str(), block["name"].as_str()) else { return };
        let input = &block["input"];
        let text = |key: &str| input[key].as_str().unwrap_or_default().to_owned();
        match name {
            "Bash" => {
                let command = text("command");
                let running = Item::Command { command: command.clone(), status: CommandStatus::Running, output: String::new() };
                let index = self.push(running);
                self.commands.insert(id.to_owned(), (index, command));
            }
            "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => {
                self.edits.insert(id.to_owned());
            }
            "Read" => self.call(ToolKind::Read, name, self.relative(&text("file_path"))),
            "Grep" | "Glob" => self.call(ToolKind::Search, name, text("pattern")),
            "WebFetch" => self.call(ToolKind::Fetch, name, text("url")),
            "WebSearch" => self.call(ToolKind::Web, name, text("query")),
            _ => self.call(ToolKind::Other, name, input.to_string()),
        }
    }

    fn results(&mut self, content: &Value, structured: &Value) {
        for block in content.as_array().into_iter().flatten().filter(|block| block["type"] == "tool_result") {
            let Some(id) = block["tool_use_id"].as_str() else { continue };
            if self.edits.remove(id) && block["is_error"] != true {
                self.edit(structured);
            }
            let Some((index, command)) = self.commands.remove(id) else { continue };
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
    fn edit(&mut self, result: &Value) {
        let text = |key: &str| result[key].as_str().unwrap_or_default();
        let diff = match result["structuredPatch"].as_array() {
            Some(_) if result["type"] == "create" => unified("", text("content")),
            Some(hunks) => hunks.iter().map(hunk).collect(),
            None => unified(text("original_file"), text("updated_file")),
        };
        let path = result["filePath"].as_str().unwrap_or(text("notebook_path"));
        if !path.is_empty() {
            self.push(Item::edit(self.relative(path), diff));
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
            return self.called(&event["message"]);
        }
        let Some(block) = event["index"].as_u64() else { return };
        match event["type"].as_str() {
            Some("content_block_start") if event["content_block"]["type"] == "text" => {
                let text = event["content_block"]["text"].as_str().unwrap_or_default().to_owned();
                let index = self.push(Item::Text { text: text.clone(), streaming: true });
                self.streaming.insert(block, (index, text));
            }
            Some("content_block_delta") if event["delta"]["type"] == "text_delta" => {
                if let Some((index, text)) = self.streaming.get_mut(&block) {
                    text.push_str(event["delta"]["text"].as_str().unwrap_or_default());
                    let item = Item::Text { text: text.clone(), streaming: true };
                    let _ = self.events.send(Event::Item(*index, item));
                }
            }
            Some("content_block_stop") => {
                if let Some((index, text)) = self.streaming.remove(&block) {
                    let _ = self.events.send(Event::Item(index, Item::Text { text, streaming: false }));
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
