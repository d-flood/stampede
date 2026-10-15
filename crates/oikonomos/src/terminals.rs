use std::{
    collections::HashMap,
    env, io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};

use futures_util::StreamExt;
use tokio::{
    io::AsyncWriteExt,
    net::{
        UnixStream,
        unix::{OwnedReadHalf, OwnedWriteHalf},
    },
    process::Command,
    sync::{mpsc, watch},
};
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};

use crate::{
    holder,
    protocol::{self, Message, Size},
};

type Frames = FramedRead<OwnedReadHalf, LengthDelimitedCodec>;

pub struct Terminals {
    data_dir: PathBuf,
    terminals: Mutex<Vec<(String, Arc<Entry>)>>,
}

struct Entry {
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
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            terminals: Mutex::default(),
        }
    }

    pub fn ids(&self) -> Vec<String> {
        self.terminals.lock().unwrap().iter().map(|(id, _)| id.clone()).collect()
    }

    pub async fn create(self: &Arc<Self>, device: &str) -> io::Result<String> {
        let id = format!("{:016x}", uuid::Uuid::new_v4().as_u64_pair().1);
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        let status = Command::new(env::current_exe()?)
            .arg("hold")
            .arg("--data-dir")
            .arg(&self.data_dir)
            .arg("--id")
            .arg(&id)
            .arg("--cwd")
            .arg(crate::home())
            .arg("--")
            .arg(shell)
            .arg("-l")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !status.success() {
            return Err(io::Error::other(format!("holder {status}")));
        }
        self.open(&id).await?;
        self.lead(&id, device);
        Ok(id)
    }

    async fn open(self: &Arc<Self>, id: &str) -> io::Result<()> {
        let record = std::fs::read(holder::record_path(&self.data_dir, id))?;
        let record: holder::Record = serde_json::from_slice(&record)?;
        let (mut frames, mut writer) = connect(&self.data_dir, id).await?;
        let (control, mut pending) = mpsc::unbounded_channel();
        let entry = Entry {
            control,
            size: watch::Sender::new(record.size),
            lead: Mutex::default(),
        };
        self.terminals.lock().unwrap().push((id.to_owned(), Arc::new(entry)));
        let terminals = Arc::clone(self);
        let id = id.to_owned();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(message) = pending.recv() => {
                        if writer.write_all(&protocol::encode(&message)).await.is_err() {
                            break;
                        }
                    }
                    frame = frames.next() => {
                        if !matches!(frame, Some(Ok(_))) {
                            break;
                        }
                    }
                }
            }
            terminals.terminals.lock().unwrap().retain(|(other, _)| *other != id);
        });
        Ok(())
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

impl Entry {
    fn apply(&self, size: Size) {
        if size.cols > 0 && size.rows > 0 && *self.size.borrow() != size {
            let _ = self.control.send(Message::Resize(size));
            self.size.send_replace(size);
        }
    }
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
        Some(Message::Hello(hello)) if hello.protocol == protocol::VERSION => Ok((frames, writer)),
        _ => Err(io::Error::other(format!("holder {id} did not say hello"))),
    }
}
