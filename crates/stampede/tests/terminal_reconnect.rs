use crate::common;

use std::{fs, time::Duration};

use common::{Server, View, state};
use libghostty_vt::screen::Screen;
use tokio::time::{Instant, sleep};

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

    watcher.terminal.set_scrollback_max_bytes(None).unwrap();
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
