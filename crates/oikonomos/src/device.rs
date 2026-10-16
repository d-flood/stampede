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
    sync::{broadcast::error::RecvError, mpsc, watch},
    task::{AbortHandle, JoinSet},
};
use uuid::Uuid;

use crate::{
    AppState,
    config::Config,
    holder::Owner,
    projects::{self, Projects},
    protocol::Size,
    terminals::{Attachment, Attention, Change, Terminals},
};

const OUTPUT: u8 = 1;
const INPUT: u8 = 2;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
enum Control {
    #[serde(rename = "hello")]
    Hello { device_id: String },
    #[serde(rename = "terminal.create")]
    Create { project_id: Option<String>, owner: Option<Owner>, rig: Option<String> },
    #[serde(rename = "terminal.close")]
    Close { terminal_id: String },
    #[serde(rename = "terminal.resume")]
    Resume { terminal_id: String },
    #[serde(rename = "terminal.attach")]
    Attach { terminal_id: String },
    #[serde(rename = "terminal.resize")]
    Resize { terminal_id: String, cols: u16, rows: u16 },
    #[serde(rename = "terminal.focus")]
    Focus { terminal_id: String },
    #[serde(rename = "terminal.detach")]
    Detach { channel: u32 },
    #[serde(rename = "projects.list")]
    ListProjects {},
    #[serde(rename = "project.open")]
    OpenProject { path: String },
    #[serde(rename = "project.close")]
    CloseProject { project_id: String },
    #[serde(rename = "project.reorder")]
    ReorderProjects { order: Vec<String> },
    #[serde(rename = "picker.list")]
    ListPicker {},
    #[serde(rename = "device.view")]
    View { terminal_id: Option<String> },
}

pub async fn upgrade(State(state): State<AppState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(|socket| connect(socket, state))
}

struct Device {
    id: String,
    connection: Uuid,
    terminals: Arc<Terminals>,
    projects: Arc<Projects>,
    config: watch::Receiver<Arc<Config>>,
    outgoing: mpsc::UnboundedSender<Message>,
    channels: HashMap<u32, (String, AbortHandle)>,
    next_channel: u32,
    attachments: JoinSet<()>,
}

async fn connect(socket: WebSocket, AppState { terminals, projects, mut config, disk }: AppState) {
    let (mut sink, mut stream) = socket.split();
    let (outgoing, mut pending) = mpsc::unbounded_channel();
    let writer = tokio::spawn(async move {
        while let Some(message) = pending.recv().await {
            if sink.send(message).await.is_err() {
                break;
            }
        }
    });
    let mut list = projects.subscribe();
    let mut changes = terminals.subscribe();
    let mut titles = terminals.subscribe_titles();
    let mut attention = terminals.subscribe_attention();
    let mut disk = disk.subscribe();
    let mut device = Device {
        id: Uuid::new_v4().to_string(),
        connection: Uuid::new_v4(),
        terminals,
        projects,
        config: config.clone(),
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
            Ok(()) = list.changed() => {
                let projects = list.borrow_and_update().clone();
                send(&device.outgoing, json!({ "type": "projects.changed", "projects": *projects }));
            }
            changed = changes.recv() => match changed {
                Ok(Change::Terminals(owner)) => device.terminals_changed(&owner),
                Ok(Change::Status(terminal_id, status)) => {
                    let status = json!({ "type": "terminal.status", "terminalId": terminal_id, "status": status });
                    send(&device.outgoing, status);
                }
                Err(RecvError::Lagged(_)) => {
                    for project in device.projects.list().iter() {
                        device.terminals_changed(&Owner::Project(project.id.clone()));
                    }
                    device.terminals_changed(&Owner::Drawer);
                }
                Err(RecvError::Closed) => break,
            },
            title = titles.recv() => match title {
                Ok((terminal_id, label)) => device.retitle(&terminal_id, &label),
                Err(RecvError::Lagged(_)) => {
                    for terminal in device.terminals.list() {
                        device.retitle(&terminal.id, &terminal.label);
                    }
                }
                Err(RecvError::Closed) => break,
            },
            changed = disk.recv() => match changed {
                Ok(changed) => send(&device.outgoing, json!(changed)),
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            },
            Ok(()) = attention.changed() => {
                let items = attention.borrow_and_update().clone();
                device.attention_changed(&items);
            }
        }
    }
    device.terminals.view(device.connection, None);
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
                    "terminals": self.terminals.list(),
                });
                send(&self.outgoing, hello);
                self.attention_changed(&self.terminals.attention());
            }
            Control::Create { project_id, owner, rig } => {
                let projects = self.projects.list();
                let project = match (project_id, owner) {
                    (Some(project_id), None) => {
                        let Some(project) = projects.iter().find(|project| project.id == project_id) else {
                            return eprintln!("cannot create a terminal in {project_id}: not open");
                        };
                        Some(project)
                    }
                    (None, Some(Owner::Drawer)) => None,
                    _ => return eprintln!("terminal.create needs a projectId or owner \"drawer\""),
                };
                let rig = match rig {
                    Some(name) => match self.config.borrow().rigs.iter().find(|rig| rig.name == name) {
                        Some(rig) => Some(rig.clone()),
                        None => return eprintln!("cannot launch rig {name}: not in the config"),
                    },
                    None => None,
                };
                let scrollback = self.config.borrow().scrollback;
                match self.terminals.create(project, rig.as_ref(), None, scrollback, &self.id).await {
                    Ok(terminal) => {
                        send(&self.outgoing, json!({ "type": "terminal.created", "terminal": terminal }))
                    }
                    Err(error) => eprintln!("cannot create a terminal: {error}"),
                }
            }
            Control::Close { terminal_id } => self.terminals.close(&terminal_id),
            Control::Resume { terminal_id } => self.resume(&terminal_id).await,
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
            Control::ListProjects {} => {
                let projects = self.projects.list();
                send(&self.outgoing, json!({ "type": "projects.changed", "projects": *projects }));
            }
            Control::OpenProject { path } => match self.projects.open(&path) {
                Ok(project_id) => {
                    send(&self.outgoing, json!({ "type": "project.opened", "projectId": project_id }))
                }
                Err(error) => {
                    send(&self.outgoing, json!({ "type": "project.error", "path": path, "error": error }))
                }
            },
            Control::CloseProject { project_id } => {
                for terminal in self.terminals.owned(&Owner::Project(project_id.clone())) {
                    self.terminals.close(&terminal.id);
                }
                self.projects.close(&project_id);
            }
            Control::ReorderProjects { order } => self.projects.reorder(&order),
            Control::ListPicker {} => {
                let roots = self.config.borrow().roots.clone();
                let directories = projects::directories(&roots);
                send(&self.outgoing, json!({ "type": "picker.listed", "directories": directories }));
            }
            Control::View { terminal_id } => self.terminals.view(self.connection, terminal_id),
        }
    }

    async fn resume(&self, terminal_id: &str) {
        let Some((ended, _)) = self.terminals.ended(terminal_id) else { return };
        let (Some(name), Some(session)) = (ended.rig, ended.agent_session) else {
            return eprintln!("cannot resume terminal {terminal_id}: no Agent session");
        };
        let Some(rig) = self.config.borrow().rigs.iter().find(|rig| rig.name == name).cloned() else {
            return eprintln!("cannot resume rig {name}: not in the config");
        };
        let projects = self.projects.list();
        let project = match &ended.owner {
            Owner::Project(project_id) => match projects.iter().find(|project| project.id == *project_id) {
                Some(project) => Some(project),
                None => return eprintln!("cannot resume terminal {terminal_id} in {project_id}: not open"),
            },
            Owner::Drawer => None,
        };
        let scrollback = self.config.borrow().scrollback;
        match self.terminals.create(project, Some(&rig), Some(&session), scrollback, &self.id).await {
            Ok(terminal) => {
                self.terminals.dismiss(terminal_id);
                send(&self.outgoing, json!({ "type": "terminal.created", "terminal": terminal }));
            }
            Err(error) => eprintln!("cannot resume terminal {terminal_id}: {error}"),
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

    fn terminals_changed(&self, owner: &Owner) {
        let terminals = self.terminals.owned(owner);
        let changed = match owner {
            Owner::Project(project_id) => json!({
                "type": "terminals.changed",
                "projectId": project_id,
                "terminals": terminals,
            }),
            Owner::Drawer => json!({
                "type": "terminals.changed",
                "drawer": true,
                "terminals": terminals,
            }),
        };
        send(&self.outgoing, changed);
    }

    fn attention_changed(&self, items: &[Attention]) {
        send(&self.outgoing, json!({ "type": "attention.changed", "items": items }));
    }

    fn retitle(&self, terminal_id: &str, label: &str) {
        send(&self.outgoing, json!({ "type": "terminal.title", "terminalId": terminal_id, "label": label }));
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
