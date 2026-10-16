use std::{
    collections::HashMap,
    env, fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};

use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use tokio::{
    io::AsyncWriteExt,
    net::{
        UnixStream,
        unix::{OwnedReadHalf, OwnedWriteHalf},
    },
    process::Command,
    sync::{broadcast, mpsc, watch},
};
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};

use crate::{
    config::{Agent, Rig},
    holder::{self, Owner, Record},
    hooks::{self, Status, Transition},
    projects::Project,
    protocol::{self, AgentState, Foreground, Message, Size},
};

type Frames = FramedRead<OwnedReadHalf, LengthDelimitedCodec>;

pub struct Terminals {
    data_dir: PathBuf,
    hook_url: String,
    terminals: Mutex<Vec<(String, Arc<Entry>)>>,
    changed: broadcast::Sender<Change>,
}

#[derive(Clone)]
pub enum Change {
    Terminals(Owner),
    Status(String, Status),
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Terminal {
    pub id: String,
    pub owner: Owner,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rig: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<Agent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_session: Option<String>,
}

struct Entry {
    terminal: Mutex<Terminal>,
    control: mpsc::UnboundedSender<Message>,
    size: watch::Sender<Size>,
    lead: Mutex<Lead>,
}

#[derive(Default)]
struct Lead {
    device: Option<String>,
    sizes: HashMap<String, Size>,
}

pub struct Attachment {
    pub size: watch::Receiver<Size>,
    pub output: Output,
}

pub struct Output {
    frames: Frames,
    _writer: OwnedWriteHalf,
}

impl Output {
    pub async fn next(&mut self) -> Option<Vec<u8>> {
        while let Some(Ok(frame)) = self.frames.next().await {
            if let Ok(Some(Message::Snapshot(bytes) | Message::Output(bytes))) =
                protocol::decode(&frame)
            {
                return Some(bytes);
            }
        }
        None
    }
}

impl Terminals {
    pub fn new(data_dir: PathBuf, hook_url: String) -> Self {
        Self {
            data_dir,
            hook_url,
            terminals: Mutex::default(),
            changed: broadcast::Sender::new(64),
        }
    }

    pub async fn restore(self: &Arc<Self>) -> io::Result<()> {
        let entries = match fs::read_dir(self.data_dir.join("terminals")) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            entries => entries?,
        };
        let mut records = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path.extension() != Some("json".as_ref()) {
                continue;
            }
            match read_record(&path) {
                Ok(record) => records.push((path, record)),
                Err(error) => eprintln!("cannot read {}: {error}", path.display()),
            }
        }
        records.sort_by_key(|(_, record)| record.created);
        for (path, record) in records {
            let id = record.id.clone();
            match self.open(record).await {
                Ok(_) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                    ) =>
                {
                    let _ = fs::remove_file(holder::socket_path(&self.data_dir, &id));
                    let _ = fs::remove_file(&path);
                }
                Err(error) => eprintln!("cannot reattach terminal {id}: {error}"),
            }
        }
        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Change> {
        self.changed.subscribe()
    }

    pub fn list(&self) -> Vec<Terminal> {
        let terminals = self.terminals.lock().unwrap();
        terminals.iter().map(|(_, entry)| entry.terminal.lock().unwrap().clone()).collect()
    }

    pub fn owned(&self, owner: &Owner) -> Vec<Terminal> {
        self.list().into_iter().filter(|terminal| terminal.owner == *owner).collect()
    }

    pub async fn create(
        self: &Arc<Self>,
        project: Option<&Project>,
        rig: Option<&Rig>,
        device: &str,
    ) -> io::Result<Terminal> {
        let id = format!("{:016x}", uuid::Uuid::new_v4().as_u64_pair().1);
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        let exe = env::current_exe()?;
        let mut command = Command::new(&exe);
        command.env("OIKONOMOS_HOOK_URL", &self.hook_url);
        command.arg("hold").arg("--data-dir").arg(&self.data_dir).arg("--id").arg(&id);
        match project {
            Some(project) => command.arg("--cwd").arg(&project.path).arg("--project").arg(&project.id),
            None => command.arg("--cwd").arg(crate::home()),
        };
        if let Some(rig) = rig {
            command.arg("--rig").arg(&rig.name).arg("--agent").arg(rig.agent.name()).envs(&rig.env);
        }
        command.arg("--").arg(&shell);
        match rig {
            Some(rig) => command.args(["-l", "-i", "-c"]).arg(launch(rig, &exe, &shell.to_string_lossy())),
            None => command.arg("-l"),
        };
        let status = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !status.success() {
            return Err(io::Error::other(format!("holder {status}")));
        }
        let record = read_record(&holder::record_path(&self.data_dir, &id))?;
        let terminal = self.open(record).await?;
        self.lead(&id, device);
        Ok(terminal)
    }

    async fn open(self: &Arc<Self>, record: Record) -> io::Result<Terminal> {
        let (mut frames, mut writer) = connect(&self.data_dir, &record.id).await?;
        let (control, mut pending) = mpsc::unbounded_channel();
        let terminal = Terminal {
            id: record.id,
            owner: record.owner,
            status: record.rig.as_ref().map(|_| record.status.unwrap_or(Status::Done)),
            agent_session: record.agent_session,
            rig: record.rig,
            agent: record.agent,
        };
        let entry = Arc::new(Entry {
            terminal: Mutex::new(terminal.clone()),
            control,
            size: watch::Sender::new(record.size),
            lead: Mutex::default(),
        });
        self.terminals.lock().unwrap().push((terminal.id.clone(), Arc::clone(&entry)));
        let _ = self.changed.send(Change::Terminals(terminal.owner.clone()));
        let terminals = Arc::clone(self);
        let (id, owner) = (terminal.id.clone(), terminal.owner.clone());
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(message) = pending.recv() => {
                        if writer.write_all(&protocol::encode(&message)).await.is_err() {
                            break;
                        }
                    }
                    frame = frames.next() => {
                        let Some(Ok(frame)) = frame else { break };
                        if let Ok(Some(Message::ForegroundChanged(Foreground { is_root_shell: true }))) =
                            protocol::decode(&frame)
                            && entry.agent_exited()
                        {
                            let _ = terminals.changed.send(Change::Terminals(owner.clone()));
                        }
                    }
                }
            }
            terminals.terminals.lock().unwrap().retain(|(other, _)| *other != id);
            let _ = terminals.changed.send(Change::Terminals(owner));
        });
        Ok(terminal)
    }

    pub async fn attach(&self, id: &str) -> io::Result<Attachment> {
        let entry = self.get(id).ok_or(io::ErrorKind::NotFound)?;
        let (frames, mut writer) = connect(&self.data_dir, id).await?;
        writer.write_all(&protocol::encode(&Message::Attach)).await?;
        Ok(Attachment {
            size: entry.size.subscribe(),
            output: Output {
                frames,
                _writer: writer,
            },
        })
    }

    pub fn input(&self, id: &str, device: &str, bytes: Vec<u8>) {
        self.lead(id, device);
        if let Some(entry) = self.get(id) {
            let _ = entry.control.send(Message::Input(bytes));
        }
    }

    pub fn hook(&self, id: &str, agent: Agent, event: &str, payload: &Value) {
        let Some(entry) = self.get(id) else { return };
        let mut terminal = entry.terminal.lock().unwrap();
        if terminal.agent != Some(agent) {
            return;
        }
        let before = (terminal.status, terminal.agent_session.clone());
        if let Some(session) = payload["session_id"].as_str() {
            terminal.agent_session = Some(session.to_owned());
        }
        match hooks::transition(agent, event, payload) {
            Some(Transition::To(now)) => {
                terminal.status = Some(now);
                if Some(now) != before.0 {
                    let _ = self.changed.send(Change::Status(terminal.id.clone(), now));
                }
            }
            Some(Transition::Exit) => {
                terminal.end_agent();
                let _ = self.changed.send(Change::Terminals(terminal.owner.clone()));
            }
            None => {}
        }
        if (terminal.status, &terminal.agent_session) != (before.0, &before.1) {
            let state = AgentState { status: terminal.status, session: terminal.agent_session.clone() };
            let _ = entry.control.send(Message::Agent(state));
        }
    }

    pub fn close(&self, id: &str) {
        if let Some(entry) = self.get(id) {
            let _ = entry.control.send(Message::Hangup);
        }
    }

    pub fn lead(&self, id: &str, device: &str) {
        let Some(entry) = self.get(id) else { return };
        let mut lead = entry.lead.lock().unwrap();
        if lead.device.as_deref() != Some(device) {
            lead.device = Some(device.to_owned());
            if let Some(&size) = lead.sizes.get(device) {
                entry.apply(size);
            }
        }
    }

    pub fn resize(&self, id: &str, device: &str, size: Size) {
        let Some(entry) = self.get(id) else { return };
        let mut lead = entry.lead.lock().unwrap();
        lead.sizes.insert(device.to_owned(), size);
        if lead.device.as_deref().is_none_or(|leader| leader == device) {
            entry.apply(size);
        }
    }

    fn get(&self, id: &str) -> Option<Arc<Entry>> {
        let terminals = self.terminals.lock().unwrap();
        terminals.iter().find(|(other, _)| other == id).map(|(_, entry)| Arc::clone(entry))
    }
}

impl Terminal {
    fn end_agent(&mut self) {
        self.rig = None;
        self.agent = None;
        self.status = None;
        self.agent_session = None;
    }
}

impl Entry {
    /// The holder reports the root shell in the foreground only once a Rig's Agent has exited.
    fn agent_exited(&self) -> bool {
        let mut terminal = self.terminal.lock().unwrap();
        let was = terminal.rig.is_some();
        terminal.end_agent();
        was
    }

    fn apply(&self, size: Size) {
        if size.cols > 0 && size.rows > 0 && *self.size.borrow() != size {
            let _ = self.control.send(Message::Resize(size));
            self.size.send_replace(size);
        }
    }
}

fn launch(rig: &Rig, exe: &Path, shell: &str) -> String {
    let args: String = rig
        .args
        .iter()
        .cloned()
        .chain(hooks::wiring(rig.agent, exe))
        .map(|arg| format!(" {}", quote(&arg)))
        .collect();
    format!("{}{args}; exec {} -l -i", rig.command, quote(shell))
}

pub fn quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

fn read_record(path: &Path) -> io::Result<Record> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

async fn connect(data_dir: &Path, id: &str) -> io::Result<(Frames, OwnedWriteHalf)> {
    let socket = UnixStream::connect(holder::socket_path(data_dir, id)).await?;
    let (reader, writer) = socket.into_split();
    let codec = LengthDelimitedCodec::builder().max_frame_length(protocol::MAX_FRAME).new_codec();
    let mut frames = FramedRead::new(reader, codec);
    let hello = match frames.next().await {
        Some(frame) => protocol::decode(&frame?)?,
        None => None,
    };
    match hello {
        Some(Message::Hello(hello)) if hello.protocol <= protocol::VERSION => Ok((frames, writer)),
        Some(Message::Hello(hello)) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "holder {id} speaks protocol {}, newer than {}; leaving it running",
                hello.protocol,
                protocol::VERSION
            ),
        )),
        _ => Err(io::Error::other(format!("holder {id} did not say hello"))),
    }
}
