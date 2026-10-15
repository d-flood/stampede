use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{sync::mpsc, task::JoinSet};

use crate::{
    protocol::Size,
    terminals::{Attachment, Terminals},
};

const OUTPUT: u8 = 1;
const INPUT: u8 = 2;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
enum Control {
    #[serde(rename = "hello")]
    Hello { device_id: String },
    #[serde(rename = "terminal.create")]
    Create {},
    #[serde(rename = "terminal.attach")]
    Attach { terminal_id: String },
    #[serde(rename = "terminal.resize")]
    Resize { terminal_id: String, cols: u16, rows: u16 },
    #[serde(rename = "terminal.focus")]
    Focus { terminal_id: String },
}

pub async fn upgrade(
    State(terminals): State<Arc<Terminals>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    upgrade.on_upgrade(|socket| connect(socket, terminals))
}

struct Device {
    id: String,
    terminals: Arc<Terminals>,
    outgoing: mpsc::UnboundedSender<Message>,
    channels: HashMap<u32, String>,
    attachments: JoinSet<()>,
}

async fn connect(socket: WebSocket, terminals: Arc<Terminals>) {
    let (mut sink, mut stream) = socket.split();
    let (outgoing, mut pending) = mpsc::unbounded_channel();
    let writer = tokio::spawn(async move {
        while let Some(message) = pending.recv().await {
            if sink.send(message).await.is_err() {
                break;
            }
        }
    });
    let mut device = Device {
        id: uuid::Uuid::new_v4().to_string(),
        terminals,
        outgoing,
        channels: HashMap::new(),
        attachments: JoinSet::new(),
    };
    while let Some(Ok(message)) = stream.next().await {
        match message {
            Message::Text(text) => {
                if let Ok(control) = serde_json::from_str(&text) {
                    device.control(control).await;
                }
            }
            Message::Binary(bytes) => device.binary(&bytes),
            Message::Close(_) => break,
            _ => {}
        }
    }
    writer.abort();
}

impl Device {
    async fn control(&mut self, control: Control) {
        match control {
            Control::Hello { device_id } => {
                self.id = device_id;
                let hello = json!({
                    "type": "hello",
                    "version": env!("CARGO_PKG_VERSION"),
                    "terminals": self.terminals.ids(),
                });
                send(&self.outgoing, hello);
            }
            Control::Create {} => match self.terminals.create(&self.id).await {
                Ok(terminal_id) => self.attach(terminal_id).await,
                Err(error) => eprintln!("cannot create a terminal: {error}"),
            },
            Control::Attach { terminal_id } => self.attach(terminal_id).await,
            Control::Resize { terminal_id, cols, rows } => {
                self.terminals.resize(&terminal_id, &self.id, Size { cols, rows })
            }
            Control::Focus { terminal_id } => self.terminals.lead(&terminal_id, &self.id),
        }
    }

    async fn attach(&mut self, terminal_id: String) {
        let Attachment { mut size, mut output } = match self.terminals.attach(&terminal_id).await {
            Ok(attachment) => attachment,
            Err(error) => return eprintln!("cannot attach terminal {terminal_id}: {error}"),
        };
        let channel = self.channels.len() as u32 + 1;
        self.channels.insert(channel, terminal_id.clone());
        let Size { cols, rows } = *size.borrow_and_update();
        let attached = json!({
            "type": "terminal.attached",
            "terminalId": terminal_id,
            "channel": channel,
            "cols": cols,
            "rows": rows,
        });
        send(&self.outgoing, attached);
        let outgoing = self.outgoing.clone();
        self.attachments.spawn(async move {
            loop {
                tokio::select! {
                    bytes = output.next() => {
                        let Some(bytes) = bytes else { break };
                        let mut frame = Vec::with_capacity(5 + bytes.len());
                        frame.push(OUTPUT);
                        frame.extend_from_slice(&channel.to_be_bytes());
                        frame.extend_from_slice(&bytes);
                        let _ = outgoing.send(Message::Binary(frame.into()));
                    }
                    Ok(()) = size.changed() => {
                        let Size { cols, rows } = *size.borrow_and_update();
                        let resized = json!({
                            "type": "terminal.resized",
                            "terminalId": terminal_id,
                            "cols": cols,
                            "rows": rows,
                        });
                        send(&outgoing, resized);
                    }
                }
            }
        });
    }

    fn binary(&self, bytes: &[u8]) {
        let [kind, c0, c1, c2, c3, payload @ ..] = bytes else { return };
        let channel = u32::from_be_bytes([*c0, *c1, *c2, *c3]);
        if *kind == INPUT
            && let Some(terminal_id) = self.channels.get(&channel)
        {
            self.terminals.input(terminal_id, &self.id, payload.to_vec());
        }
    }
}

fn send(outgoing: &mpsc::UnboundedSender<Message>, message: Value) {
    let _ = outgoing.send(Message::Text(message.to_string().into()));
}
