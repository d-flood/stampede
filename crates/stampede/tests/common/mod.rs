#![allow(dead_code)]

use std::{
    env,
    ffi::OsString,
    fs,
    io::Write,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use axum::{Router, body::Bytes, extract, http::StatusCode, routing::post};
use base64ct::{Base64UrlUnpadded, Encoding};
use futures_util::{SinkExt, StreamExt};
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
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
    sync::mpsc,
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use web_push_native::{Auth, decrypt, p256::SecretKey};

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
        let parent = env::var_os("STAMPEDE_E2E_ROOT").map_or_else(env::temp_dir, PathBuf::from);
        let root = tempfile::Builder::new().prefix(prefix).tempdir_in(parent).unwrap();
        fs::create_dir(root.path().join("home")).unwrap();
        fs::write(root.path().join("config.toml"), config).unwrap();
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let child = spawn(root.path(), port, &env).await;
        Self { port, root, env, child }
    }

    pub async fn stop(&mut self) {
        self.child.kill().await.unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while TcpStream::connect(("127.0.0.1", self.port)).await.is_ok() {
            assert!(Instant::now() < deadline, "the stopped server's listener remained open");
            sleep(Duration::from_millis(10)).await;
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.id().unwrap()
    }

    pub async fn restart(&mut self) {
        if self.child.try_wait().unwrap().is_none() { self.stop().await; }
        self.child = spawn(self.root.path(), self.port, &self.env).await;
    }
}

async fn spawn(root: &Path, port: u16, env: &[(&str, OsString)]) -> Child {
    let pid = std::process::id();
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let started = stat.rsplit_once(") ").unwrap().1.split_whitespace().nth(19).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_stampede"))
        .args(["serve", "--port", &port.to_string(), "--data-dir"])
        .arg(root.join("data"))
        .arg("--config")
        .arg(root.join("config.toml"))
        .env("SHELL", "/bin/bash")
        .env("HOME", root.join("home"))
        .env("STAMPEDE_TEST_ROOT", root)
        .env("STAMPEDE_TEST_OWNER", format!("{pid}:{started}"))
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
        let _ = self.child.start_kill();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while matches!(self.child.try_wait(), Ok(None)) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = std::process::Command::new("python3")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/check_cleanup.py"))
            .arg(self.root.path())
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

pub async fn next(socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) -> Value {
    loop {
        if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

/// Sends `text` to a Chat whose status is `status`, and gathers its statuses and items until it is done again.
pub async fn chat_turn(
    socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    chat_id: &str,
    status: &Value,
    text: &str,
) -> (Vec<Value>, Vec<Value>) {
    control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": text })).await;
    let mut statuses = vec![status.clone()];
    let mut items: Vec<Value> = Vec::new();
    timeout(Duration::from_secs(20), async {
        while statuses.len() < 2 || statuses.last().unwrap() != "done" {
            let message = next(socket).await;
            if message["chatId"] != chat_id {
                continue;
            }
            match message["type"].as_str().unwrap() {
                "chat.status" => statuses.push(message["status"].clone()),
                "chat.item" => {
                    let index = message["index"].as_u64().unwrap() as usize;
                    assert!(index <= items.len(), "{message}");
                    match items.get_mut(index) {
                        Some(item) => *item = message["item"].clone(),
                        None => items.push(message["item"].clone()),
                    }
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the turn never ended: {statuses:?} {items:?}"));
    (statuses, items)
}

/// Closes a Chat and waits until it is unlisted, its record is gone, and nothing it ran is left.
pub async fn close_chat(socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>, root: &Path, chat_id: &str) {
    control(socket, json!({ "type": "chat.close", "chatId": chat_id })).await;
    timeout(Duration::from_secs(10), async {
        loop {
            let changed = next_control(socket, "chats.changed").await;
            if !changed["chats"].as_array().unwrap().iter().any(|chat| chat["id"] == chat_id) {
                break;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the chat never closed; running: {:?}", leftovers(root)));
    assert!(!root.join(format!("data/chats/{chat_id}.json")).exists());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let running = leftovers(root);
        if running.is_empty() {
            break;
        }
        assert!(Instant::now() < deadline, "left running: {running:?}");
        sleep(Duration::from_millis(50)).await;
    }
}

/// Every process but the server that names the test's root in its command line or runs inside it.
pub fn leftovers(root: &Path) -> Vec<String> {
    processes(root).into_iter().map(|(_, cmdline)| cmdline).collect()
}

pub fn without_storage(params: &Value) -> Value {
    let mut params = params.clone();
    if let Some(config) = params.get_mut("config").and_then(Value::as_object_mut) {
        config.remove("shell_environment_policy.set");
        config.remove("sandbox_workspace_write.writable_roots");
    }
    if let Some(policy) = params.get_mut("sandboxPolicy").and_then(Value::as_object_mut) { policy.remove("writableRoots"); }
    params
}

pub fn reboot(root: &Path) {
    for (pid, _) in processes(root) {
        let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
    }
}

fn processes(root: &Path) -> Vec<(i32, String)> {
    let canonical = fs::canonicalize(root).unwrap();
    let root = root.to_str().unwrap();
    fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|entry| {
            let pid = entry.file_name().to_str()?.parse::<i32>().ok()?;
            let cmdline = String::from_utf8_lossy(&fs::read(entry.path().join("cmdline")).ok()?).replace('\0', " ");
            let inside = fs::read_link(entry.path().join("cwd")).is_ok_and(|cwd| cwd.starts_with(&canonical));
            ((cmdline.contains(root) || inside) && !cmdline.contains(" serve ")).then_some((pid, cmdline))
        })
        .collect()
}

pub struct Cli {
    pub data_dir: PathBuf,
}

impl Cli {
    pub fn run(&self, cwd: &Path, args: &[&str], stdin: &str) -> Result<String, String> {
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_stampede"))
            .args(args)
            .arg("--data-dir")
            .arg(&self.data_dir)
            .current_dir(cwd)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        if output.status.success() {
            assert_eq!(stderr, "", "{args:?}");
            Ok(stdout)
        } else {
            assert_eq!(stdout, "", "{args:?}");
            Err(stderr)
        }
    }

    pub fn ok(&self, cwd: &Path, args: &[&str], stdin: &str) -> String {
        self.run(cwd, args, stdin).unwrap_or_else(|error| panic!("{args:?} failed: {error}"))
    }

    pub fn refused(&self, cwd: &Path, args: &[&str], stdin: &str) -> String {
        match self.run(cwd, args, stdin) {
            Ok(output) => panic!("{args:?} succeeded: {output}"),
            Err(error) => error,
        }
    }
}

pub fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(["-c", "user.name=stampede", "-c", "user.email=test@stampede.invalid", "-C"])
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

pub fn init(root: &Path, name: &str) -> PathBuf {
    let repo = root.join(name);
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    fs::canonicalize(repo).unwrap()
}

pub async fn subscribe(server: &Server, browsers: &[&str]) -> mpsc::UnboundedReceiver<(String, Value)> {
    let secret = SecretKey::from_slice(&[7; 32]).unwrap();
    let auth = Auth::clone_from_slice(&[9; 16]);
    let keys = json!({
        "p256dh": Base64UrlUnpadded::encode_string(&secret.public_key().to_sec1_bytes()),
        "auth": Base64UrlUnpadded::encode_string(&auth),
    });
    let (sender, pushes) = mpsc::unbounded_channel();
    let receive = move |extract::Path(browser): extract::Path<String>, body: Bytes| {
        let (secret, auth, sender) = (secret.clone(), auth, sender.clone());
        async move {
            let payload = decrypt(body.to_vec(), &secret, &auth).unwrap();
            sender.send((browser, serde_json::from_slice(&payload).unwrap())).unwrap();
            StatusCode::CREATED
        }
    };
    let endpoint = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = endpoint.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(endpoint, Router::new().route("/{browser}", post(receive))).await });
    let client = reqwest::Client::new();
    for browser in browsers {
        let subscription = json!({
            "browserId": browser,
            "subscription": { "endpoint": format!("http://127.0.0.1:{port}/{browser}"), "keys": keys },
        });
        let response = client
            .post(format!("http://127.0.0.1:{}/api/push/subscribe", server.port))
            .header("content-type", "application/json")
            .body(subscription.to_string())
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success(), "{}", response.status());
    }
    pushes
}

pub async fn pushed(pushes: &mut mpsc::UnboundedReceiver<(String, Value)>) -> (String, Value) {
    timeout(Duration::from_secs(10), pushes.recv()).await.expect("no push arrived").unwrap()
}
