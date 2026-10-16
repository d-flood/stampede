use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
};

use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::{
    io::AsyncWriteExt,
    net::unix::OwnedWriteHalf,
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use uuid::Uuid;

use super::{
    Chat, Context, Driver, Event, Item, Tokens,
    holder::{self, Frames},
    protocol::{self, Message},
};
use crate::hooks::Status;

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
) -> io::Result<(Claude, UnboundedReceiver<Event>)> {
    let env = env.iter().map(|(name, value)| (name.as_str(), value.as_str())).chain(SDK_ENV);
    holder::spawn(data_dir, id, cwd, env, ["claude"].iter().chain(&ARGS)).await?;
    let (frames, mut writer, _) = holder::connect(data_dir, id).await?;
    let initialize = json!({
        "type": "control_request",
        "request_id": Uuid::new_v4().to_string(),
        "request": { "subtype": "initialize" },
    });
    writer.write_all(&input(&initialize)).await?;
    let (events, received) = mpsc::unbounded_channel();
    Ok((drive(frames, writer, Transcript::new(events)), received))
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
    let mut transcript = Transcript::new(events);
    transcript.replaying = true;
    transcript.window = chat.context.map(|context| context.window);
    if let Some(session) = &chat.agent_session
        && let Some(contents) = session_file(env, session).and_then(|path| fs::read_to_string(path).ok())
    {
        for entry in contents.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()) {
            if turn.is_some() && entry["uuid"].as_str() == turn.as_deref() {
                break;
            }
            transcript.recorded(&entry);
        }
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
    len: usize,
    streaming: HashMap<u64, (usize, String)>,
    status: Status,
    replaying: bool,
    /// The model of the main loop's latest API call, and how many tokens it was sent.
    call: Option<(String, u64)>,
    window: Option<u64>,
}

impl Transcript {
    fn new(events: UnboundedSender<Event>) -> Self {
        Self {
            events,
            len: 0,
            streaming: HashMap::new(),
            status: Status::Done,
            replaying: false,
            call: None,
            window: None,
        }
    }

    fn push(&mut self, item: Item) -> usize {
        let index = self.len;
        self.len += 1;
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

    fn recorded(&mut self, entry: &Value) {
        if entry["isSidechain"] == true || entry["isMeta"] == true {
            return;
        }
        match entry["type"].as_str() {
            Some("user") => {
                if let Some(text) = entry["message"]["content"].as_str() {
                    self.push(Item::User { text: text.to_owned(), images: 0 });
                }
            }
            Some("assistant") => {
                for block in entry["message"]["content"].as_array().into_iter().flatten() {
                    if block["type"] == "text" {
                        let text = block["text"].as_str().unwrap_or_default().to_owned();
                        self.push(Item::Text { text, streaming: false });
                    }
                }
            }
            _ => {}
        }
    }

    fn read(&mut self, message: &Value) {
        match message["type"].as_str() {
            Some("system") if message["subtype"] == "init" => {
                if let Some(session) = message["session_id"].as_str() {
                    let _ = self.events.send(Event::Session(session.to_owned()));
                }
            }
            Some("stream_event") if message["parent_tool_use_id"].is_null() => self.stream(&message["event"]),
            Some("result") => {
                self.total(&message["modelUsage"]);
                self.report(Status::Done);
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
