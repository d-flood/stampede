use std::{
    collections::HashMap,
    env, fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};

use futures_util::StreamExt;
use serde::Serialize;
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
use uuid::Uuid;

use crate::{
    holder::{self, Owner, Record},
    projects::Project,
    protocol::{self, Message, Size, Title},
};

type Frames = FramedRead<OwnedReadHalf, LengthDelimitedCodec>;

pub struct Terminals {
    data_dir: PathBuf,
    terminals: Mutex<Vec<(String, Slot)>>,
    changed: broadcast::Sender<Owner>,
    titles: broadcast::Sender<(String, String)>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Terminal {
    pub id: String,
    pub owner: Owner,
    pub label: String,
    pub state: State,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Live,
    Ended,
}

enum Slot {
    Live(Arc<Entry>),
    Ended(Terminal, Size),
}

struct Entry {
    terminal: Mutex<Terminal>,
    shell: String,
    title: Mutex<Title>,
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

pub enum Output {
    Live { frames: Frames, _writer: OwnedWriteHalf },
    Saved(Option<Vec<u8>>),
}

impl Output {
    pub async fn next(&mut self) -> Option<Vec<u8>> {
        match self {
            Output::Live { frames, .. } => {
                while let Some(Ok(frame)) = frames.next().await {
                    if let Ok(Some(Message::Snapshot(bytes) | Message::Output(bytes))) =
                        protocol::decode(&frame)
                    {
                        return Some(bytes);
                    }
                }
                None
            }
            Output::Saved(saved) => saved.take(),
        }
    }
}

impl Terminals {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            terminals: Mutex::default(),
            changed: broadcast::Sender::new(64),
            titles: broadcast::Sender::new(64),
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
                Ok(record) => records.push(record),
                Err(error) => eprintln!("cannot read {}: {error}", path.display()),
            }
        }
        records.sort_by_key(|record| record.created);
        for record in records {
            match self.open(record.clone()).await {
                Ok(_) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                    ) =>
                {
                    self.end(record);
                }
                Err(error) => eprintln!("cannot reattach terminal {}: {error}", record.id),
            }
        }
        Ok(())
    }

    fn end(&self, record: Record) {
        let size = record.size;
        let (terminal, ..) = describe(record);
        let terminal = Terminal { state: State::Ended, ..terminal };
        self.terminals.lock().unwrap().push((terminal.id.clone(), Slot::Ended(terminal, size)));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Owner> {
        self.changed.subscribe()
    }

    pub fn subscribe_titles(&self) -> broadcast::Receiver<(String, String)> {
        self.titles.subscribe()
    }

    pub fn list(&self) -> Vec<Terminal> {
        let terminals = self.terminals.lock().unwrap();
        terminals
            .iter()
            .map(|(_, slot)| match slot {
                Slot::Live(entry) => entry.terminal.lock().unwrap().clone(),
                Slot::Ended(terminal, _) => terminal.clone(),
            })
            .collect()
    }

    pub fn owned(&self, owner: &Owner) -> Vec<Terminal> {
        self.list().into_iter().filter(|terminal| terminal.owner == *owner).collect()
    }

    pub async fn create(
        self: &Arc<Self>,
        project: Option<&Project>,
        scrollback: usize,
        device: &str,
    ) -> io::Result<Terminal> {
        let id = format!("{:016x}", Uuid::new_v4().as_u64_pair().1);
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        let mut command = Command::new("/proc/self/exe");
        for key in ["STAMPEDE_HOOK_URL", "STAMPEDE_TERMINAL_ID", "STAMPEDE_DATA_DIR"] {
            command.env_remove(key);
        }
        command.arg("hold").arg("--data-dir").arg(&self.data_dir).arg("--id").arg(&id);
        command.arg("--scrollback").arg(scrollback.to_string());
        match project {
            Some(project) => command.arg("--cwd").arg(&project.path).arg("--project").arg(&project.id),
            None => command.arg("--cwd").arg(crate::home()),
        };
        command.arg("--").arg(&shell).arg("-l");
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
        let (mut frames, mut writer, initial) = connect(&self.data_dir, &record.id).await?;
        let (control, mut pending) = mpsc::unbounded_channel();
        let size = record.size;
        let (mut terminal, shell, mut title) = describe(record);
        if let Some(initial) = initial {
            title = initial;
            terminal.label = label(&shell, &title);
        }
        let entry = Arc::new(Entry {
            terminal: Mutex::new(terminal.clone()),
            shell,
            title: Mutex::new(title),
            control,
            size: watch::Sender::new(size),
            lead: Mutex::default(),
        });
        self.terminals.lock().unwrap().push((terminal.id.clone(), Slot::Live(Arc::clone(&entry))));
        let _ = self.changed.send(terminal.owner.clone());
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
                        match protocol::decode(&frame) {
                            Ok(Some(Message::TitleChanged(title))) => {
                                if let Some(label) = entry.retitle(title) {
                                    let _ = terminals.titles.send((id.clone(), label));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            terminals.terminals.lock().unwrap().retain(|(other, _)| *other != id);
            let _ = terminals.changed.send(owner);
        });
        Ok(terminal)
    }

    pub async fn attach(&self, id: &str) -> io::Result<Attachment> {
        if let Some((_, size)) = self.ended(id) {
            let saved = match tokio::fs::read(holder::scrollback_path(&self.data_dir, id)).await {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
                saved => saved?,
            };
            return Ok(Attachment {
                size: watch::channel(size).1,
                output: Output::Saved(Some(saved)),
            });
        }
        let entry = self.get(id).ok_or(io::ErrorKind::NotFound)?;
        let (frames, mut writer, _) = connect(&self.data_dir, id).await?;
        writer.write_all(&protocol::encode(&Message::Attach)).await?;
        Ok(Attachment {
            size: entry.size.subscribe(),
            output: Output::Live {
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

    pub fn close(&self, id: &str) {
        match self.get(id) {
            Some(entry) => {
                let _ = entry.control.send(Message::Hangup);
            }
            None => self.dismiss(id),
        }
    }

    pub fn ended(&self, id: &str) -> Option<(Terminal, Size)> {
        let terminals = self.terminals.lock().unwrap();
        terminals.iter().find_map(|(other, slot)| match slot {
            Slot::Ended(terminal, size) if other == id => Some((terminal.clone(), *size)),
            _ => None,
        })
    }

    pub fn dismiss(&self, id: &str) {
        let Some((terminal, _)) = self.ended(id) else { return };
        self.terminals.lock().unwrap().retain(|(other, _)| other != id);
        for path in [holder::record_path, holder::socket_path, holder::scrollback_path] {
            let _ = fs::remove_file(path(&self.data_dir, id));
        }
        let _ = self.changed.send(terminal.owner);
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

    pub fn scrollback(&self, lines: usize) {
        for (_, slot) in self.terminals.lock().unwrap().iter() {
            if let Slot::Live(entry) = slot {
                let _ = entry.control.send(Message::Scrollback(lines));
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
        terminals.iter().find_map(|(other, slot)| match slot {
            Slot::Live(entry) if other == id => Some(Arc::clone(entry)),
            _ => None,
        })
    }
}

impl Entry {
    fn retitle(&self, title: Title) -> Option<String> {
        let mut terminal = self.terminal.lock().unwrap();
        let mut current = self.title.lock().unwrap();
        *current = title;
        let label = label(&self.shell, &current);
        (label != terminal.label).then(|| {
            terminal.label.clone_from(&label);
            label
        })
    }

    fn apply(&self, size: Size) {
        if size.cols > 0 && size.rows > 0 && *self.size.borrow() != size {
            let _ = self.control.send(Message::Resize(size));
            self.size.send_replace(size);
        }
    }
}

fn describe(record: Record) -> (Terminal, String, Title) {
    let shell = Path::new(&record.command[0]).file_name().unwrap_or_default();
    let shell = shell.to_string_lossy().into_owned();
    let title = Title {
        title: None,
        cwd: record.cwd,
    };
    let terminal = Terminal {
        id: record.id,
        owner: record.owner,
        label: label(&shell, &title),
        state: State::Live,
    };
    (terminal, shell, title)
}

fn label(shell: &str, title: &Title) -> String {
    if let Some(title) = &title.title {
        return title.clone();
    }
    match title.cwd.strip_prefix(crate::home()) {
        Ok(rest) if rest.as_os_str().is_empty() => format!("{shell} — ~"),
        Ok(rest) => format!("{shell} — ~/{}", rest.display()),
        Err(_) => format!("{shell} — {}", title.cwd.display()),
    }
}

fn read_record(path: &Path) -> io::Result<Record> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

async fn connect(data_dir: &Path, id: &str) -> io::Result<(Frames, OwnedWriteHalf, Option<Title>)> {
    let address = crate::unix::path(&holder::socket_path(data_dir, id))?;
    let socket = UnixStream::connect(&address).await?;
    let (reader, writer) = socket.into_split();
    let codec = LengthDelimitedCodec::builder().max_frame_length(protocol::MAX_FRAME).new_codec();
    let mut frames = FramedRead::new(reader, codec);
    let hello = match frames.next().await {
        Some(frame) => protocol::decode(&frame?)?,
        None => None,
    };
    match hello {
        Some(Message::Hello(hello)) if hello.protocol <= protocol::VERSION => {
            let initial = if hello.protocol >= 6 {
                loop {
                    let frame = frames.next().await.ok_or(io::ErrorKind::UnexpectedEof)??;
                    if let Some(Message::TitleChanged(title)) = protocol::decode(&frame)? {
                        break Some(title);
                    }
                }
            } else { None };
            Ok((frames, writer, initial))
        },
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
