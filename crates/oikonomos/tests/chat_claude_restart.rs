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
env = { OIKONOMOS_FAKE_FIXTURE = "long-turn" }
"#;

const GREET: &str = "Say hello in exactly three words.";
const GREETING: &str = "Hello there, friend!";
const COUNT: &str = "Count from 1 to 40 in words, one per line, then say Done. No tools.";
const COUNTED: &str = "One\nTwo\nThree\nFour\nFive\nSix\nSeven\nEight\nNine\nTen\nEleven\nTwelve\nThirteen\nFourteen\nFifteen\nSixteen\nSeventeen\nEighteen\nNineteen\nTwenty\nTwenty-one\nTwenty-two\nTwenty-three\nTwenty-four\nTwenty-five\nTwenty-six\nTwenty-seven\nTwenty-eight\nTwenty-nine\nThirty\nThirty-one\nThirty-two\nThirty-three\nThirty-four\nThirty-five\nThirty-six\nThirty-seven\nThirty-eight\nThirty-nine\nForty\nDone";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Default)]
struct View {
    items: Vec<Value>,
    statuses: Vec<Value>,
}

#[tokio::test]
async fn a_claude_chat_streams_on_through_a_server_restart() {
    let gates = tempfile::tempdir().unwrap();
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let env = vec![("PATH", path.into()), ("OIKONOMOS_FAKE_GATES", gates.path().into())];
    let mut server = Server::with_env("chat-restart", CONFIG, env).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "claude" })).await;
    let chat_id = next_control(&mut socket, "chat.created").await["chat"]["id"].as_str().unwrap().to_owned();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    let mut view = View::default();
    assert_eq!(next_control(&mut socket, "chat.items").await["items"], json!([]));

    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": GREET })).await;
    view.follow(&mut socket, &chat_id, |view| view.statuses.last().is_some_and(|status| status == "done")).await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": COUNT })).await;
    let before = &COUNTED[..COUNTED.find("Fifte").unwrap() + "Fifte".len()];
    view.follow(&mut socket, &chat_id, |view| view.items.get(3).is_some_and(|item| item["text"] == before)).await;
    gated(gates.path(), "stop").await;

    server.stop().await;
    fs::remove_file(gates.path().join("stop")).unwrap();
    gated(gates.path(), "restart").await;
    server.restart().await;

    let (mut socket, status, attention, mut view) = reconnect(server.port, &chat_id).await;
    assert_eq!(status, "working");
    assert!(!attention);
    let during = &COUNTED[..COUNTED.find("\nThirty").unwrap()];
    view.follow(&mut socket, &chat_id, |view| view.items.get(3).is_some_and(|item| item["text"] == during)).await;
    assert_eq!(
        view.items,
        [
            json!({ "kind": "user", "text": GREET, "images": 0 }),
            json!({ "kind": "text", "text": GREETING, "streaming": false }),
            json!({ "kind": "user", "text": COUNT, "images": 0 }),
            json!({ "kind": "text", "text": during, "streaming": true }),
        ]
    );

    fs::remove_file(gates.path().join("restart")).unwrap();
    view.follow(&mut socket, &chat_id, |view| !view.statuses.is_empty()).await;
    assert_eq!(view.statuses, ["done"]);
    let finished = [
        json!({ "kind": "user", "text": GREET, "images": 0 }),
        json!({ "kind": "text", "text": GREETING, "streaming": false }),
        json!({ "kind": "user", "text": COUNT, "images": 0 }),
        json!({ "kind": "text", "text": COUNTED, "streaming": false }),
    ];
    assert_eq!(view.items, finished);
    let record = fs::read(server.root.path().join(format!("data/chats/{chat_id}.json"))).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&record).unwrap()["status"], "done");

    server.restart().await;
    let (mut socket, status, attention, mut view) = reconnect(server.port, &chat_id).await;
    assert_eq!(status, "done");
    assert!(attention, "the restored Chat is done and unseen but not in the attention feed");
    view.follow(&mut socket, &chat_id, |view| view.items.len() >= finished.len()).await;
    assert_eq!(view.items, finished);
}

async fn reconnect(port: u16, chat_id: &str) -> (Socket, Value, bool, View) {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    let hello = next_control(&mut socket, "hello").await;
    let chat = hello["chats"].as_array().unwrap().iter().find(|chat| chat["id"] == chat_id).cloned();
    let status = chat.expect("the chat was not restored")["status"].clone();
    let attention = next_control(&mut socket, "attention.changed").await["items"].clone();
    let attention = attention.as_array().unwrap().iter().any(|item| item["chatId"] == chat_id);
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    let items = next_control(&mut socket, "chat.items").await["items"].as_array().unwrap().clone();
    (socket, status, attention, View { items, statuses: Vec::new() })
}

impl View {
    async fn follow(&mut self, socket: &mut Socket, chat_id: &str, until: impl Fn(&Self) -> bool) {
        timeout(Duration::from_secs(20), async {
            while !until(self) {
                let message = next(socket).await;
                if message["chatId"] != chat_id {
                    continue;
                }
                match message["type"].as_str().unwrap() {
                    "chat.status" => self.statuses.push(message["status"].clone()),
                    "chat.item" => {
                        let index = message["index"].as_u64().unwrap() as usize;
                        assert!(index <= self.items.len(), "{message}");
                        match self.items.get_mut(index) {
                            Some(item) => *item = message["item"].clone(),
                            None => self.items.push(message["item"].clone()),
                        }
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out with {:?} {:?}", self.statuses, self.items));
    }
}

async fn gated(gates: &Path, name: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !gates.join(name).exists() {
        assert!(Instant::now() < deadline, "the fake never reached gate {name}");
        sleep(Duration::from_millis(20)).await;
    }
}

async fn next(socket: &mut Socket) -> Value {
    loop {
        if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
            return serde_json::from_str(&text).unwrap();
        }
    }
}
