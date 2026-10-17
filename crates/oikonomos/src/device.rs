use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

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
    attention::{Attention, Feed},
    chats::{self, Builtin, Chats},
    config::{Agent, Config},
    holder::Owner,
    language_servers::{self, Input, LanguageServers, Server, Switch},
    projects::{self, Projects},
    protocol::Size,
    search,
    terminals::{Attachment, Terminals},
    usage::{Reading, Usage},
};

const OUTPUT: u8 = 1;
const INPUT: u8 = 2;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
enum Control {
    #[serde(rename = "hello")]
    Hello { device_id: String, browser_id: Option<String> },
    #[serde(rename = "terminal.create")]
    Create { project_id: Option<String>, owner: Option<Owner> },
    #[serde(rename = "terminal.close")]
    Close { terminal_id: String },
    #[serde(rename = "terminal.attach")]
    Attach { terminal_id: String },
    #[serde(rename = "terminal.resize")]
    Resize { terminal_id: String, cols: u16, rows: u16 },
    #[serde(rename = "terminal.focus")]
    Focus { terminal_id: String },
    #[serde(rename = "terminal.detach")]
    Detach { channel: u32 },
    #[serde(rename = "chat.create")]
    CreateChat { project_id: String, rig: String },
    #[serde(rename = "chat.resume")]
    ResumeChat { project_id: String, agent: Agent, session_id: String },
    #[serde(rename = "sessions.list")]
    ListSessions { project_id: String },
    #[serde(rename = "chat.send")]
    SendChat {
        chat_id: String,
        text: String,
        #[serde(default)]
        queue: bool,
        #[serde(default)]
        images: Vec<chats::Image>,
    },
    #[serde(rename = "chat.interrupt")]
    InterruptChat { chat_id: String },
    #[serde(rename = "chat.answer")]
    AnswerChat { chat_id: String, request_id: String, answer: chats::Answer },
    #[serde(rename = "chat.run")]
    RunChat {
        chat_id: String,
        #[serde(flatten)]
        builtin: Builtin,
    },
    #[serde(rename = "chat.close")]
    CloseChat { chat_id: String },
    #[serde(rename = "chat.attach")]
    AttachChat { chat_id: String },
    #[serde(rename = "chat.detach")]
    DetachChat { chat_id: String },
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
    View {
        #[serde(default)]
        chat_ids: Vec<String>,
    },
    #[serde(rename = "search.start")]
    Search(search::Query),
    #[serde(rename = "lsp.switch")]
    Switch { project_id: String, on: bool },
    #[serde(rename = "lsp.open")]
    OpenServer { channel: String },
    #[serde(rename = "lsp.close")]
    CloseServer { channel: String },
    #[serde(rename = "lsp")]
    Lsp { channel: String, message: String },
}

pub async fn upgrade(State(state): State<AppState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(|socket| connect(socket, state))
}

struct Device {
    id: String,
    browser: Option<String>,
    connection: Uuid,
    terminals: Arc<Terminals>,
    chats: Arc<Chats>,
    attached: HashMap<String, u64>,
    attention: Arc<Feed>,
    usage: Arc<Usage>,
    projects: Arc<Projects>,
    config: watch::Receiver<Arc<Config>>,
    outgoing: mpsc::UnboundedSender<Message>,
    channels: HashMap<u32, (String, AbortHandle)>,
    next_channel: u32,
    attachments: JoinSet<()>,
    search: Option<Arc<AtomicBool>>,
    languages: Arc<LanguageServers>,
    servers: HashMap<String, Server>,
}

async fn connect(
    socket: WebSocket,
    AppState { terminals, chats, attention, usage, projects, mut config, disk, git, languages, .. }: AppState,
) {
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
    let mut chat_changes = chats.subscribe();
    let mut attention_changes = attention.subscribe();
    let mut usage_changes = usage.subscribe();
    let mut disk = disk.subscribe();
    let mut git = git.subscribe();
    let mut switches = languages.subscribe();
    let mut device = Device {
        id: Uuid::new_v4().to_string(),
        browser: None,
        connection: Uuid::new_v4(),
        terminals,
        chats,
        attached: HashMap::new(),
        attention,
        usage,
        projects,
        config: config.clone(),
        outgoing,
        channels: HashMap::new(),
        next_channel: 1,
        attachments: JoinSet::new(),
        search: None,
        languages,
        servers: HashMap::new(),
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
                device.stop_servers(|project_id| !projects.iter().any(|project| project.id == project_id));
            }
            changed = changes.recv() => match changed {
                Ok(owner) => device.terminals_changed(&owner),
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
            changed = chat_changes.recv() => match changed {
                Ok(chats::Change::Chats(project_id)) => device.chats_changed(&project_id),
                Ok(chats::Change::Status(chat_id, status)) => {
                    send(&device.outgoing, json!({ "type": "chat.status", "chatId": chat_id, "status": status }));
                }
                Ok(chats::Change::Truncated(chat_id)) => {
                    if device.attached.contains_key(&chat_id) {
                        device.attach_chat(chat_id);
                    }
                }
                Ok(chats::Change::Queued(chat_id, queued)) => {
                    send(&device.outgoing, json!({ "type": "chat.queued", "chatId": chat_id, "queued": queued }));
                }
                Ok(chats::Change::Item { chat_id, seq, index, item }) => {
                    if device.attached.get(&chat_id).is_some_and(|&attached| seq > attached) {
                        send(&device.outgoing, json!({ "type": "chat.item", "chatId": chat_id, "index": index, "item": item }));
                    }
                }
                Ok(chats::Change::Request { chat_id, seq, request }) => {
                    if device.attached.get(&chat_id).is_some_and(|&attached| seq > attached) {
                        send(&device.outgoing, json!({ "type": "chat.request", "chatId": chat_id, "request": request }));
                    }
                }
                Ok(chats::Change::Resolved { chat_id, seq, request_id }) => {
                    if device.attached.get(&chat_id).is_some_and(|&attached| seq > attached) {
                        send(&device.outgoing, json!({ "type": "chat.resolved", "chatId": chat_id, "requestId": request_id }));
                    }
                }
                Ok(chats::Change::Meters { chat_id, context, tokens }) => {
                    let meters = json!({ "type": "chat.meters", "chatId": chat_id, "context": context, "tokens": tokens });
                    send(&device.outgoing, meters);
                }
                Ok(chats::Change::Todos { chat_id, todos }) => {
                    send(&device.outgoing, json!({ "type": "chat.todos", "chatId": chat_id, "todos": todos }));
                }
                Ok(chats::Change::Settings(chat_id, settings)) => {
                    send(&device.outgoing, json!({ "type": "chat.settings", "chatId": chat_id, "settings": settings }));
                }
                Ok(chats::Change::Capabilities(chat_id, capabilities)) => {
                    if device.attached.contains_key(&chat_id) {
                        let message = json!({ "type": "chat.capabilities", "chatId": chat_id, "capabilities": capabilities });
                        send(&device.outgoing, message);
                    }
                }
                Err(RecvError::Lagged(_)) => {
                    for project in device.projects.list().iter() {
                        device.chats_changed(&project.id);
                    }
                    for chat_id in device.attached.keys().cloned().collect::<Vec<_>>() {
                        device.attach_chat(chat_id);
                    }
                }
                Err(RecvError::Closed) => break,
            },
            Ok(()) = attention_changes.changed() => {
                let items = attention_changes.borrow_and_update().clone();
                device.attention_changed(&items);
            }
            Ok(()) = usage_changes.changed() => {
                let readings = usage_changes.borrow_and_update().clone();
                device.usage_changed(&readings);
            }
            changed = git.recv() => match changed {
                Ok(changed) => send(&device.outgoing, json!(changed)),
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            },
            switch = switches.recv() => match switch {
                Ok(switch) => device.switched(switch),
                Err(RecvError::Lagged(_)) => device.send_switches(),
                Err(RecvError::Closed) => break,
            },
        }
    }
    device.chats.view(device.connection, None, Vec::new());
    device.cancel_search();
    writer.abort();
}

impl Device {
    async fn control(&mut self, control: Control) {
        match control {
            Control::Hello { device_id, browser_id } => {
                self.id = device_id;
                self.browser = browser_id;
                let hello = json!({
                    "type": "hello",
                    "version": env!("CARGO_PKG_VERSION"),
                    "buildId": env!("OIKONOMOS_BUILD_ID"),
                    "terminals": self.terminals.list(),
                    "chats": self.chats.list(),
                });
                send(&self.outgoing, hello);
                self.attention_changed(&self.attention.items());
                self.usage_changed(&self.usage.readings());
                self.send_switches();
            }
            Control::Create { project_id, owner } => {
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
                let scrollback = self.config.borrow().scrollback;
                match self.terminals.create(project, scrollback, &self.id).await {
                    Ok(terminal) => {
                        send(&self.outgoing, json!({ "type": "terminal.created", "terminal": terminal }))
                    }
                    Err(error) => eprintln!("cannot create a terminal: {error}"),
                }
            }
            Control::Close { terminal_id } => self.terminals.close(&terminal_id),
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
            Control::CreateChat { project_id, rig } => {
                let projects = self.projects.list();
                let Some(project) = projects.iter().find(|project| project.id == project_id) else {
                    return eprintln!("cannot create a chat in {project_id}: not open");
                };
                let Some(rig) = self.config.borrow().rigs.iter().find(|other| other.name == rig).cloned() else {
                    return eprintln!("cannot launch rig {rig} as a chat: not in the config");
                };
                match self.chats.create(project, &rig, None).await {
                    Ok(chat) => send(&self.outgoing, json!({ "type": "chat.created", "chat": chat })),
                    Err(error) => eprintln!("cannot create a chat: {error}"),
                }
            }
            Control::ResumeChat { project_id, agent, session_id } => {
                let projects = self.projects.list();
                let Some(project) = projects.iter().find(|project| project.id == project_id) else {
                    return eprintln!("cannot resume a chat in {project_id}: not open");
                };
                let rig = self.config.borrow().rig_for(agent);
                match self.chats.create(project, &rig, Some(&session_id)).await {
                    Ok(chat) => send(&self.outgoing, json!({ "type": "chat.created", "chat": chat })),
                    Err(error) => eprintln!("cannot resume session {session_id}: {error}"),
                }
            }
            Control::ListSessions { project_id } => self.list_sessions(project_id),
            Control::SendChat { chat_id, text, queue, images } => self.chats.send(&chat_id, &text, queue, &images),
            Control::InterruptChat { chat_id } => self.chats.interrupt(&chat_id),
            Control::AnswerChat { chat_id, request_id, answer } => self.chats.answer(&chat_id, &request_id, answer),
            Control::RunChat { chat_id, builtin } => self.chats.run(&chat_id, builtin),
            Control::CloseChat { chat_id } => self.chats.close(&chat_id),
            Control::AttachChat { chat_id } => self.attach_chat(chat_id),
            Control::DetachChat { chat_id } => {
                self.attached.remove(&chat_id);
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
                for chat in self.chats.owned(&project_id) {
                    self.chats.close(&chat.id);
                }
                self.projects.close(&project_id);
            }
            Control::ReorderProjects { order } => self.projects.reorder(&order),
            Control::ListPicker {} => {
                let roots = self.config.borrow().roots.clone();
                let directories = projects::directories(&roots);
                send(&self.outgoing, json!({ "type": "picker.listed", "directories": directories }));
            }
            Control::View { chat_ids } => {
                self.chats.view(self.connection, self.browser.clone(), chat_ids);
            }
            Control::Search(query) => self.search(query),
            Control::Switch { project_id, on } => self.languages.switch(&project_id, on),
            Control::OpenServer { channel } => self.open_server(channel),
            Control::CloseServer { channel } => {
                if let Some(server) = self.servers.get(&channel) {
                    server.send(Input::Hidden);
                }
            }
            Control::Lsp { channel, message } => {
                if let Some(server) = self.servers.get(&channel) {
                    server.send(Input::Message(message));
                }
            }
        }
    }

    fn open_server(&mut self, channel: String) {
        if let Some(server) = self.servers.get(&channel)
            && server.running()
        {
            return server.send(Input::Shown);
        }
        let Some((project_id, _)) = language_servers::parse(&channel) else { return };
        if !self.languages.is_on(project_id) {
            return;
        }
        let projects = self.projects.list();
        let Some(project) = projects.iter().find(|project| project.id == project_id) else { return };
        let pins = self.config.borrow().language_servers.pins.clone();
        match self.languages.start(&channel, &project.path, pins, self.outgoing.clone()) {
            Some(server) => {
                self.servers.insert(channel, server);
            }
            None => eprintln!("no language server for {channel}"),
        }
    }

    fn stop_servers(&mut self, stop: impl Fn(&str) -> bool) {
        self.servers.retain(|channel, _| {
            let keep = !language_servers::parse(channel).is_some_and(|(project_id, _)| stop(project_id));
            if !keep {
                send(&self.outgoing, json!({ "type": "lsp.state", "channel": channel, "state": "off" }));
            }
            keep
        });
    }

    fn switched(&mut self, switch: Switch) {
        send(&self.outgoing, json!(switch));
        if !switch.on {
            self.stop_servers(|project_id| project_id == switch.project_id);
        }
    }

    fn send_switches(&self) {
        for project in self.projects.list().iter() {
            send(&self.outgoing, json!(Switch { project_id: project.id.clone(), on: self.languages.is_on(&project.id) }));
        }
    }

    fn search(&mut self, query: search::Query) {
        self.cancel_search();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.search = Some(Arc::clone(&cancelled));
        let search_id = query.search_id.clone();
        send(&self.outgoing, json!({ "type": "search.started", "searchId": search_id }));
        let projects = self.projects.list();
        let Some(project) = projects.iter().find(|project| project.id == query.project_id) else {
            let error = format!("Project {} is not open", query.project_id);
            let done = json!({ "type": "search.done", "searchId": search_id, "truncated": false, "error": error });
            return send(&self.outgoing, done);
        };
        let root = PathBuf::from(&project.path);
        let outgoing = self.outgoing.clone();
        tokio::task::spawn_blocking(move || {
            let found = |file: &str, matches| {
                let results = json!({ "type": "search.results", "searchId": search_id, "file": file, "matches": matches });
                send(&outgoing, results);
            };
            let done = match search::run(&root, &query, &cancelled, found) {
                Ok(truncated) => json!({ "type": "search.done", "searchId": search_id, "truncated": truncated }),
                Err(error) => json!({ "type": "search.done", "searchId": search_id, "truncated": false, "error": error }),
            };
            if !cancelled.load(Ordering::Relaxed) {
                send(&outgoing, done);
            }
        });
    }

    fn cancel_search(&self) {
        if let Some(cancelled) = &self.search {
            cancelled.store(true, Ordering::Relaxed);
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

    fn attach_chat(&mut self, chat_id: String) {
        let Some((seq, items, requests)) = self.chats.attach(&chat_id) else { return };
        send(&self.outgoing, json!({ "type": "chat.items", "chatId": chat_id, "items": items }));
        for request in requests {
            send(&self.outgoing, json!({ "type": "chat.request", "chatId": chat_id, "request": request }));
        }
        let capabilities = self.chats.capabilities(&chat_id).unwrap_or_default();
        send(&self.outgoing, json!({ "type": "chat.capabilities", "chatId": chat_id, "capabilities": capabilities }));
        self.attached.insert(chat_id, seq);
    }

    fn list_sessions(&self, project_id: String) {
        let projects = self.projects.list();
        let Some(project) = projects.iter().find(|project| project.id == project_id) else {
            return eprintln!("cannot list sessions in {project_id}: not open");
        };
        let cwd = PathBuf::from(&project.path);
        let config = self.config.borrow();
        let (claude, codex) = (config.rig_for(Agent::ClaudeCode), config.rig_for(Agent::Codex));
        drop(config);
        let chats = self.chats.list().into_iter().map(|chat| chat.agent_session);
        let live: HashSet<String> = chats.flatten().collect();
        let outgoing = self.outgoing.clone();
        tokio::spawn(async move {
            let mut sessions = chats::sessions(&cwd, &claude, &codex).await;
            sessions.retain(|session| !live.contains(&session.session_id));
            send(&outgoing, json!({ "type": "sessions.listed", "projectId": project_id, "sessions": sessions }));
        });
    }

    fn chats_changed(&self, project_id: &str) {
        let chats = self.chats.owned(project_id);
        send(&self.outgoing, json!({ "type": "chats.changed", "projectId": project_id, "chats": chats }));
    }

    fn attention_changed(&self, items: &[Attention]) {
        send(&self.outgoing, json!({ "type": "attention.changed", "items": items }));
    }

    fn usage_changed(&self, readings: &[Reading]) {
        send(&self.outgoing, json!({ "type": "usage.changed", "readings": readings }));
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

pub(crate) fn send(outgoing: &mpsc::UnboundedSender<Message>, message: Value) {
    let _ = outgoing.send(Message::Text(message.to_string().into()));
}
