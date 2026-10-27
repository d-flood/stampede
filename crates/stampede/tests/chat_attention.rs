use crate::common;

use std::{env, fs, time::Duration};

use common::{Server, control, next_control, pushed, subscribe};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

const CONFIG: &str = r#"
[[rig]]
name = "claude"
agent = "claude-code"
command = "claude"
env = { STAMPEDE_FAKE_FIXTURE = "simple-turn" }
"#;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn a_finished_chat_needs_attention_until_viewed_and_pushes_to_browsers_not_showing_it() {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("attention", CONFIG, vec![("PATH", path.into())]).await;
    let mut pushes = subscribe(&server, &["showing", "away"]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device", "browserId": "showing" })).await;
    next_control(&mut socket, "hello").await;
    assert_eq!(feed(&mut socket).await, json!([]));
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();

    let unseen = start(&mut socket, &project_id, false).await;
    let item = json!({ "projectId": project_id, "chatId": unseen, "label": "claude", "status": "done" });
    assert_eq!(feed(&mut socket).await, json!([item]));
    let mut called = [pushed(&mut pushes).await, pushed(&mut pushes).await];
    called.sort_by_key(|(browser, _)| browser.clone());
    assert_eq!(called, [("away".into(), item.clone()), ("showing".into(), item)]);

    control(&mut socket, json!({ "type": "device.view", "terminalIds": [], "chatIds": [unseen] })).await;
    assert_eq!(feed(&mut socket).await, json!([]));
    let record = server.root.path().join(format!("data/chats/{unseen}.json"));
    let record: Value = serde_json::from_slice(&fs::read(record).unwrap()).unwrap();
    assert_eq!(record["viewedSinceDone"], true);

    let seen = start(&mut socket, &project_id, true).await;
    let item = json!({ "projectId": project_id, "chatId": seen, "label": "claude", "status": "done" });
    assert_eq!(pushed(&mut pushes).await, ("away".into(), item));
    assert!(pushes.try_recv().is_err(), "a browser showing the Chat was pushed to");
    control(&mut socket, json!({ "type": "hello", "deviceId": "device", "browserId": "showing" })).await;
    assert_eq!(feed(&mut socket).await, json!([]));
}

#[tokio::test]
async fn a_done_chat_with_a_running_shell_waits_for_the_shell_before_it_needs_attention() {
    let gates = tempfile::tempdir().unwrap();
    let fixture = format!("{}/tests/fixtures/chats/claude-code/background-shell.synthetic.jsonl", env!("CARGO_MANIFEST_DIR"));
    let config = format!("[[rig]]\nname = \"claude\"\nagent = \"claude-code\"\ncommand = \"claude\"\nenv = {{ STAMPEDE_FAKE_FIXTURE = {fixture:?}, STAMPEDE_FAKE_GATES = {:?} }}\n", gates.path());
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("background", &config, vec![("PATH", path.into())]).await;
    let mut pushes = subscribe(&server, &["away"]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device", "browserId": "away" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    let chat_id = start(&mut socket, &project_id, false).await;

    let seen = settled(&mut socket, &chat_id).await;
    assert!(seen.contains(&json!(["chat.background", true])), "the shell is running when the turn ends: {seen:?}");
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(pushes.try_recv().is_err(), "a done Chat with a running shell was pushed");

    let gate = gates.path().join("shell");
    timeout(Duration::from_secs(10), async { while !gate.exists() { tokio::time::sleep(Duration::from_millis(20)).await; } }).await.unwrap();
    fs::remove_file(&gate).unwrap();
    assert_eq!(settled(&mut socket, &chat_id).await, [json!(["chat.background", false]), json!(["chat.status", "working"]), json!(["chat.status", "done"])]);
    let item = json!({ "projectId": project_id, "chatId": chat_id, "label": "claude", "status": "done" });
    assert_eq!(pushed(&mut pushes).await, ("away".into(), item));
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(pushes.try_recv().is_err(), "the resumed turn was announced twice");
}

#[tokio::test]
async fn a_shell_that_finishes_without_the_agent_resuming_announces_the_chat_once() {
    let gates = tempfile::tempdir().unwrap();
    let fixture = format!("{}/tests/fixtures/chats/claude-code/background-shell-idle.synthetic.jsonl", env!("CARGO_MANIFEST_DIR"));
    let config = format!("[[rig]]\nname = \"claude\"\nagent = \"claude-code\"\ncommand = \"claude\"\nenv = {{ STAMPEDE_FAKE_FIXTURE = {fixture:?}, STAMPEDE_FAKE_GATES = {:?} }}\n", gates.path());
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("background-idle", &config, vec![("PATH", path.into())]).await;
    let mut pushes = subscribe(&server, &["away"]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device", "browserId": "away" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    let chat_id = start(&mut socket, &project_id, false).await;
    settled(&mut socket, &chat_id).await;
    let gate = gates.path().join("shell");
    timeout(Duration::from_secs(10), async { while !gate.exists() { tokio::time::sleep(Duration::from_millis(20)).await; } }).await.unwrap();
    fs::remove_file(&gate).unwrap();
    let item = json!({ "projectId": project_id, "chatId": chat_id, "label": "claude", "status": "done" });
    assert_eq!(feed(&mut socket).await, json!([item]));
    assert_eq!(pushed(&mut pushes).await, ("away".into(), item));
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(pushes.try_recv().is_err(), "the Chat was announced twice");
}

/// The Chat's status and background changes until it is next done.
async fn settled(socket: &mut Socket, chat_id: &str) -> Vec<Value> {
    let mut seen = Vec::new();
    timeout(Duration::from_secs(10), async {
        while !seen.contains(&json!(["chat.status", "done"])) {
            let Message::Text(text) = socket.next().await.unwrap().unwrap() else { continue };
            let message: Value = serde_json::from_str(&text).unwrap();
            if message["chatId"] == chat_id && matches!(message["type"].as_str(), Some("chat.status" | "chat.background")) {
                seen.push(json!([message["type"], message.get("status").or(message.get("background"))]));
            }
        }
    }).await.expect("the Chat never finished");
    seen
}

/// Creates a Chat, shown on the socket's Device or not, and starts a turn that runs to done.
async fn start(socket: &mut Socket, project_id: &Value, shown: bool) -> String {
    control(socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "claude" })).await;
    let chat_id = next_control(socket, "chat.created").await["chat"]["id"].as_str().unwrap().to_owned();
    let shown: &[&str] = if shown { &[&chat_id] } else { &[] };
    control(socket, json!({ "type": "device.view", "terminalIds": [], "chatIds": shown })).await;
    control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Greet the reader" })).await;
    chat_id
}

/// The attention feed's next change.
async fn feed(socket: &mut Socket) -> Value {
    timeout(Duration::from_secs(20), async {
        loop {
            if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
                let mut message: Value = serde_json::from_str(&text).unwrap();
                if message["type"] == "attention.changed" {
                    return message["items"].take();
                }
            }
        }
    })
    .await
    .expect("the attention feed never changed")
}
