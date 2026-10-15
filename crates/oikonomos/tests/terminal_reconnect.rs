use std::{
    env, fs,
    net::TcpListener,
    path::PathBuf,
    process::Stdio,
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use libghostty_vt::{
    Terminal,
    fmt::{Format, Formatter, FormatterOptions},
    screen::Screen,
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

const SCRIPT: &str = r#"
for i in $(seq 1 60); do printf '\033[3%dmline %d\033[0m\n' $((i % 8)) "$i"; done
printf '\033[1;4mbold underline\033[0m \033[7minverse\033[0m \033[38;2;255;128;0mtruecolor\033[0m\n'
printf 'soft wrap %0120d\n' 0
printf 'cursor parked here'
printf '\033[?1h\033[?2004h\033[?1000h\033[?1006h\033[?1004h'
printf '\033[?1049h\033[2J\033[H\033[44;97malternate screen\033[0m\033[5;10H\033[1;35mready\033[0m\033[3;7H'
read -r _
printf '\033[?1049l\033[?1l\033[?2004l\033[?1000l\033[?1006l\033[?1004l\nfinished\n'
"#;

#[tokio::test]
async fn a_newly_attached_device_sees_the_holders_screen() {
    let server = Server::start().await;

    let mut watcher = View::open(server.port, "watcher", None).await;
    watcher.settle(|text| text.contains('$')).await;
    watcher.send(&format!("bash {}\r", server.script.display())).await;
    watcher.settle(|text| text.contains("ready")).await;

    let terminal_id = watcher.terminal_id.clone();
    let mut late = View::open(server.port, "late", Some(&terminal_id)).await;
    late.settle(|text| text.contains("ready")).await;
    assert_eq!(late.terminal.active_screen().unwrap(), Screen::Alternate);
    assert_eq!(state(&late.terminal), state(&watcher.terminal));

    watcher.send("\r").await;
    watcher.settle(|text| text.contains("finished")).await;
    late.settle(|text| text.contains("finished")).await;
    assert_eq!(late.terminal.active_screen().unwrap(), Screen::Primary);
    assert!(late.terminal.scrollback_rows().unwrap() > 0);
    assert!(late.text().contains("line 1\n"));
    assert_eq!(state(&late.terminal), state(&watcher.terminal));

    watcher.send("exit\r").await;
    let socket = server.root.path().join(format!("data/terminals/{terminal_id}.sock"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while socket.exists() {
        assert!(Instant::now() < deadline, "the holder outlived its shell");
        sleep(Duration::from_millis(50)).await;
    }
}

fn state(terminal: &Terminal) -> String {
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

struct Server {
    port: u16,
    root: TempDir,
    script: PathBuf,
    _child: Child,
}

impl Server {
    async fn start() -> Self {
        let parent = env::var_os("OIKONOMOS_E2E_ROOT").map_or_else(env::temp_dir, PathBuf::from);
        let root = tempfile::Builder::new().prefix("reconnect").tempdir_in(parent).unwrap();
        fs::create_dir(root.path().join("home")).unwrap();
        let script = root.path().join("script.sh");
        fs::write(&script, SCRIPT).unwrap();
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let child = Command::new(env!("CARGO_BIN_EXE_oikonomos"))
            .args(["serve", "--port", &port.to_string(), "--data-dir"])
            .arg(root.path().join("data"))
            .arg("--config")
            .arg(root.path().join("config.toml"))
            .env("SHELL", "/bin/bash")
            .env("HOME", root.path().join("home"))
            .stdout(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while TcpStream::connect(("127.0.0.1", port)).await.is_err() {
            assert!(Instant::now() < deadline, "the server never listened");
            sleep(Duration::from_millis(50)).await;
        }
        Self {
            port,
            root,
            script,
            _child: child,
        }
    }
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

struct View {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    terminal_id: String,
    channel: u32,
    terminal: Terminal<'static, 'static>,
}

impl View {
    async fn open(port: u16, device_id: &str, terminal_id: Option<&str>) -> Self {
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
        control(&mut socket, json!({ "type": "hello", "deviceId": device_id })).await;
        assert_eq!(next_control(&mut socket).await["type"], "hello");
        let request = match terminal_id {
            Some(id) => json!({ "type": "terminal.attach", "terminalId": id }),
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

    async fn send(&mut self, text: &str) {
        let mut frame = vec![2];
        frame.extend_from_slice(&self.channel.to_be_bytes());
        frame.extend_from_slice(text.as_bytes());
        self.socket.send(Message::Binary(frame.into())).await.unwrap();
    }

    fn text(&self) -> String {
        format(&self.terminal, Format::Plain)
    }

    async fn settle(&mut self, until: impl Fn(&str) -> bool) {
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
