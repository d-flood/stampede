use crate::common;

use std::{env, fs};

use common::{Server, chat_turn, close_chat, control, leftovers, next_control};
use serde_json::{Value, json};
use tokio_tungstenite::connect_async;

const CONFIG: &str = r#"
[[rig]]
name = "claude"
agent = "claude-code"
command = "claude"
env = { STAMPEDE_FAKE_FIXTURE = "simple-turn" }
"#;

const PROMPT: &str = "Greet the reader in one short sentence with one word in **bold**. Use no names and no tools.";
const SESSION: &str = "5678b8c0-71a1-4040-8ccc-e6bcaf84f90e";

#[tokio::test]
async fn a_claude_chat_streams_a_turn_and_closing_stops_its_agent() {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("chat", CONFIG, vec![("PATH", path.into())]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();

    control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "claude" })).await;
    let chat = next_control(&mut socket, "chat.created").await["chat"].clone();
    let chat_id = chat["id"].as_str().unwrap().to_owned();
    assert_eq!(chat["agent"], "claude-code");
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    let attached = next_control(&mut socket, "chat.items").await;
    assert_eq!(attached["items"], json!([]));

    let (statuses, items) = chat_turn(&mut socket, &chat_id, &chat["status"], PROMPT).await;
    assert_eq!(statuses, ["done", "working", "done"]);
    assert_eq!(
        items,
        [
            json!({ "kind": "user", "text": PROMPT, "images": 0 }),
            json!({ "kind": "text", "text": "Hello and **welcome**! 👋", "streaming": false, "phase": "final" }),
        ]
    );

    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    assert_eq!(next_control(&mut socket, "chat.items").await["items"], json!(items));
    let record_path = server.root.path().join(format!("data/chats/{chat_id}.json"));
    let record: Value = serde_json::from_slice(&fs::read(&record_path).unwrap()).unwrap();
    assert_eq!(record["agentSession"], SESSION);
    assert_eq!(record["status"], "done");
    assert_eq!(record["rig"], "claude");
    let running = leftovers(server.root.path());
    assert!(running.iter().any(|process| process.contains(" hold-chat ")), "{running:?}");
    assert!(running.iter().any(|process| process.contains("/tests/fake/claude ")), "{running:?}");

    close_chat(&mut socket, server.root.path(), &chat_id).await;
}
