use std::{
    collections::{BTreeMap, HashMap},
    io,
    path::Path,
    process::Stdio,
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::{
    io::Join,
    process::{ChildStdin, ChildStdout, Command},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time::timeout,
};
use tokio_tungstenite::{WebSocketStream, client_async, tungstenite::Message};

use super::{Driver, Event, Item};
use crate::hooks::Status;

const CLOSING: Duration = Duration::from_secs(5);

pub struct Codex {
    requests: UnboundedSender<Request>,
}

enum Request {
    Send(String),
    Close,
}

impl Driver for Codex {
    fn send(&self, text: &str) {
        let _ = self.requests.send(Request::Send(text.to_owned()));
    }

    fn close(&self) {
        let _ = self.requests.send(Request::Close);
    }
}

pub async fn start(cwd: &Path, env: &BTreeMap<String, String>) -> io::Result<(Codex, UnboundedReceiver<Event>)> {
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
    let started = connection.call("thread/start", json!({ "cwd": cwd })).await?;
    let thread = started["thread"]["id"].as_str().ok_or_else(|| io::Error::other("thread/start gave no thread"))?;
    let thread = thread.to_owned();
    let (requests, mut pending) = mpsc::unbounded_channel();
    let (events, received) = mpsc::unbounded_channel();
    let _ = events.send(Event::Session(thread.clone()));
    tokio::spawn(async move {
        let mut transcript = Transcript { thread, events, len: 0, starting: None, turn: None, streaming: HashMap::new() };
        loop {
            tokio::select! {
                request = pending.recv() => match request {
                    Some(Request::Send(text)) => {
                        let _ = transcript.events.send(Event::Status(Status::Working));
                        let input = json!([{ "type": "text", "text": text, "text_elements": [] }]);
                        let turn = json!({ "threadId": transcript.thread, "input": input });
                        match connection.request("turn/start", turn).await {
                            Ok(id) => transcript.starting = Some(id),
                            Err(_) => break,
                        }
                    }
                    Some(Request::Close) | None => {
                        let _ = timeout(CLOSING, connection.close(&mut transcript)).await;
                        break;
                    }
                },
                message = connection.next() => match message {
                    Some(message) => transcript.read(&message),
                    None => break,
                },
            }
        }
        let _ = proxy.kill().await;
    });
    Ok((Codex { requests }, received))
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

    async fn close(&mut self, transcript: &mut Transcript) {
        while transcript.starting.is_some() {
            let Some(message) = self.next().await else { return };
            transcript.read(&message);
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
    events: UnboundedSender<Event>,
    len: usize,
    starting: Option<u64>,
    turn: Option<String>,
    streaming: HashMap<String, (usize, String)>,
}

impl Transcript {
    fn push(&mut self, item: Item) -> usize {
        let index = self.len;
        self.len += 1;
        let _ = self.events.send(Event::Item(index, item));
        index
    }

    fn read(&mut self, message: &Value) {
        if self.starting.is_some() && response(message) == self.starting {
            self.starting = None;
            match message["result"]["turn"]["id"].as_str() {
                Some(turn) => self.turn = Some(turn.to_owned()),
                None => {
                    eprintln!("codex turn/start failed: {}", message["error"]);
                    let _ = self.events.send(Event::Status(Status::Done));
                }
            }
            return;
        }
        let params = &message["params"];
        if params.get("threadId").is_some_and(|thread| thread != self.thread.as_str()) {
            return;
        }
        let item = &params["item"];
        match message["method"].as_str() {
            Some("turn/completed") => {
                self.turn = None;
                let _ = self.events.send(Event::Status(Status::Done));
            }
            Some("item/started") if item["type"] == "userMessage" => {
                let content = item["content"].as_array().into_iter().flatten();
                let text: Vec<&str> = content.filter_map(|input| input["text"].as_str()).collect();
                self.push(Item::User { text: text.join("\n"), images: 0 });
            }
            Some("item/started") if item["type"] == "agentMessage" => {
                let (Some(id), Some(text)) = (item["id"].as_str(), item["text"].as_str()) else { return };
                let index = self.push(Item::Text { text: text.to_owned(), streaming: true });
                self.streaming.insert(id.to_owned(), (index, text.to_owned()));
            }
            Some("item/agentMessage/delta") => {
                let Some((index, text)) = params["itemId"].as_str().and_then(|id| self.streaming.get_mut(id)) else {
                    return;
                };
                text.push_str(params["delta"].as_str().unwrap_or_default());
                let _ = self.events.send(Event::Item(*index, Item::Text { text: text.clone(), streaming: true }));
            }
            Some("item/completed") if item["type"] == "agentMessage" => {
                let Some((index, _)) = item["id"].as_str().and_then(|id| self.streaming.remove(id)) else { return };
                let text = item["text"].as_str().unwrap_or_default().to_owned();
                let _ = self.events.send(Event::Item(index, Item::Text { text, streaming: false }));
            }
            _ => {}
        }
    }
}
