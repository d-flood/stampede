mod common;

use std::{env, fs, ops::RangeInclusive, path::Path, time::Duration};

use common::{Server, apply, close_chat, control, next, next_control};
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const CONFIG: &str = r#"
[[rig]]
name = "codex"
agent = "codex"
command = "codex"
env = { STAMPEDE_FAKE_FIXTURE = "restart-mid-turn" }
"#;

const THREAD: &str = "01a1466b-3e4b-7bc0-8663-e1c3dba129e8";
const COUNT: &str = "Count from 1 to 60, one number per line. No tools.";
const SAY: &str = "Now say Done, and nothing else.";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Default)]
struct View {
    status: Value,
    statuses: Vec<Value>,
    attention: bool,
    items: Vec<Value>,
}

#[tokio::test]
async fn a_codex_chat_resumes_its_thread_through_server_restarts() {
    let gates = tempfile::tempdir().unwrap();
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let env = vec![("PATH", path.into()), ("STAMPEDE_FAKE_GATES", gates.path().into())];
    let mut server = Server::with_env("chat-codex-restart", CONFIG, env).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "codex" })).await;
    let chat_id = next_control(&mut socket, "chat.created").await["chat"]["id"].as_str().unwrap().to_owned();
    let scratch = server.root.path().join("data/storage/scratch").join(&chat_id);
    fs::write(scratch.join("keep"), "survives restart").unwrap();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    assert_eq!(next_control(&mut socket, "chat.items").await["items"], json!([]));
    let mut view = View::default();

    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": COUNT })).await;
    let before = counted(1..=5);
    view.follow(&mut socket, &chat_id, |view| view.items.get(1).is_some_and(|item| item["text"] == before)).await;
    gated(gates.path(), "stop").await;
    server.stop().await;
    fs::remove_file(gates.path().join("stop")).unwrap();
    server.restart().await;
    assert_eq!(fs::read_to_string(scratch.join("keep")).unwrap(), "survives restart");

    let (mut socket, mut view) = reconnect(server.port, &chat_id).await;
    assert_eq!(view.status, "working");
    assert!(!view.attention);
    let during = counted(28..=40);
    view.follow(&mut socket, &chat_id, |view| view.items.get(1).is_some_and(|item| item["text"] == during)).await;
    assert_eq!(
        view.items,
        [
            json!({ "kind": "user", "text": COUNT, "images": 0 }),
            json!({ "kind": "text", "text": during, "streaming": true, "phase": "final" }),
        ]
    );
    assert!(view.statuses.is_empty(), "{:?}", view.statuses);

    gated(gates.path(), "resumed").await;
    fs::remove_file(gates.path().join("resumed")).unwrap();
    view.follow(&mut socket, &chat_id, |view| !view.statuses.is_empty()).await;
    assert_eq!(view.statuses, ["done"]);
    let finished = [
        json!({ "kind": "user", "text": COUNT, "images": 0 }),
        json!({ "kind": "text", "text": counted(1..=60).trim_end(), "streaming": false, "phase": "final" }),
    ];
    assert_eq!(view.items, finished);

    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": SAY })).await;
    view.follow(&mut socket, &chat_id, |view| view.status == "working").await;
    gated(gates.path(), "stop again").await;
    server.stop().await;
    fs::remove_file(gates.path().join("stop again")).unwrap();
    server.restart().await;

    let (mut socket, mut view) = reconnect(server.port, &chat_id).await;
    view.follow(&mut socket, &chat_id, |view| view.status == "done" && view.attention && view.items.len() == 4).await;
    assert_eq!(
        view.items,
        [
            finished[0].clone(),
            finished[1].clone(),
            json!({ "kind": "user", "text": SAY, "images": 0 }),
            json!({ "kind": "text", "text": "Done", "streaming": false, "phase": "final" }),
        ]
    );
    let record = fs::read(server.root.path().join(format!("data/chats/{chat_id}.json"))).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&record).unwrap()["status"], "done");

    close_chat(&mut socket, server.root.path(), &chat_id).await;
    let received = received(server.root.path());
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
            "initialize",
            "initialized",
            "thread/resume",
            "model/list",
            "skills/list",
            "turn/start",
            "initialize",
            "initialized",
            "thread/resume",
            "model/list",
            "skills/list",
            "thread/unsubscribe",
        ]
    );
    assert_eq!(common::without_storage(&received[8]["params"]), json!({ "threadId": THREAD, "config": {
        "tools.update_plan": { "enabled": true },
        "mcp_servers.stampede_chat": { "command": env!("CARGO_BIN_EXE_stampede"), "args": ["chat-tools", "--data-dir", server.root.path().join("data"), "--chat-id", chat_id], "tool_timeout_sec": 86400 },
    } }));
    assert_eq!(common::without_storage(&received[14]["params"]), json!({ "threadId": THREAD, "config": {
        "tools.update_plan": { "enabled": true },
        "mcp_servers.stampede_chat": { "command": env!("CARGO_BIN_EXE_stampede"), "args": ["chat-tools", "--data-dir", server.root.path().join("data"), "--chat-id", chat_id], "tool_timeout_sec": 86400 },
    } }));
}

fn counted(numbers: RangeInclusive<u32>) -> String {
    numbers.map(|number| format!("{number}\n")).collect()
}

async fn reconnect(port: u16, chat_id: &str) -> (Socket, View) {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    let hello = next_control(&mut socket, "hello").await;
    let chat = hello["chats"].as_array().unwrap().iter().find(|chat| chat["id"] == chat_id).cloned();
    let status = chat.expect("the chat was not restored")["status"].clone();
    let attention = next_control(&mut socket, "attention.changed").await;
    let attention = attention["items"].as_array().unwrap().iter().any(|item| item["chatId"] == chat_id);
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    let items = next_control(&mut socket, "chat.items").await["items"].as_array().unwrap().clone();
    (socket, View { status, statuses: Vec::new(), attention, items })
}

impl View {
    async fn follow(&mut self, socket: &mut Socket, chat_id: &str, until: impl Fn(&Self) -> bool) {
        timeout(Duration::from_secs(20), async {
            while !until(self) {
                let message = next(socket).await;
                match message["type"].as_str().unwrap() {
                    "attention.changed" => {
                        let items = message["items"].as_array().unwrap();
                        self.attention = items.iter().any(|item| item["chatId"] == chat_id);
                    }
                    "chat.status" if message["chatId"] == chat_id => {
                        self.status = message["status"].clone();
                        self.statuses.push(message["status"].clone());
                    }
                    "chat.item" | "chat.append" if message["chatId"] == chat_id => apply(&mut self.items, &message),
                    _ => {}
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out with {} {:?} {:?}", self.status, self.statuses, self.items));
    }
}

async fn gated(gates: &Path, name: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !gates.join(name).exists() {
        assert!(Instant::now() < deadline, "the fake never reached gate {name}");
        sleep(Duration::from_millis(20)).await;
    }
}

/// What the fake daemon was sent, in order, across every proxy.
fn received(root: &Path) -> Vec<Value> {
    let log = fs::read_to_string(root.join("home/.codex/fake-daemon/received.jsonl")).unwrap();
    log.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}
