mod common;

use std::{fs, time::Duration};

use common::{Server, View, state};
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn an_interrupted_test_owner_ends_its_server_and_terminal() {
    let mut owner = tokio::process::Command::new("sleep").arg("60").kill_on_drop(true).spawn().unwrap();
    let pid = owner.id().unwrap();
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let started = stat.rsplit_once(") ").unwrap().1.split_whitespace().nth(19).unwrap();
    let identity = format!("{pid}:{started}");
    let server = Server::with_env("interrupted-owner", "", vec![("STAMPEDE_TEST_OWNER", identity.into())]).await;
    let mut view = View::open(server.port, "device", None).await;
    view.settle(|text| text.contains('$')).await;
    view.send("printf 'marker %d\\n' $$; read -r _\r").await;
    view.settle(|text| marker(text).is_some()).await;
    let shell = marker(&view.text()).unwrap();
    drop(view);
    owner.kill().await.unwrap();
    timeout(Duration::from_secs(5), async {
        while alive(shell) || tokio::net::TcpStream::connect(("127.0.0.1", server.port)).await.is_ok() {
            sleep(Duration::from_millis(25)).await;
        }
    }).await.expect("an interrupted test kept its processes alive");
}

#[tokio::test]
async fn dropping_a_test_server_reaps_its_detached_terminal() {
    let server = Server::start("teardown").await;
    let mut view = View::open(server.port, "device", None).await;
    view.settle(|text| text.contains('$')).await;
    view.send("printf 'marker %d\\n' $$; read -r _\r").await;
    view.settle(|text| marker(text).is_some()).await;
    let pid = marker(&view.text()).unwrap();
    drop(view);
    drop(server);
    timeout(Duration::from_secs(3), async {
        while alive(pid) { sleep(Duration::from_millis(25)).await; }
    }).await.expect("a test Terminal's shell survived its owning test server");
}

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
