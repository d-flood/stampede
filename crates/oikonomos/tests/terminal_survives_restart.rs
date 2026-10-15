mod common;

use std::fs;

use common::{Server, View, state};

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

fn marker(text: &str) -> Option<u32> {
    text.lines().find_map(|line| line.strip_prefix("marker ")?.trim().parse().ok())
}

fn alive(pid: u32) -> bool {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
    stat.rsplit_once(") ").is_some_and(|(_, fields)| !fields.starts_with('Z'))
}
