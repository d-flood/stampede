mod common;

use std::{env, fs, time::Duration};

use common::{Server, chat_turn, close_chat, control, leftovers, next, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const PROMPT: &str = "Greet the reader in one short sentence with one word in **bold**. Use no names and no tools.";
const THREAD: &str = "01a14644-9d4c-76e3-85ad-597b2ce8db9f";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn a_codex_chat_streams_a_turn_and_closing_stops_its_proxy() {
    let (server, mut socket, chat) = launch("simple-turn").await;
    let chat_id = chat["id"].as_str().unwrap();

    let (statuses, items) = chat_turn(&mut socket, chat_id, &chat["status"], PROMPT).await;
    assert_eq!(statuses, ["done", "working", "done"]);
    assert_eq!(
        items,
        [
            json!({ "kind": "user", "text": PROMPT, "images": 0 }),
            json!({ "kind": "text", "text": "Welcome—**glad** you’re here.", "streaming": false, "phase": "final" }),
        ]
    );

    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    assert_eq!(next_control(&mut socket, "chat.items").await["items"], json!(items));
    let record_path = server.root.path().join(format!("data/chats/{chat_id}.json"));
    let record: Value = serde_json::from_slice(&fs::read(&record_path).unwrap()).unwrap();
    assert_eq!(record["agentSession"], THREAD);
    assert_eq!(record["status"], "done");
    assert_eq!(record["rig"], "codex");
    let running = leftovers(server.root.path());
    assert!(running.iter().any(|process| process.contains("/tests/fake/codex app-server proxy")), "{running:?}");

    close_chat(&mut socket, server.root.path(), chat_id).await;
    let methods: Vec<Value> = received(&server).iter().map(|message| message["method"].clone()).collect();
    assert_eq!(
        methods,
        ["initialize", "initialized", "thread/start", "model/list", "skills/list", "turn/start", "thread/unsubscribe"]
    );
}

#[tokio::test]
async fn closing_a_working_codex_chat_interrupts_its_turn() {
    let (server, mut socket, chat) = launch("close-while-working").await;
    let chat_id = chat["id"].as_str().unwrap();
    let send = json!({ "type": "chat.send", "chatId": chat_id, "text": "Count from 1 to 300, one number per line." });
    control(&mut socket, send).await;
    let streaming = json!({ "kind": "text", "text": "1", "streaming": true, "phase": "final" });
    timeout(Duration::from_secs(20), async {
        while next(&mut socket).await["item"] != streaming {}
    })
    .await
    .expect("the turn never streamed");

    close_chat(&mut socket, server.root.path(), chat_id).await;
    let received = received(&server);
    let methods: Vec<Value> = received.iter().map(|message| message["method"].clone()).collect();
    assert_eq!(
        methods,
        [
            "initialize",
            "initialized",
            "thread/start",
            "model/list",
            "skills/list",
            "turn/start",
            "turn/interrupt",
            "thread/unsubscribe"
        ]
    );
    assert_eq!(
        received[6]["params"],
        json!({ "threadId": "01a1464b-f380-7053-b351-54717787b916", "turnId": "01a1464b-f40b-7c53-af63-840ce5e8a92b" })
    );
}

async fn launch(fixture: &str) -> (Server, Socket, Value) {
    let config = format!(
        r#"
[[rig]]
name = "codex"
agent = "codex"
command = "codex"
env = {{ OIKONOMOS_FAKE_FIXTURE = "{fixture}" }}
"#
    );
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("chat", &config, vec![("PATH", path.into())]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();

    control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "codex" })).await;
    let created = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.created"));
    let chat = created.await.expect("the chat was never created")["chat"].clone();
    assert_eq!(chat["agent"], "codex");
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat["id"] })).await;
    assert_eq!(next_control(&mut socket, "chat.items").await["items"], json!([]));
    (server, socket, chat)
}

/// What the fake daemon was sent, in order.
fn received(server: &Server) -> Vec<Value> {
    let log = fs::read_to_string(server.root.path().join("home/.codex/fake-daemon/received.jsonl")).unwrap();
    log.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}
