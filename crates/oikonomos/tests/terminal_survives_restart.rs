mod common;

use std::{fs, time::Duration};

use common::{Server, View, state};
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn restarting_the_server_never_ends_a_terminal() {
    let mut server = Server::start("restart").await;

    let mut before = View::open(server.port, "device", None).await;
    before.settle(|text| text.contains('$')).await;
    before.send("printf 'marker %d\\n' $$; read -r _\r").await;
    before.settle(|text| marker(text).is_some()).await;
    let pid = marker(&before.text()).unwrap();
    let screen = state(&before.terminal);

    server.restart().await;

    let mut after = View::open(server.port, "device", Some(&before.terminal_id)).await;
    after.settle(|text| marker(text).is_some()).await;
    assert!(alive(pid), "the Terminal's shell {pid} ended with the server");
    assert_eq!(state(&after.terminal), screen);
}

#[tokio::test]
async fn a_terminal_command_can_restart_its_own_server_and_finish() {
    let mut server = Server::start("self-restart").await;
    let mut before = View::open(server.port, "device", None).await;
    before.settle(|text| text.contains('$')).await;
    let requested = server.root.path().join("requested");
    let finished = server.root.path().join("finished");
    before.send(&format!(
        "kill -TERM {}; touch '{}'; sleep 0.5; printf 'survived %d\\n' $$ | tee '{}'\r",
        server.pid(), requested.display(), finished.display(),
    )).await;
    timeout(Duration::from_secs(5), async {
        while !requested.exists() { sleep(Duration::from_millis(20)).await; }
    }).await.unwrap();
    server.restart().await;
    let mut after = View::open(server.port, "device", Some(&before.terminal_id)).await;
    after.settle(|text| text.lines().any(|line| line.starts_with("survived "))).await;
    let output = fs::read_to_string(finished).unwrap();
    let pid = output.trim().strip_prefix("survived ").unwrap().parse().unwrap();
    assert!(alive(pid));
    assert!(after.text().contains(output.trim()));
}

fn marker(text: &str) -> Option<u32> {
    text.lines().find_map(|line| line.strip_prefix("marker ")?.trim().parse().ok())
}

fn alive(pid: u32) -> bool {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
    stat.rsplit_once(") ").is_some_and(|(_, fields)| !fields.starts_with('Z'))
}
