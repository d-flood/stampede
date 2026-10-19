use std::{
    cell::Cell,
    collections::HashMap,
    error::Error,
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use libghostty_vt::{
    Terminal,
    fmt::{Format, Formatter, FormatterOptions},
    screen::Screen,
    snapshot::Decoder,
    terminal::Mode,
};
use nix::{
    sys::signal::{Signal, killpg},
    unistd::{ForkResult, Pid, fork, setsid},
};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};

use crate::protocol::{self, Hello, Message, Size, Title};

const MAX_CONTINUATION: usize = 1 << 20;
const FOREGROUND_POLL: Duration = Duration::from_millis(100);
const SAVE_INTERVAL: Duration = Duration::from_secs(2);

#[derive(clap::Args)]
pub struct HoldArgs {
    #[arg(long, value_name = "DIR")]
    pub data_dir: PathBuf,
    #[arg(long)]
    pub id: String,
    #[arg(long, value_name = "DIR")]
    pub cwd: Option<PathBuf>,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long, value_name = "LINES")]
    pub scrollback: usize,
    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Owner {
    Project(String),
    #[default]
    Drawer,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    #[serde(default)]
    pub owner: Owner,
    #[serde(default)]
    pub created: u128,
    pub cwd: PathBuf,
    pub command: Vec<String>,
    pub size: Size,
    pub protocol: u16,
}

pub fn socket_path(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join("terminals").join(format!("{id}.sock"))
}

pub fn record_path(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join("terminals").join(format!("{id}.json"))
}

pub fn scrollback_path(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join("terminals").join(format!("{id}.scrollback"))
}

pub fn test_owner_alive() -> bool {
    let Ok(owner) = std::env::var("OIKONOMOS_TEST_OWNER") else { return true };
    let Some((pid, started)) = owner.split_once(':') else { return false };
    if !pid.bytes().all(|byte| byte.is_ascii_digit()) { return false }
    fs::read_to_string(format!("/proc/{pid}/stat")).ok().is_some_and(|stat| {
        stat.rsplit_once(") ").is_some_and(|(_, fields)| {
            !fields.starts_with('Z') && fields.split_whitespace().nth(19) == Some(started)
        })
    })
}

struct Files {
    socket: PathBuf,
    record: PathBuf,
    scrollback: PathBuf,
}

impl Files {
    fn write_record(&self, record: &Record) -> io::Result<()> {
        let tmp = self.record.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec(record)?)?;
        fs::rename(tmp, &self.record)
    }

    fn write_scrollback(&self, snapshot: &[u8]) -> io::Result<()> {
        let tmp = self.scrollback.with_extension("scrollback.tmp");
        fs::write(&tmp, snapshot)?;
        fs::rename(tmp, &self.scrollback)
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket);
        let _ = fs::remove_file(&self.record);
        let _ = fs::remove_file(&self.scrollback);
    }
}

pub fn hold(args: HoldArgs) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(args.data_dir.join("terminals"))?;
    let address = crate::unix::path(&socket_path(&args.data_dir, &args.id))?;
    let listener = UnixListener::bind(&address)?;
    let files = Files {
        socket: socket_path(&args.data_dir, &args.id),
        record: record_path(&args.data_dir, &args.id),
        scrollback: scrollback_path(&args.data_dir, &args.id),
    };
    let mut record = Record {
        cwd: match args.cwd {
            Some(cwd) => cwd,
            None => std::env::current_dir()?,
        },
        id: args.id,
        owner: args.project.map_or(Owner::Drawer, Owner::Project),
        created: SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
        command: args.command,
        size: Size { cols: 80, rows: 24 },
        protocol: protocol::VERSION,
    };
    files.write_record(&record)?;
    // SAFETY: no other threads exist yet.
    if let ForkResult::Parent { .. } = unsafe { fork() }? {
        // The detached child owns `files` now, so skip their Drop.
        std::process::exit(0);
    }
    setsid()?;
    run(listener, &files, &mut record, args.scrollback)
}

enum Event {
    Output(Vec<u8>),
    Exited,
    Connected(UnixStream),
    Received(u64, Message),
    Closed(u64),
    Tick,
}

struct Conn {
    frames: Sender<Vec<u8>>,
    attached: bool,
}

fn run(
    listener: UnixListener,
    files: &Files,
    record: &mut Record,
    scrollback: usize,
) -> Result<(), Box<dyn Error>> {
    let pty = native_pty_system().openpty(pty_size(record.size))?;
    let mut command = CommandBuilder::new(&record.command[0]);
    command.args(&record.command[1..]);
    command.cwd(&record.cwd);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    let mut child = pty.slave.spawn_command(command)?;
    let pid = child.process_id();
    drop(pty.slave);

    let (events, received) = mpsc::channel();
    let mut reader = pty.master.try_clone_reader()?;
    let tx = events.clone();
    thread::spawn(move || {
        let mut buf = vec![0; 1 << 16];
        while let Ok(n @ 1..) = reader.read(&mut buf) {
            if tx.send(Event::Output(buf[..n].to_vec())).is_err() {
                break;
            }
        }
    });
    let tx = events.clone();
    thread::spawn(move || {
        let _ = child.wait();
        let _ = tx.send(Event::Exited);
    });
    let tx = events.clone();
    thread::spawn(move || {
        while tx.send(Event::Tick).is_ok() {
            thread::sleep(FOREGROUND_POLL);
        }
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
    let mut writer = pty.master.take_writer()?;
    thread::spawn(move || {
        for bytes in pending {
            if writer.write_all(&bytes).is_err() {
                break;
            }
        }
    });

    let titled = Cell::new(false);
    let mut terminal = Terminal::new(record.size.cols, record.size.rows)?;
    terminal
        // The default byte cap (10 KB) would prune long before the line cap.
        .set_scrollback_max_bytes(None)?
        .set_scrollback_max_lines(Some(scrollback))?
        .set_continuation_max_bytes(MAX_CONTINUATION)?
        .on_pty_write({
            let input = input.clone();
            move |_, data| {
                let _ = input.send(data.to_vec());
            }
        })?
        .on_title_changed(|_| titled.set(true))?;

    let mut titles = Titles {
        root: pid.map(|pid| pid as i32),
        foreground: None,
        setter: None,
        current: Title {
            title: None,
            cwd: record.cwd.clone(),
        },
    };
    let mut reported = titles.current.clone();
    let mut conns: HashMap<u64, Conn> = HashMap::new();
    let mut next_id = 0;
    let mut changed = false;
    let mut saved = Instant::now();
    for event in received {
        match event {
            Event::Output(bytes) => {
                terminal.vt_write(&bytes);
                changed = true;
                if titled.replace(false) {
                    titles.set(terminal.title().unwrap_or_default(), pty.master.process_group_leader());
                }
                let frame = protocol::encode(&Message::Output(bytes));
                for conn in conns.values().filter(|conn| conn.attached) {
                    let _ = conn.frames.send(frame.clone());
                }
            }
            Event::Exited => break,
            Event::Connected(stream) => {
                next_id += 1;
                if let Ok(conn) = connect(next_id, stream, events.clone()) {
                    let _ = conn.frames.send(protocol::encode(&Message::TitleChanged(reported.clone())));
                    conns.insert(next_id, conn);
                }
            }
            Event::Received(id, Message::Attach) => match snapshot(&mut terminal) {
                Ok(bytes) => {
                    if let Some(conn) = conns.get_mut(&id) {
                        let _ = conn.frames.send(protocol::encode(&Message::Snapshot(bytes)));
                        conn.attached = true;
                    }
                }
                Err(_) => {
                    conns.remove(&id);
                }
            },
            Event::Received(_, Message::Input(bytes)) => {
                let _ = input.send(bytes);
            }
            Event::Received(_, Message::Resize(size)) => {
                if resize(&mut terminal, &*pty.master, size).is_ok() {
                    record.size = size;
                    changed = true;
                    let _ = files.write_record(record);
                }
            }
            Event::Received(_, Message::Scrollback(lines)) => {
                terminal.set_scrollback_max_lines(Some(lines))?;
                changed = true;
            }
            Event::Received(_, Message::Hangup) => {
                // portable-pty starts the child with setsid, so its pid names its process group.
                if let Some(pid) = pid {
                    let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGHUP);
                }
                break;
            }
            Event::Received(..) => {}
            Event::Closed(id) => {
                conns.remove(&id);
            }
            Event::Tick => {
                if !files.record.exists() || !test_owner_alive() {
                    if let Some(pid) = pid {
                        let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGHUP);
                    }
                    break;
                }
                if changed && saved.elapsed() >= SAVE_INTERVAL {
                    changed = false;
                    saved = Instant::now();
                    if let Ok(bytes) = snapshot(&mut terminal) {
                        let _ = files.write_scrollback(&bytes);
                    }
                }
                let Some(leader) = pty.master.process_group_leader() else { continue };
                titles.foreground(leader);
            }
        }
        if titles.current != reported {
            reported = titles.current.clone();
            let frame = protocol::encode(&Message::TitleChanged(reported.clone()));
            for conn in conns.values() {
                let _ = conn.frames.send(frame.clone());
            }
        }
    }
    Ok(())
}

/// A title holds while the process group that set it keeps the foreground. One the root shell set
/// itself also holds through the job it starts next, as a zsh preexec title does, until the root
/// shell takes the foreground back.
struct Titles {
    root: Option<i32>,
    foreground: Option<i32>,
    setter: Option<i32>,
    current: Title,
}

impl Titles {
    fn foreground(&mut self, leader: i32) {
        if self.foreground == Some(leader) {
            return;
        }
        self.foreground = Some(leader);
        if self.setter != self.root || self.root == Some(leader) {
            self.current.title = None;
        }
        if let Some(cwd) = self.root.and_then(|root| fs::read_link(format!("/proc/{root}/cwd")).ok()) {
            self.current.cwd = cwd;
        }
    }

    fn set(&mut self, title: &str, leader: Option<i32>) {
        if let Some(leader) = leader {
            self.foreground(leader);
        }
        self.setter = self.foreground;
        self.current.title = (!title.is_empty()).then(|| title.to_owned());
    }
}

fn pty_size(size: Size) -> PtySize {
    PtySize {
        cols: size.cols,
        rows: size.rows,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn resize(terminal: &mut Terminal, pty: &dyn MasterPty, size: Size) -> Result<(), Box<dyn Error>> {
    terminal.resize(size.cols, size.rows, 0, 0)?;
    pty.resize(pty_size(size))?;
    Ok(())
}

fn connect(id: u64, stream: UnixStream, events: Sender<Event>) -> io::Result<Conn> {
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
            if events.send(Event::Received(id, message)).is_err() {
                return;
            }
        }
        let _ = events.send(Event::Closed(id));
    });
    let hello = Hello {
        protocol: protocol::VERSION,
        holder_version: env!("CARGO_PKG_VERSION").into(),
    };
    let _ = frames.send(protocol::encode(&Message::Hello(hello)));
    Ok(Conn {
        frames,
        attached: false,
    })
}

fn snapshot(terminal: &mut Terminal) -> Result<Vec<u8>, libghostty_vt::Error> {
    let mut out = Vec::new();
    if terminal.active_screen()? == Screen::Alternate {
        // The formatter only reaches the active screen, so the primary screen
        // comes from a copy switched back to it.
        let mut encoded = Vec::new();
        terminal.encode_snapshot(&mut encoded)?;
        let mut primary = Decoder::new_buf(&encoded)?.decode()?;
        primary.vt_write(if primary.mode(Mode::ALT_SCREEN_SAVE)? {
            b"\x18\x1b[?1049l"
        } else {
            b"\x18\x1b[?47l"
        });
        out.extend_from_slice(&format(&primary, false)?);
    }
    out.extend_from_slice(&format(terminal, true)?);
    if let Ok(Some(continuation)) = terminal.continuation_alloc(None) {
        out.extend_from_slice(&continuation);
    }
    Ok(out)
}

fn format(terminal: &Terminal, terminal_state: bool) -> Result<Vec<u8>, libghostty_vt::Error> {
    let options = FormatterOptions::new()
        .with_format(Format::Vt)
        .with_unwrap(true)
        .with_modes(terminal_state)
        .with_scrolling_region(terminal_state)
        .with_tabstops(terminal_state)
        .with_pwd(terminal_state)
        .with_keyboard(terminal_state)
        .with_cursor(true)
        .with_style(true)
        .with_hyperlink(true)
        .with_protection(true)
        .with_kitty_keyboard(true)
        .with_charsets(true);
    Ok(Formatter::new(terminal, options)?.format_alloc(None)?.to_vec())
}
