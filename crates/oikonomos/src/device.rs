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
use tokio::{
    sync::mpsc,
    task::{AbortHandle, JoinSet},
};

use crate::{
    AppState,
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
    #[serde(rename = "terminal.detach")]
    Detach { channel: u32 },
}

pub async fn upgrade(State(state): State<AppState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(|socket| connect(socket, state))
}

struct Device {
    id: String,
    terminals: Arc<Terminals>,
    outgoing: mpsc::UnboundedSender<Message>,
    channels: HashMap<u32, (String, AbortHandle)>,
    next_channel: u32,
    attachments: JoinSet<()>,
}

async fn connect(socket: WebSocket, AppState { terminals, mut config }: AppState) {
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
        next_channel: 1,
        attachments: JoinSet::new(),
    };
    config.mark_unchanged();
    loop {
        tokio::select! {
            message = stream.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(control) = serde_json::from_str(&text) {
                        device.control(control).await;
                    }
                }
                Some(Ok(Message::Binary(bytes))) => device.binary(&bytes),
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            Ok(()) = config.changed() => {
                config.mark_unchanged();
                send(&device.outgoing, json!({ "type": "config.changed" }));
            }
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
            Control::Detach { channel } => {
                if let Some((_, attachment)) = self.channels.remove(&channel) {
                    attachment.abort();
                }
            }
        }
    }

    async fn attach(&mut self, terminal_id: String) {
        let Attachment { mut size, mut output } = match self.terminals.attach(&terminal_id).await {
            Ok(attachment) => attachment,
            Err(error) => return eprintln!("cannot attach terminal {terminal_id}: {error}"),
        };
        let channel = self.next_channel;
        self.next_channel += 1;
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
        let terminal = terminal_id.clone();
        let attachment = self.attachments.spawn(async move {
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
        self.channels.insert(channel, (terminal, attachment));
    }

    fn binary(&self, bytes: &[u8]) {
        let [kind, c0, c1, c2, c3, payload @ ..] = bytes else { return };
        let channel = u32::from_be_bytes([*c0, *c1, *c2, *c3]);
        if *kind == INPUT
            && let Some((terminal_id, _)) = self.channels.get(&channel)
        {
            self.terminals.input(terminal_id, &self.id, payload.to_vec());
        }
    }
}

fn send(outgoing: &mpsc::UnboundedSender<Message>, message: Value) {
    let _ = outgoing.send(Message::Text(message.to_string().into()));
}
