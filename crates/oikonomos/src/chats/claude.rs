use std::{collections::{BTreeMap, HashMap}, io, path::Path};

use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::{
    io::AsyncWriteExt,
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use uuid::Uuid;

use super::{
    Driver, Event, Item, holder,
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
    let (mut frames, mut writer) = holder::connect(data_dir, id).await?;
    let initialize = json!({
        "type": "control_request",
        "request_id": Uuid::new_v4().to_string(),
        "request": { "subtype": "initialize" },
    });
    writer.write_all(&input(&initialize)).await?;
    let (commands, mut pending) = mpsc::unbounded_channel();
    let (events, received) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut transcript = Transcript { events, len: 0, streaming: HashMap::new() };
        loop {
            tokio::select! {
                command = pending.recv() => match command {
                    Some(Command::Send(text)) => {
                        transcript.push(Item::User { text: text.clone(), images: 0 });
                        let _ = transcript.events.send(Event::Status(Status::Working));
                        let message = json!({
                            "type": "user",
                            "message": { "role": "user", "content": text },
                            "parent_tool_use_id": null,
                        });
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
    Ok((Claude { commands }, received))
}

fn input(message: &Value) -> Vec<u8> {
    protocol::encode(&Message::Input(message.to_string().into_bytes()))
}

struct Transcript {
    events: UnboundedSender<Event>,
    len: usize,
    streaming: HashMap<u64, (usize, String)>,
}

impl Transcript {
    fn push(&mut self, item: Item) -> usize {
        let index = self.len;
        self.len += 1;
        let _ = self.events.send(Event::Item(index, item));
        index
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
                let _ = self.events.send(Event::Status(Status::Done));
            }
            _ => {}
        }
    }

    fn stream(&mut self, event: &Value) {
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
