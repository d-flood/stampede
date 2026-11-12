use crate::common;

use std::time::Duration;

use common::{Server, View, leftovers};
use tokio::time::{Instant, sleep};

#[tokio::test]
async fn a_running_terminal_says_so_and_closing_it_stops_what_escaped_its_session() {
    let server = Server::start("close").await;
    let mut view = View::open(server.port, "device", None).await;
    view.settle(|text| text.contains('$')).await;
    assert_eq!(view.running, Vec::<String>::new());

    view.send("setsid sleep 600 & sleep 700\r").await;
    view.wait(|view| view.running == ["sleep 700"]).await;
    view.close().await;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let running: Vec<_> = leftovers(server.root.path()).into_iter().filter(|process| process.starts_with("sleep ")).collect();
        if running.is_empty() {
            break;
        }
        assert!(Instant::now() < deadline, "left running: {running:?}");
        sleep(Duration::from_millis(50)).await;
    }
}
