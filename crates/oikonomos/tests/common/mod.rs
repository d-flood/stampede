#![allow(dead_code)]

use std::{
    env, fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use libghostty_vt::{
    Terminal,
    fmt::{Format, Formatter, FormatterOptions},
    terminal::Mode,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{
    net::TcpStream,
    process::{Child, Command},
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

pub fn state(terminal: &Terminal) -> String {
    let modes = [
        Mode::DECCKM,
        Mode::BRACKETED_PASTE,
        Mode::NORMAL_MOUSE,
        Mode::SGR_MOUSE,
        Mode::FOCUS_EVENT,
        Mode::ALT_SCREEN_SAVE,
    ]
    .map(|mode| terminal.mode(mode).unwrap());
    format!(
        "{:?} cursor=({}, {}) scrollback={} modes={modes:?}\n{}",
        terminal.active_screen().unwrap(),
        terminal.cursor_x().unwrap(),
        terminal.cursor_y().unwrap(),
        terminal.scrollback_rows().unwrap(),
        format(terminal, Format::Vt),
    )
}

fn format(terminal: &Terminal, format: Format) -> String {
    let options = FormatterOptions::new()
        .with_format(format)
        .with_unwrap(true)
        .with_modes(true)
        .with_cursor(true)
        .with_style(true);
    let bytes = Formatter::new(terminal, options).unwrap().format_alloc(None).unwrap();
    String::from_utf8_lossy(&bytes).into_owned()
}

pub struct Server {
    pub port: u16,
    pub root: TempDir,
    child: Child,
}

impl Server {
    pub async fn start(prefix: &str) -> Self {
        let parent = env::var_os("OIKONOMOS_E2E_ROOT").map_or_else(env::temp_dir, PathBuf::from);
        let root = tempfile::Builder::new().prefix(prefix).tempdir_in(parent).unwrap();
        fs::create_dir(root.path().join("home")).unwrap();
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let child = spawn(root.path(), port).await;
        Self { port, root, child }
    }

    pub async fn restart(&mut self) {
        self.child.kill().await.unwrap();
        self.child = spawn(self.root.path(), self.port).await;
    }
}

async fn spawn(root: &Path, port: u16) -> Child {
    let child = Command::new(env!("CARGO_BIN_EXE_oikonomos"))
        .args(["serve", "--port", &port.to_string(), "--data-dir"])
        .arg(root.join("data"))
        .arg("--config")
        .arg(root.join("config.toml"))
        .env("SHELL", "/bin/bash")
        .env("HOME", root.join("home"))
        .stdout(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while TcpStream::connect(("127.0.0.1", port)).await.is_err() {
        assert!(Instant::now() < deadline, "the server never listened");
        sleep(Duration::from_millis(50)).await;
    }
    child
}

impl Drop for Server {
    fn drop(&mut self) {
        let data_dir = self.root.path().join("data");
        let _ = std::process::Command::new("pkill")
            .arg("-f")
            .arg(format!("oikonomos hold --data-dir {}", data_dir.display()))
            .status();
    }
}

pub struct View {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    pub terminal_id: String,
    channel: u32,
    pub terminal: Terminal<'static, 'static>,
}

impl View {
    pub async fn open(port: u16, device_id: &str, terminal_id: Option<&str>) -> Self {
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
        control(&mut socket, json!({ "type": "hello", "deviceId": device_id })).await;
        let hello = next_control(&mut socket).await;
        assert_eq!(hello["type"], "hello");
        let request = match terminal_id {
            Some(id) => {
                assert!(hello["terminals"].as_array().unwrap().contains(&json!(id)), "{id} is not listed");
                json!({ "type": "terminal.attach", "terminalId": id })
            }
            None => json!({ "type": "terminal.create" }),
        };
        control(&mut socket, request).await;
        let attached = next_control(&mut socket).await;
        assert_eq!(attached["type"], "terminal.attached");
        let mut terminal = Terminal::new(
            attached["cols"].as_u64().unwrap() as u16,
            attached["rows"].as_u64().unwrap() as u16,
        )
        .unwrap();
        terminal.set_scrollback_max_lines(Some(10_000)).unwrap();
        Self {
            socket,
            terminal_id: attached["terminalId"].as_str().unwrap().to_owned(),
            channel: attached["channel"].as_u64().unwrap() as u32,
            terminal,
        }
    }

    pub async fn send(&mut self, text: &str) {
        let mut frame = vec![2];
        frame.extend_from_slice(&self.channel.to_be_bytes());
        frame.extend_from_slice(text.as_bytes());
        self.socket.send(Message::Binary(frame.into())).await.unwrap();
    }

    pub fn text(&self) -> String {
        format(&self.terminal, Format::Plain)
    }

    pub async fn settle(&mut self, until: impl Fn(&str) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            assert!(Instant::now() < deadline, "timed out with screen:\n{}", self.text());
            match timeout(Duration::from_millis(500), self.socket.next()).await {
                Ok(Some(Ok(Message::Binary(frame)))) => {
                    if frame[0] == 1 && frame[1..5] == self.channel.to_be_bytes() {
                        self.terminal.vt_write(&frame[5..]);
                    }
                }
                Ok(Some(Ok(Message::Text(text)))) => {
                    let message: Value = serde_json::from_str(&text).unwrap();
                    if message["type"] == "terminal.resized" {
                        let cols = message["cols"].as_u64().unwrap() as u16;
                        let rows = message["rows"].as_u64().unwrap() as u16;
                        self.terminal.resize(cols, rows, 0, 0).unwrap();
                    }
                }
                Ok(Some(Ok(_))) => {}
                Ok(other) => panic!("the socket ended: {other:?}"),
                Err(_) if until(&self.text()) => return,
                Err(_) => {}
            }
        }
    }
}

async fn control(socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>, message: Value) {
    socket.send(Message::Text(message.to_string().into())).await.unwrap();
}

async fn next_control(socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) -> Value {
    loop {
        if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
            return serde_json::from_str(&text).unwrap();
        }
    }
}
