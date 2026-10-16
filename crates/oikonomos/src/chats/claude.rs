use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
    process::Stdio,
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
    Chat, CommandStatus, Context, Driver, Event, Item, Notice, Tokens, ToolCall, ToolKind,
    holder::{self, Frames},
    protocol::{self, Message},
    seconds,
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

pub struct Claude {
    commands: UnboundedSender<Command>,
}

enum Command {
    Send(String),
    Close,
}

impl Driver for Claude {
    fn send(&self, text: &str) {
        let _ = self.commands.send(Command::Send(text.to_owned()));
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
) -> io::Result<(Claude, UnboundedReceiver<Event>)> {
    let env = env.iter().map(|(name, value)| (name.as_str(), value.as_str())).chain(SDK_ENV);
    let command = ["claude"].iter().chain(&ARGS).map(|arg| arg.to_string()).chain(settings.flags(Agent::ClaudeCode));
    holder::spawn(data_dir, id, cwd, env, command).await?;
    let (frames, mut writer, _) = holder::connect(data_dir, id).await?;
    let initialize = json!({
        "type": "control_request",
        "request_id": Uuid::new_v4().to_string(),
        "request": { "subtype": "initialize" },
    });
    writer.write_all(&input(&initialize)).await?;
    let (events, received) = mpsc::unbounded_channel();
    Ok((drive(frames, writer, Transcript::new(events, cwd)), received))
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
    if let Some(session) = &chat.agent_session
        && let Some(contents) = session_file(env, session).and_then(|path| fs::read_to_string(path).ok())
    {
        for entry in contents.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()) {
            if turn.is_some() && entry["uuid"].as_str() == turn.as_deref() {
                // A compaction recorded just before the replayed turn's prompt belongs to that turn, which replays it.
                transcript.compacted = false;
                break;
            }
            transcript.recorded(&entry);
        }
        transcript.compact();
    }
    for message in replay {
        match message {
            Message::Line(line) => {
                if let Ok(message) = serde_json::from_slice(&line) {
                    transcript.read(&message);
                }
            }
            Message::Input(line) => {
                if let Some(message) = user(&line) {
                    transcript.sent(&message);
                }
            }
            _ => {}
        }
    }
    transcript.replaying = false;
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
                        let message = json!({
                            "type": "user",
                            "message": { "role": "user", "content": text },
                            "parent_tool_use_id": null,
                            "uuid": Uuid::new_v4().to_string(),
                        });
                        let idle = transcript.status == Status::Done;
                        transcript.sent(&message);
                        if idle && writer.write_all(&protocol::encode(&Message::Forget)).await.is_err() {
                            break;
                        }
                        if writer.write_all(&input(&message)).await.is_err() {
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

fn session_file(env: &BTreeMap<String, String>, session: &str) -> Option<PathBuf> {
    let config = match env.get("CLAUDE_CONFIG_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|| std::env::home_dir().map(|home| home.join(".claude")))?,
    };
    let name = format!("{session}.jsonl");
    fs::read_dir(config.join("projects")).ok()?.flatten().map(|project| project.path().join(&name)).find(|path| path.is_file())
}

fn input(message: &Value) -> Vec<u8> {
    protocol::encode(&Message::Input(message.to_string().into_bytes()))
}

struct Transcript {
    events: UnboundedSender<Event>,
    cwd: PathBuf,
    len: usize,
    streaming: HashMap<u64, (usize, String)>,
    tools: Option<(usize, Vec<ToolCall>)>,
    commands: HashMap<String, (usize, String)>,
    compacted: bool,
    status: Status,
    replaying: bool,
    /// The model of the main loop's latest API call, and how many tokens it was sent.
    call: Option<(String, u64)>,
    window: Option<u64>,
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
            compacted: false,
            status: Status::Done,
            replaying: false,
            call: None,
            window: None,
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

    fn sent(&mut self, message: &Value) {
        if let Some(text) = message["message"]["content"].as_str() {
            self.push(Item::User { text: text.to_owned(), images: 0 });
            self.report(Status::Working);
        }
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
                    self.results(&entry["message"]["content"]);
                }
            },
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
            Some("user") if top => self.results(&message["message"]["content"]),
            Some("result") => {
                self.total(&message["modelUsage"]);
                self.report(Status::Done);
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
            "Read" => self.call(ToolKind::Read, name, self.relative(&text("file_path"))),
            "Grep" | "Glob" => self.call(ToolKind::Search, name, text("pattern")),
            "WebFetch" => self.call(ToolKind::Fetch, name, text("url")),
            "WebSearch" => self.call(ToolKind::Web, name, text("query")),
            _ => self.call(ToolKind::Other, name, input.to_string()),
        }
    }

    fn results(&mut self, content: &Value) {
        for block in content.as_array().into_iter().flatten().filter(|block| block["type"] == "tool_result") {
            let Some((index, command)) = block["tool_use_id"].as_str().and_then(|id| self.commands.remove(id)) else {
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
        Path::new(path).strip_prefix(&self.cwd).map_or_else(|_| path.to_owned(), |path| path.display().to_string())
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
