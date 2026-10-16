use std::{
    collections::HashMap,
    error::Error,
    ffi::OsStr,
    fs,
    io::{self, BufRead, BufReader, Write},
    net::Shutdown,
    os::unix::{
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Sender},
    thread,
};

use futures_util::StreamExt;
use nix::{
    sys::signal::{Signal, killpg},
    unistd::{ForkResult, Pid, fork, setsid},
};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio_util::codec::{FramedRead, LengthDelimitedCodec};

use super::protocol::{self, Hello, Message};

pub type Frames = FramedRead<OwnedReadHalf, LengthDelimitedCodec>;

#[derive(clap::Args)]
pub struct HoldChatArgs {
    #[arg(long, value_name = "DIR")]
    pub data_dir: PathBuf,
    #[arg(long)]
    pub id: String,
    #[arg(long, value_name = "DIR")]
    pub cwd: PathBuf,
    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

pub fn socket_path(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join("chats").join(format!("{id}.sock"))
}

struct Socket(PathBuf);

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn hold_chat(args: HoldChatArgs) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(args.data_dir.join("chats"))?;
    let path = socket_path(&args.data_dir, &args.id);
    let listener = UnixListener::bind(&path)?;
    let _socket = Socket(path);
    // SAFETY: no other threads exist yet.
    if let ForkResult::Parent { .. } = unsafe { fork() }? {
        // The detached child owns the socket now, so skip its Drop.
        std::process::exit(0);
    }
    setsid()?;
    run(listener, &args.cwd, &args.command)
}

enum Event {
    Line(Vec<u8>),
    Exited,
    Connected(UnixStream),
    Received(Message),
    Closed(u64),
}

fn run(listener: UnixListener, cwd: &Path, command: &[String]) -> Result<(), Box<dyn Error>> {
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    let group = Pid::from_raw(child.id() as i32);
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    let (events, received) = mpsc::channel();
    let tx = events.clone();
    thread::spawn(move || {
        for line in BufReader::new(stdout).split(b'\n') {
            let Ok(line) = line else { break };
            if tx.send(Event::Line(line)).is_err() {
                return;
            }
        }
        let _ = child.wait();
        let _ = tx.send(Event::Exited);
    });
    let tx = events.clone();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if tx.send(Event::Connected(stream)).is_err() {
                break;
            }
        }
    });
    let (input, pending) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        for mut line in pending {
            line.push(b'\n');
            if stdin.write_all(&line).is_err() {
                break;
            }
        }
    });

    let mut conns: HashMap<u64, Sender<Vec<u8>>> = HashMap::new();
    let mut next_id = 0;
    let mut buffer = Vec::new();
    for event in received {
        match event {
            Event::Line(line) => {
                let frame = protocol::encode(&Message::Line(line));
                for conn in conns.values() {
                    let _ = conn.send(frame.clone());
                }
                buffer.push(frame);
            }
            Event::Exited => break,
            Event::Connected(stream) => {
                next_id += 1;
                if let Ok(conn) = accept(next_id, stream, events.clone(), &buffer) {
                    conns.insert(next_id, conn);
                }
            }
            Event::Received(Message::Input(line)) => {
                buffer.push(protocol::encode(&Message::Input(line.clone())));
                let _ = input.send(line);
            }
            Event::Received(Message::Forget) => buffer.clear(),
            Event::Received(Message::Hangup) => {
                let _ = killpg(group, Signal::SIGTERM);
            }
            Event::Received(_) => {}
            Event::Closed(id) => {
                conns.remove(&id);
            }
        }
    }
    Ok(())
}

fn accept(id: u64, stream: UnixStream, events: Sender<Event>, buffer: &[Vec<u8>]) -> io::Result<Sender<Vec<u8>>> {
    let mut reader = stream.try_clone()?;
    let mut writer = stream;
    let (frames, pending) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        for frame in pending {
            if writer.write_all(&frame).is_err() {
                break;
            }
        }
        let _ = writer.shutdown(Shutdown::Both);
    });
    thread::spawn(move || {
        while let Ok(Some(message)) = protocol::read(&mut reader) {
            if events.send(Event::Received(message)).is_err() {
                return;
            }
        }
        let _ = events.send(Event::Closed(id));
    });
    let hello = Hello {
        protocol: protocol::VERSION,
        holder_version: env!("CARGO_PKG_VERSION").into(),
        buffered: buffer.len(),
    };
    let _ = frames.send(protocol::encode(&Message::Hello(hello)));
    for frame in buffer {
        let _ = frames.send(frame.clone());
    }
    Ok(frames)
}

pub async fn spawn<'a>(
    data_dir: &Path,
    id: &str,
    cwd: &Path,
    env: impl IntoIterator<Item = (&'a str, &'a str)>,
    command: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> io::Result<()> {
    let status = tokio::process::Command::new("/proc/self/exe")
        .arg("hold-chat")
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--id")
        .arg(id)
        .arg("--cwd")
        .arg(cwd)
        .arg("--")
        .args(command)
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;
    if !status.success() {
        return Err(io::Error::other(format!("chat holder {status}")));
    }
    Ok(())
}

/// Connects to a Chat holder, returning how many buffered frames it replays before streaming live.
pub async fn connect(data_dir: &Path, id: &str) -> io::Result<(Frames, OwnedWriteHalf, usize)> {
    let socket = tokio::net::UnixStream::connect(socket_path(data_dir, id)).await?;
    let (reader, writer) = socket.into_split();
    let codec = LengthDelimitedCodec::builder().max_frame_length(crate::protocol::MAX_FRAME).new_codec();
    let mut frames = FramedRead::new(reader, codec);
    let hello = match frames.next().await {
        Some(frame) => protocol::decode(&frame?)?,
        None => None,
    };
    match hello {
        Some(Message::Hello(hello)) if hello.protocol <= protocol::VERSION => Ok((frames, writer, hello.buffered)),
        Some(Message::Hello(hello)) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("chat holder {id} speaks protocol {}, newer than {}", hello.protocol, protocol::VERSION),
        )),
        _ => Err(io::Error::other(format!("chat holder {id} did not say hello"))),
    }
}
