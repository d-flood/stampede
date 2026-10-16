#![allow(dead_code)]

use std::{
    env,
    ffi::OsString,
    fs,
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
    env: Vec<(&'static str, OsString)>,
    child: Child,
}

impl Server {
    pub async fn start(prefix: &str) -> Self {
        Self::with_config(prefix, "").await
    }

    pub async fn with_config(prefix: &str, config: &str) -> Self {
        Self::with_env(prefix, config, Vec::new()).await
    }

    pub async fn with_env(prefix: &str, config: &str, env: Vec<(&'static str, OsString)>) -> Self {
        let parent = env::var_os("OIKONOMOS_E2E_ROOT").map_or_else(env::temp_dir, PathBuf::from);
        let root = tempfile::Builder::new().prefix(prefix).tempdir_in(parent).unwrap();
        fs::create_dir(root.path().join("home")).unwrap();
        fs::write(root.path().join("config.toml"), config).unwrap();
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let child = spawn(root.path(), port, &env).await;
        Self { port, root, env, child }
    }

    pub async fn restart(&mut self) {
        self.child.kill().await.unwrap();
        self.child = spawn(self.root.path(), self.port, &self.env).await;
    }
}

async fn spawn(root: &Path, port: u16, env: &[(&str, OsString)]) -> Child {
    let child = Command::new(env!("CARGO_BIN_EXE_oikonomos"))
        .args(["serve", "--port", &port.to_string(), "--data-dir"])
        .arg(root.join("data"))
        .arg("--config")
        .arg(root.join("config.toml"))
        .env("SHELL", "/bin/bash")
        .env("HOME", root.join("home"))
        .envs(env.iter().map(|(name, value)| (name, value)))
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
    pub label: String,
    channel: u32,
    pub terminal: Terminal<'static, 'static>,
}

impl View {
    pub async fn open(port: u16, device_id: &str, terminal_id: Option<&str>) -> Self {
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
        control(&mut socket, json!({ "type": "hello", "deviceId": device_id })).await;
        let hello = next_control(&mut socket, "hello").await;
        let listed = match terminal_id {
            Some(id) => {
                let terminals = hello["terminals"].as_array().unwrap();
                let listed = terminals.iter().find(|terminal| terminal["id"] == id);
                listed.unwrap_or_else(|| panic!("{id} is not listed")).clone()
            }
            None => {
                control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
                let opened = next_control(&mut socket, "project.opened").await;
                let create = json!({ "type": "terminal.create", "projectId": opened["projectId"] });
                control(&mut socket, create).await;
                let created = next_control(&mut socket, "terminal.created").await;
                created["terminal"].clone()
            }
        };
        let attach = json!({ "type": "terminal.attach", "terminalId": listed["id"] });
        control(&mut socket, attach).await;
        let attached = next_control(&mut socket, "terminal.attached").await;
        let mut terminal = Terminal::new(
            attached["cols"].as_u64().unwrap() as u16,
            attached["rows"].as_u64().unwrap() as u16,
        )
        .unwrap();
        terminal.set_scrollback_max_lines(Some(10_000)).unwrap();
        Self {
            socket,
            terminal_id: attached["terminalId"].as_str().unwrap().to_owned(),
            label: listed["label"].as_str().unwrap().to_owned(),
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
        self.wait(|view| until(&view.text())).await;
    }

    pub async fn wait(&mut self, until: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            assert!(Instant::now() < deadline, "timed out with label {:?} and screen:\n{}", self.label, self.text());
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
                    } else if message["type"] == "terminal.title" && message["terminalId"] == *self.terminal_id {
                        self.label = message["label"].as_str().unwrap().to_owned();
                    }
                }
                Ok(Some(Ok(_))) => {}
                Ok(other) => panic!("the socket ended: {other:?}"),
                Err(_) if until(self) => return,
                Err(_) => {}
            }
        }
    }
}

pub async fn control(socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>, message: Value) {
    socket.send(Message::Text(message.to_string().into())).await.unwrap();
}

pub async fn next_control(socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>, kind: &str) -> Value {
    loop {
        if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
            let message: Value = serde_json::from_str(&text).unwrap();
            if message["type"] == kind {
                return message;
            }
        }
    }
}
