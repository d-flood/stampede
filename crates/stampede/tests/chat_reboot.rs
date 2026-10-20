mod common;

use std::{env, fs, path::Path, time::Duration};

use common::{Server, apply, close_chat, control, next, next_control, pushed, reboot, subscribe};
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    sync::mpsc::UnboundedReceiver,
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const CONFIG: &str = r#"
[[rig]]
name = "claude"
agent = "claude-code"
command = "claude"
env = { STAMPEDE_FAKE_FIXTURE = "reboot-mid-turn" }

[[rig]]
name = "codex"
agent = "codex"
command = "codex"
env = { STAMPEDE_FAKE_FIXTURE = "daemon-restart" }
"#;

const GREET: &str = "Say hello in exactly three words.";
const COUNT_WORDS: &str = "Count from 1 to 60 in words, one per line, then say Done. No tools.";
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
async fn a_claude_chat_whose_holder_is_gone_resumes_its_session_and_is_interrupted() {
    let (mut server, gates, mut pushes) = start().await;
    let (mut socket, project_id) = open(server.port).await;
    let chat_id = create(&mut socket, &project_id, "claude").await;
    let mut view = View::default();
    turn(&mut socket, &chat_id, &mut view, GREET).await;
    pushed(&mut pushes).await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": COUNT_WORDS })).await;
    let partial = "One\nTwo\nThree\nFour\nFive\nSix\nSeven\nEight\nNine\nTen\nEleven\nTwel";
    view.follow(&mut socket, &chat_id, |view| view.items.get(3).is_some_and(|item| item["text"] == partial)).await;
    reached(gates.path(), "stop").await;
    queue(&mut socket, &chat_id).await;

    server.stop().await;
    reboot(server.root.path());
    fs::remove_file(gates.path().join("stop")).unwrap();
    assert!(server.root.path().join(format!("data/chats/{chat_id}.sock")).exists());
    server.restart().await;

    let (mut socket, mut view) = reconnect(server.port, &chat_id).await;
    let interrupted = [
        json!({ "kind": "user", "text": GREET, "images": 0 }),
        json!({ "kind": "text", "text": "Hello there, friend!", "streaming": false, "phase": "final" }),
        json!({ "kind": "user", "text": COUNT_WORDS, "images": 0 }),
        json!({ "kind": "notice", "notice": "interrupted", "text": "Interrupted" }),
    ];
    view.follow(&mut socket, &chat_id, |view| view.status == "done" && view.attention && view.items.len() == 4).await;
    assert_eq!(view.items, interrupted);
    let item = json!({ "projectId": project_id, "chatId": chat_id, "label": "claude", "status": "done" });
    assert_eq!(pushed(&mut pushes).await, ("away".into(), item));
    let record = record(&server, &chat_id);
    assert_eq!((&record["status"], &record["viewedSinceDone"]), (&json!("done"), &json!(false)));

    assert_eq!(record["queued"], json!(["Leave this queued"]));
    control(&mut socket, json!({ "type": "chat.interrupt", "chatId": chat_id })).await;
    assert_eq!(next_control(&mut socket, "chat.queued").await["queued"], json!([]));
    turn(&mut socket, &chat_id, &mut view, SAY).await;
    assert_eq!(view.statuses, ["working", "done"]);
    let said = [
        json!({ "kind": "user", "text": SAY, "images": 0 }),
        json!({ "kind": "thinking", "seconds": 0 }),
        json!({ "kind": "text", "text": "Done", "streaming": false, "phase": "final" }),
    ];
    assert_eq!(view.items[4..], said);
    close_chat(&mut socket, server.root.path(), &chat_id).await;
}

#[tokio::test]
async fn a_codex_chat_whose_daemon_restarts_mid_turn_is_interrupted() {
    let (server, gates, mut pushes) = start().await;
    let (mut socket, project_id) = open(server.port).await;
    let chat_id = create(&mut socket, &project_id, "codex").await;
    let mut view = View::default();
    count(&mut socket, &chat_id, &mut view, gates.path()).await;

    fs::remove_file(gates.path().join("stop")).unwrap();
    view.follow(&mut socket, &chat_id, |view| view.status == "done" && view.attention).await;
    assert_eq!(view.items, interrupted_count());
    let item = json!({ "projectId": project_id, "chatId": chat_id, "label": "codex", "status": "done" });
    assert_eq!(pushed(&mut pushes).await, ("away".into(), item));

    said(&mut socket, &server, &chat_id, &mut view, false).await;
}

#[tokio::test]
async fn a_codex_chat_whose_daemon_forgot_its_turn_across_a_reboot_is_interrupted() {
    let (mut server, gates, mut pushes) = start().await;
    let (mut socket, project_id) = open(server.port).await;
    let chat_id = create(&mut socket, &project_id, "codex").await;
    let mut view = View::default();
    count(&mut socket, &chat_id, &mut view, gates.path()).await;

    server.stop().await;
    fs::remove_file(gates.path().join("stop")).unwrap();
    reboot(server.root.path());
    server.restart().await;

    let (mut socket, mut view) = reconnect(server.port, &chat_id).await;
    view.follow(&mut socket, &chat_id, |view| view.status == "done" && view.attention && view.items.len() == 2).await;
    assert_eq!(view.items, interrupted_count());
    let item = json!({ "projectId": project_id, "chatId": chat_id, "label": "codex", "status": "done" });
    assert_eq!(pushed(&mut pushes).await, ("away".into(), item));
    assert_eq!(record(&server, &chat_id)["viewedSinceDone"], false);

    said(&mut socket, &server, &chat_id, &mut view, true).await;
}

async fn start() -> (Server, tempfile::TempDir, UnboundedReceiver<(String, Value)>) {
    let gates = tempfile::tempdir().unwrap();
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let env = vec![("PATH", path.into()), ("STAMPEDE_FAKE_GATES", gates.path().into())];
    let server = Server::with_env("chat-reboot", CONFIG, env).await;
    let pushes = subscribe(&server, &["away"]).await;
    (server, gates, pushes)
}

async fn open(port: u16) -> (Socket, Value) {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    (socket, project_id)
}

async fn create(socket: &mut Socket, project_id: &Value, rig: &str) -> String {
    control(socket, json!({ "type": "chat.create", "projectId": project_id, "rig": rig })).await;
    let chat_id = next_control(socket, "chat.created").await["chat"]["id"].as_str().unwrap().to_owned();
    control(socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    assert_eq!(next_control(socket, "chat.items").await["items"], json!([]));
    chat_id
}

async fn turn(socket: &mut Socket, chat_id: &str, view: &mut View, text: &str) {
    view.statuses.clear();
    control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": text })).await;
    view.follow(socket, chat_id, |view| view.statuses.last().is_some_and(|status| status == "done")).await;
}

async fn count(socket: &mut Socket, chat_id: &str, view: &mut View, gates: &Path) {
    control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": COUNT })).await;
    view.follow(socket, chat_id, |view| view.items.get(1).is_some_and(|item| item["text"] == "1\n2\n3")).await;
    reached(gates, "stop").await;
    assert_eq!(view.status, "working");
    queue(socket, chat_id).await;
}

async fn queue(socket: &mut Socket, chat_id: &str) {
    control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Leave this queued", "queue": true })).await;
    assert_eq!(next_control(socket, "chat.queued").await["queued"], json!(["Leave this queued"]));
}

fn interrupted_count() -> [Value; 2] {
    [
        json!({ "kind": "user", "text": COUNT, "images": 0 }),
        json!({ "kind": "notice", "notice": "interrupted", "text": "Interrupted" }),
    ]
}

async fn said(socket: &mut Socket, server: &Server, chat_id: &str, view: &mut View, restored: bool) {
    assert_eq!(record(server, chat_id)["queued"], json!(["Leave this queued"]));
    control(socket, json!({ "type": "chat.interrupt", "chatId": chat_id })).await;
    assert_eq!(next_control(socket, "chat.queued").await["queued"], json!([]));
    turn(socket, chat_id, view, SAY).await;
    assert_eq!(view.statuses, ["working", "done"]);
    let said = [
        json!({ "kind": "user", "text": SAY, "images": 0 }),
        json!({ "kind": "text", "text": "Done", "streaming": false, "phase": "final" }),
    ];
    assert_eq!(view.items[2..], said);
    close_chat(socket, server.root.path(), chat_id).await;
    let log = fs::read_to_string(server.root.path().join("home/.codex/fake-daemon/received.jsonl")).unwrap();
    let methods: Vec<Value> =
        log.lines().map(|line| serde_json::from_str::<Value>(line).unwrap()["method"].clone()).collect();
    let mut expected = vec![
            "initialize",
            "initialized",
            "thread/start",
            "model/list",
            "skills/list",
            "turn/start",
            "initialize",
            "initialized",
            "thread/resume",
            "turn/start",
            "thread/unsubscribe",
    ];
    if restored {
        expected.splice(9..9, ["model/list", "skills/list"]);
    }
    assert_eq!(methods, expected);
}

fn record(server: &Server, chat_id: &str) -> Value {
    serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{chat_id}.json"))).unwrap()).unwrap()
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
                    "chat.items" if message["chatId"] == chat_id => {
                        self.items = message["items"].as_array().unwrap().clone();
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

async fn reached(gates: &Path, name: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !gates.join(name).exists() {
        assert!(Instant::now() < deadline, "the fake never reached gate {name}");
        sleep(Duration::from_millis(20)).await;
    }
}
