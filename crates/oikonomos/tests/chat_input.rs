mod common;

use std::{env, fs, path::Path, time::Duration};

use common::{Server, close_chat, control, next, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const CLAUDE: [&str; 3] = [
    "Count from 1 to 30, one number per line. Use no tools.",
    "Then also say banana.",
    "Count from 1 to 300, one number per line. Use no tools.",
];
const CODEX: [&str; 3] = [
    "Count from 1 to 60, one number per line. Use no tools.",
    "Stop counting and reply with only the word \"banana\".",
    "Count from 1 to 300, one number per line. Use no tools.",
];

#[derive(Default)]
struct View {
    items: Vec<Value>,
    statuses: Vec<Value>,
    queued: Vec<Value>,
}

#[tokio::test]
async fn a_claude_chat_steers_mid_turn_queues_for_the_next_and_interrupts() {
    let (server, mut socket, chat_id) = launch("claude-code").await;
    let [first, steer, queued] = CLAUDE;
    let mut view = View::default();

    send(&mut socket, &chat_id, first, false).await;
    view.follow(&mut socket, &chat_id, |view| streaming(view, 1)).await;
    send(&mut socket, &chat_id, queued, true).await;
    view.follow(&mut socket, &chat_id, |view| !view.queued.is_empty()).await;
    assert_eq!(view.queued, [json!([queued])]);
    assert_eq!(record(&server, &chat_id)["queued"], json!([queued]));

    send(&mut socket, &chat_id, steer, false).await;
    view.follow(&mut socket, &chat_id, |view| streaming(view, 5)).await;
    assert_eq!(view.queued, [json!([queued]), json!([])]);
    assert_eq!(record(&server, &chat_id)["queued"], json!([]));
    send(&mut socket, &chat_id, "Never sent", true).await;
    interrupt(&mut socket, &chat_id).await;
    view.follow(&mut socket, &chat_id, |view| view.statuses.last().is_some_and(|status| status == "done")).await;

    assert_eq!(view.statuses, ["working", "done"]);
    assert_eq!(view.queued, [json!([queued]), json!([]), json!(["Never sent"]), json!([])]);
    assert_eq!(view.items, claude_items());
    close_chat(&mut socket, server.root.path(), &chat_id).await;
}

#[tokio::test]
async fn a_codex_chat_steers_mid_turn_queues_for_the_next_and_interrupts() {
    let (server, mut socket, chat_id) = launch("codex").await;
    let [first, steer, queued] = CODEX;
    let mut view = View::default();

    send(&mut socket, &chat_id, first, false).await;
    view.follow(&mut socket, &chat_id, |view| streaming(view, 1)).await;
    send(&mut socket, &chat_id, queued, true).await;
    view.follow(&mut socket, &chat_id, |view| !view.queued.is_empty()).await;
    assert_eq!(view.queued, [json!([queued])]);

    send(&mut socket, &chat_id, steer, false).await;
    view.follow(&mut socket, &chat_id, |view| streaming(view, 5)).await;
    assert_eq!(view.queued, [json!([queued]), json!([])]);
    interrupt(&mut socket, &chat_id).await;
    view.follow(&mut socket, &chat_id, |view| view.statuses.last().is_some_and(|status| status == "done")).await;

    assert_eq!(view.statuses, ["working", "done"]);
    let counted = json!({ "kind": "text", "text": "1\n2\n3\n", "streaming": false });
    assert_eq!(
        view.items,
        [
            json!({ "kind": "user", "text": first, "images": 0 }),
            counted.clone(),
            json!({ "kind": "user", "text": steer, "images": 0 }),
            json!({ "kind": "text", "text": "banana", "streaming": false }),
            json!({ "kind": "user", "text": queued, "images": 0 }),
            counted,
            json!({ "kind": "notice", "notice": "interrupted", "text": "Interrupted" }),
        ]
    );

    close_chat(&mut socket, server.root.path(), &chat_id).await;
    let received = received(server.root.path());
    let methods: Vec<&Value> = received.iter().map(|message| &message["method"]).collect();
    assert_eq!(
        methods,
        [
            "initialize",
            "initialized",
            "thread/start",
            "model/list",
            "skills/list",
            "turn/start",
            "turn/steer",
            "turn/start",
            "turn/interrupt",
            "thread/unsubscribe"
        ]
    );
    let thread = "01a14671-e0ff-7190-bdcf-13fce013710f";
    let input = |text: &str| json!([{ "type": "text", "text": text, "text_elements": [] }]);
    assert_eq!(
        received[6]["params"],
        json!({ "threadId": thread, "input": input(steer), "expectedTurnId": "01a14671-e17e-7fe0-9319-de9ae90d79be" })
    );
    assert_eq!(received[7]["params"], json!({ "threadId": thread, "input": input(queued) }));
    assert_eq!(
        received[8]["params"],
        json!({ "threadId": thread, "turnId": "01a14671-f13a-7471-a47e-3a969b05eea6" })
    );
}

#[tokio::test]
async fn a_queued_message_survives_a_server_restart() {
    let (mut server, mut socket, chat_id) = launch("claude-code").await;
    let [first, steer, queued] = CLAUDE;
    let mut view = View::default();
    send(&mut socket, &chat_id, first, false).await;
    view.follow(&mut socket, &chat_id, |view| streaming(view, 1)).await;
    send(&mut socket, &chat_id, queued, true).await;
    view.follow(&mut socket, &chat_id, |view| !view.queued.is_empty()).await;

    server.restart().await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    let hello = next_control(&mut socket, "hello").await;
    let chat = hello["chats"].as_array().unwrap().iter().find(|chat| chat["id"] == chat_id).cloned();
    let chat = chat.expect("the chat was not restored");
    assert_eq!(chat["status"], "working");
    assert_eq!(chat["queued"], json!([queued]));
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    let items = next_control(&mut socket, "chat.items").await["items"].as_array().unwrap().clone();
    let mut view = View { items, ..View::default() };

    send(&mut socket, &chat_id, steer, false).await;
    view.follow(&mut socket, &chat_id, |view| streaming(view, 5)).await;
    assert_eq!(view.queued, [json!([])]);
    interrupt(&mut socket, &chat_id).await;
    view.follow(&mut socket, &chat_id, |view| view.statuses.last().is_some_and(|status| status == "done")).await;
    assert_eq!(view.statuses, ["done"]);
    assert_eq!(view.items, claude_items());
    assert_eq!(record(&server, &chat_id)["queued"], json!([]));
}

fn claude_items() -> [Value; 7] {
    let [first, steer, queued] = CLAUDE;
    let counted: Vec<String> = (1..=30).map(|number| number.to_string()).collect();
    let interrupted: Vec<String> = (1..=16).map(|number| number.to_string()).collect();
    [
        json!({ "kind": "user", "text": first, "images": 0 }),
        json!({ "kind": "text", "text": counted.join("\n"), "streaming": false }),
        json!({ "kind": "user", "text": steer, "images": 0 }),
        json!({ "kind": "text", "text": "banana 🍌", "streaming": false }),
        json!({ "kind": "user", "text": queued, "images": 0 }),
        json!({ "kind": "text", "text": interrupted.join("\n"), "streaming": false }),
        json!({ "kind": "notice", "notice": "interrupted", "text": "Interrupted" }),
    ]
}

/// Whether the item at `index` is text still streaming, which the fixtures pause in until the test acts.
fn streaming(view: &View, index: usize) -> bool {
    view.items.get(index).is_some_and(|item| item["streaming"] == true)
}

async fn launch(agent: &str) -> (Server, Socket, String) {
    let config = format!(
        r#"
[[rig]]
name = "agent"
agent = "{agent}"
command = "agent"
env = {{ OIKONOMOS_FAKE_FIXTURE = "steer-queue-interrupt" }}
"#
    );
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("chat-input", &config, vec![("PATH", path.into())]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "agent" })).await;
    let created = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.created"));
    let chat_id = created.await.expect("the chat was never created")["chat"]["id"].as_str().unwrap().to_owned();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    assert_eq!(next_control(&mut socket, "chat.items").await["items"], json!([]));
    (server, socket, chat_id)
}

async fn send(socket: &mut Socket, chat_id: &str, text: &str, queue: bool) {
    control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": text, "queue": queue })).await;
}

async fn interrupt(socket: &mut Socket, chat_id: &str) {
    control(socket, json!({ "type": "chat.interrupt", "chatId": chat_id })).await;
}

fn record(server: &Server, chat_id: &str) -> Value {
    serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{chat_id}.json"))).unwrap()).unwrap()
}

/// What the fake Codex daemon was sent, in order.
fn received(root: &Path) -> Vec<Value> {
    let log = fs::read_to_string(root.join("home/.codex/fake-daemon/received.jsonl")).unwrap();
    log.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
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
                    "chat.queued" => self.queued.push(message["queued"].clone()),
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
        .unwrap_or_else(|_| panic!("timed out with {:?} {:?} {:?}", self.statuses, self.queued, self.items));
    }
}
