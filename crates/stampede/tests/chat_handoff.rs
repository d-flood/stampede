mod common;

use std::{env, fs, time::Duration};

use common::{Server, close_chat, control, next, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::{sleep, timeout}};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const SUMMARY: &str = "Continue implementing authentication.\n\nKeep local-first behavior. Login is unfinished; next inspect src/auth.ts. Verification: no tests run.";

#[tokio::test]
async fn claude_hands_off_to_codex_without_sending_the_draft() {
    handoff("claude-code", "codex", "Authentication only").await;
}

#[tokio::test]
async fn codex_hands_off_to_claude_without_sending_the_draft() {
    handoff("codex", "claude-code", "Authentication only").await;
}

#[tokio::test]
async fn a_chat_can_hand_off_to_the_same_rig_without_a_focus() {
    handoff("claude-code", "claude-code", "").await;
}

async fn handoff(agent: &str, target: &str, focus: &str) {
    let (server, mut socket, source) = launch(agent, target).await;
    control(&mut socket, json!({ "type": "chat.handoff", "chatId": source["id"], "rig": "source", "focus": focus })).await;
    assert!(next_control(&mut socket, "chat.handoff.error").await["error"].as_str().unwrap().contains("Send a message"));
    discuss(&mut socket, &source).await;

    let rig = if agent == target { "source" } else { "target" };
    control(&mut socket, json!({ "type": "chat.handoff", "chatId": source["id"], "rig": rig, "focus": focus })).await;
    gate(&server, &mut socket).await;
    control(&mut socket, json!({ "type": "chat.handoff", "chatId": source["id"], "rig": rig })).await;
    assert!(next_control(&mut socket, "chat.handoff.error").await["error"].as_str().unwrap().contains("Wait for"));
    fs::remove_file(server.root.path().join("home/handoff")).unwrap();
    let created = timeout(Duration::from_secs(15), async {
        loop {
            let message = next(&mut socket).await;
            assert_ne!(message["type"], "chat.handoff.error", "{message}");
            if message["type"] == "chat.created" { break message; }
        }
    }).await.unwrap();
    let chat = &created["chat"];
    assert_ne!(chat["id"], source["id"]);
    assert_eq!(chat["rig"], rig);
    assert_eq!(chat["agent"], target);
    assert_eq!(chat["handoffFrom"], source["id"]);
    assert_eq!(chat["cwd"], source["cwd"]);
    assert_eq!(chat["projectId"], source["projectId"]);
    assert_eq!(chat["draft"], SUMMARY);
    assert_eq!(chat["status"], "done");
    assert_eq!(record(&server, chat)["draft"], SUMMARY);
    assert_eq!(record(&server, chat)["handoffFrom"], source["id"]);
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat["id"] })).await;
    assert_eq!(next_control(&mut socket, "chat.items").await["items"], json!([]));

    let log = if agent == "codex" { "home/.codex/fake-daemon/received.jsonl" } else { "home/source-received.jsonl" };
    let received = fs::read_to_string(server.root.path().join(log)).unwrap();
    let received: Vec<Value> = received.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let prompt = received.iter().filter_map(|message| match agent {
        "claude-code" => message["message"]["content"].as_str(),
        _ => message["params"]["input"][0]["text"].as_str(),
    }).last().unwrap();
    assert!(prompt.starts_with("Write a self-contained handoff"));
    if focus.is_empty() { assert!(!prompt.contains("Focus:")); }
    else { assert!(prompt.ends_with(&format!("Focus: {focus}"))); }
    let target_id = chat["id"].as_str().unwrap();
    control(&mut socket, json!({ "type": "chat.close", "chatId": target_id })).await;
    close_chat(&mut socket, server.root.path(), source["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn interrupting_a_handoff_does_not_create_a_target_chat() {
    let (server, mut socket, source) = launch("claude-code", "codex").await;
    discuss(&mut socket, &source).await;
    control(&mut socket, json!({ "type": "chat.handoff", "chatId": source["id"], "rig": "target" })).await;
    gate(&server, &mut socket).await;
    control(&mut socket, json!({ "type": "chat.interrupt", "chatId": source["id"] })).await;
    assert_eq!(next_control(&mut socket, "chat.handoff.error").await["error"], "Handoff interrupted.");
    fs::remove_file(server.root.path().join("home/handoff")).unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    assert_eq!(next_control(&mut socket, "hello").await["chats"].as_array().unwrap().len(), 1);
    close_chat(&mut socket, server.root.path(), source["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn an_unknown_target_rig_leaves_the_source_chat_untouched() {
    let (server, mut socket, source) = launch("codex", "claude-code").await;
    discuss(&mut socket, &source).await;
    control(&mut socket, json!({ "type": "chat.handoff", "chatId": source["id"], "rig": "missing" })).await;
    assert!(next_control(&mut socket, "chat.handoff.error").await["error"].as_str().unwrap().contains("no longer configured"));
    assert_eq!(record(&server, &source)["status"], "done");
    close_chat(&mut socket, server.root.path(), source["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn a_usage_limit_discards_the_partial_handoff() {
    let directory = tempfile::tempdir().unwrap();
    let recorded = format!("{}/tests/fixtures/chats/claude-code/handoff.synthetic.jsonl", env!("CARGO_MANIFEST_DIR"));
    let mut entries: Vec<Value> = fs::read_to_string(recorded).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let answer = entries.iter_mut().rev().find(|entry| entry["stdout"]["type"] == "assistant").unwrap();
    answer["stdout"]["error"] = json!("rate_limit");
    let fixture = directory.path().join("limited.jsonl");
    fs::write(&fixture, entries.iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
    let (server, mut socket, source) = launch_fixture("claude-code", "codex", fixture.to_str().unwrap()).await;
    discuss(&mut socket, &source).await;
    control(&mut socket, json!({ "type": "chat.handoff", "chatId": source["id"], "rig": "target" })).await;
    gate(&server, &mut socket).await;
    fs::remove_file(server.root.path().join("home/handoff")).unwrap();
    assert!(next_control(&mut socket, "chat.handoff.error").await["error"].as_str().unwrap().contains("did not finish"));
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    assert_eq!(next_control(&mut socket, "hello").await["chats"].as_array().unwrap().len(), 1);
    close_chat(&mut socket, server.root.path(), source["id"].as_str().unwrap()).await;
}

async fn launch(agent: &str, target: &str) -> (Server, Socket, Value) {
    let fixture = format!("{}/tests/fixtures/chats/{agent}/handoff.synthetic.jsonl", env!("CARGO_MANIFEST_DIR"));
    launch_fixture(agent, target, &fixture).await
}

async fn launch_fixture(agent: &str, target: &str, fixture: &str) -> (Server, Socket, Value) {
    let config = format!(r#"
[[rig]]
name = "source"
agent = "{agent}"
env = {{ STAMPEDE_FAKE_FIXTURE = "{fixture}", STAMPEDE_FAKE_GATES = ".", STAMPEDE_FAKE_RECEIVED = "source-received.jsonl" }}

[[rig]]
name = "target"
agent = "{target}"
env = {{ STAMPEDE_FAKE_FIXTURE = "simple-turn" }}
"#);
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("chat-handoff", &config, vec![("PATH", path.into())]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project, "rig": "source" })).await;
    let source = next_control(&mut socket, "chat.created").await["chat"].clone();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": source["id"] })).await;
    next_control(&mut socket, "chat.items").await;
    (server, socket, source)
}

async fn discuss(socket: &mut Socket, chat: &Value) {
    control(socket, json!({ "type": "chat.send", "chatId": chat["id"], "text": "Discuss authentication." })).await;
    loop {
        let message = timeout(Duration::from_secs(10), next_control(socket, "chat.status")).await.unwrap();
        if message["chatId"] == chat["id"] && message["status"] == "done" { break; }
    }
}

async fn gate(server: &Server, socket: &mut Socket) {
    timeout(Duration::from_secs(10), async {
        while !server.root.path().join("home/handoff").exists() {
            tokio::select! {
                _ = sleep(Duration::from_millis(20)) => {},
                message = next(socket) => assert_ne!(message["type"], "chat.handoff.error", "{message}"),
            }
        }
    }).await.unwrap();
}

fn record(server: &Server, chat: &Value) -> Value {
    serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{}.json", chat["id"].as_str().unwrap()))).unwrap()).unwrap()
}
