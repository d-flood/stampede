use crate::common;

use std::{
    fs,
    os::fd::AsRawFd,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use common::{Server, View, state};
use libghostty_vt::screen::Screen;
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    time::{Instant, sleep},
};

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
    let server = Server::start("reconnect").await;
    let script = server.root.path().join("script.sh");
    fs::write(&script, SCRIPT).unwrap();

    let mut watcher = View::open(server.port, "watcher", None).await;
    watcher.settle(|text| text.contains('$')).await;
    watcher.send(&format!("bash {}\r", script.display())).await;
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

    watcher.terminal.set_scrollback_max_bytes(Some(usize::MAX)).unwrap();
    watcher.send("seq 1 2000; echo history-ready\r").await;
    watcher.settle(|text| text.contains("2000\nhistory-ready")).await;
    fs::write(server.root.path().join("config.toml"), "scrollback = 3\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut limited = View::open(server.port, "limited", Some(&terminal_id)).await;
        limited.settle(|text| text.contains("history-ready")).await;
        if limited.terminal.scrollback_rows().unwrap() < 500 {
            assert!(!limited.text().contains("line 1\n"));
            break;
        }
        assert!(Instant::now() < deadline, "the live Terminal ignored the scrollback setting: {} rows", limited.terminal.scrollback_rows().unwrap());
    }

    watcher.send("exit\r").await;
    let socket = server.root.path().join(format!("data/terminals/{terminal_id}.sock"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while socket.exists() {
        assert!(Instant::now() < deadline, "the holder outlived its shell");
        sleep(Duration::from_millis(50)).await;
    }
}

const HELLO: u8 = 1;
const ATTACH: u8 = 2;
const SNAPSHOT: u8 = 3;
const TITLE_CHANGED: u8 = 10;

#[tokio::test]
async fn a_device_is_redrawn_when_the_holder_drops_the_server() {
    let mut server = Server::start("dropped").await;
    server.stop().await;
    let terminals = server.root.path().join("data/terminals");
    fs::create_dir_all(&terminals).unwrap();
    let record = json!({ "id": "held", "cwd": "/", "command": ["bash"], "size": { "cols": 80, "rows": 24 }, "protocol": 7 });
    fs::write(terminals.join("held.json"), record.to_string()).unwrap();
    let directory = fs::File::open(&terminals).unwrap();
    // A socket path must fit in 108 bytes.
    let listener = UnixListener::bind(format!("/proc/self/fd/{}/held.sock", directory.as_raw_fd())).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            tokio::spawn(hold(socket, Arc::clone(&dropped)));
        }
    });
    server.restart().await;

    let mut view = View::open(server.port, "device", Some("held")).await;
    view.settle(|text| text.contains("redrawn")).await;
}

/// Speaks for a holder, dropping the first connection to attach once it is drawn.
async fn hold(mut socket: UnixStream, dropped: Arc<AtomicBool>) {
    let hello = json!({ "protocol": 7, "holder_version": "test" });
    let title = json!({ "title": null, "cwd": "/" });
    for (tag, payload) in [(HELLO, hello), (TITLE_CHANGED, title)] {
        socket.write_all(&frame(tag, payload.to_string().as_bytes())).await.unwrap();
    }
    while let Ok(len) = socket.read_u32().await {
        let mut message = vec![0; len as usize];
        socket.read_exact(&mut message).await.unwrap();
        if message[0] == ATTACH {
            let first = !dropped.swap(true, Ordering::Relaxed);
            let screen: &[u8] = if first { b"drawn" } else { b"redrawn" };
            socket.write_all(&frame(SNAPSHOT, screen)).await.unwrap();
            if first { return }
        }
    }
}

fn frame(tag: u8, payload: &[u8]) -> Vec<u8> {
    [&(payload.len() as u32 + 1).to_be_bytes(), [tag].as_slice(), payload].concat()
}
