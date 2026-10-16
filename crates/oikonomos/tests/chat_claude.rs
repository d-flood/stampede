mod common;

use std::{env, fs, path::Path, time::Duration};

use common::{Server, control, next_control};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

const CONFIG: &str = r#"
[[rig]]
name = "claude"
agent = "claude-code"
command = "claude"
env = { OIKONOMOS_FAKE_FIXTURE = "simple-turn" }
"#;

const PROMPT: &str = "Greet the reader in one short sentence with one word in **bold**. Use no names and no tools.";
const SESSION: &str = "5678b8c0-71a1-4040-8ccc-e6bcaf84f90e";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

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

    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": PROMPT })).await;
    let mut statuses = vec![chat["status"].clone()];
    let mut items: Vec<Value> = Vec::new();
    timeout(Duration::from_secs(20), async {
        while statuses.len() < 2 || statuses.last().unwrap() != "done" {
            let message = next(&mut socket).await;
            if message["chatId"] != *chat_id {
                continue;
            }
            match message["type"].as_str().unwrap() {
                "chat.status" => statuses.push(message["status"].clone()),
                "chat.item" => {
                    let index = message["index"].as_u64().unwrap() as usize;
                    assert!(index <= items.len(), "{message}");
                    match items.get_mut(index) {
                        Some(item) => *item = message["item"].clone(),
                        None => items.push(message["item"].clone()),
                    }
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the turn never ended: {statuses:?} {items:?}"));
    assert_eq!(statuses, ["done", "working", "done"]);
    assert_eq!(
        items,
        [
            json!({ "kind": "user", "text": PROMPT, "images": 0 }),
            json!({ "kind": "text", "text": "Hello and **welcome**! 👋", "streaming": false }),
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

    control(&mut socket, json!({ "type": "chat.close", "chatId": chat_id })).await;
    timeout(Duration::from_secs(10), async {
        loop {
            let changed = next_control(&mut socket, "chats.changed").await;
            if !changed["chats"].as_array().unwrap().iter().any(|chat| chat["id"] == *chat_id) {
                break;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the chat never closed; running: {:?}", leftovers(server.root.path())));
    assert!(!record_path.exists());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let running = leftovers(server.root.path());
        if running.is_empty() {
            break;
        }
        assert!(Instant::now() < deadline, "left running: {running:?}");
        sleep(Duration::from_millis(50)).await;
    }
}

async fn next(socket: &mut Socket) -> Value {
    loop {
        if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

/// Every process but the server that names the test's root in its command line or runs inside it.
fn leftovers(root: &Path) -> Vec<String> {
    let canonical = fs::canonicalize(root).unwrap();
    let root = root.to_str().unwrap();
    fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|entry| {
            entry.file_name().to_str()?.parse::<u32>().ok()?;
            let cmdline = String::from_utf8_lossy(&fs::read(entry.path().join("cmdline")).ok()?).replace('\0', " ");
            let inside = fs::read_link(entry.path().join("cwd")).is_ok_and(|cwd| cwd.starts_with(&canonical));
            ((cmdline.contains(root) || inside) && !cmdline.contains(" serve ")).then_some(cmdline)
        })
        .collect()
}
