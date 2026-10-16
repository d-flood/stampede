mod common;

use std::{env, fs, time::Duration};

use common::{Server, close_chat, control, next, next_control};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const COMPOUND: &str =
    "Run the shell command `mkdir -p out && touch out/approved.txt` exactly as written, then reply with the single word Done.";
const INSTEAD: &str = "Do not touch any files. Reply with the single word Skipped.";
const THREAD: &str = "01a14694-b087-7cd3-bfa6-0cbe95e3d067";
const TURN: &str = "01a14694-bb62-7b03-a652-d57d23c910c3";

#[tokio::test]
async fn the_first_answer_to_a_claude_request_wins() {
    first_answer_wins("claude-code", json!({ "behavior": "allow" })).await;
}

#[tokio::test]
async fn the_first_answer_to_a_codex_request_wins() {
    first_answer_wins("codex", json!({ "decision": "accept" })).await;
}

#[tokio::test]
async fn a_claude_deny_interrupts_and_a_deny_with_an_instruction_continues() {
    let (launched, mut device) = denied(
        "claude-code",
        "Use the Write tool to create hello.txt containing the single line hi, then reply with the single word Done.",
        "Run the shell command `touch approved.txt` with the Bash tool, then reply with the single word Done.",
    )
    .await;
    close_chat(&mut device.socket, launched.server.root.path(), &device.chat_id).await;
    let denied = json!({ "behavior": "deny", "message": "The user denied this request.", "interrupt": true });
    assert_eq!(launched.answers(), [denied, json!({ "behavior": "deny", "message": INSTEAD })]);
}

#[tokio::test]
async fn a_codex_deny_cancels_and_a_deny_with_an_instruction_declines_then_steers() {
    let (launched, mut device) = denied(
        "codex",
        "Use apply_patch to create a file named hello.txt containing the single line hi. Then reply with the single word Done.",
        "Run the shell command `touch approved.txt`, then reply with the single word Done.",
    )
    .await;
    assert!(device.items.contains(&json!({ "kind": "user", "text": INSTEAD, "images": 0 })));
    close_chat(&mut device.socket, launched.server.root.path(), &device.chat_id).await;
    assert_eq!(launched.answers(), [json!({ "decision": "cancel" }), json!({ "decision": "decline" })]);
    let received = launched.received();
    let declined = received.iter().position(|message| message["result"]["decision"] == "decline").unwrap();
    let input = json!([{ "type": "text", "text": INSTEAD, "text_elements": [] }]);
    let steer = json!({ "threadId": THREAD, "input": input, "expectedTurnId": TURN });
    assert_eq!(received[declined + 1]["method"], "turn/steer");
    assert_eq!(received[declined + 1]["params"], steer);
}

#[tokio::test]
async fn a_pending_claude_request_survives_a_server_restart() {
    restart("claude-code", "approval", json!({ "behavior": "allow" })).await;
}

#[tokio::test]
async fn a_pending_codex_request_is_replayed_after_a_server_restart() {
    let launched = restart("codex", "approval-restart", json!({ "decision": "accept" })).await;
    let methods: Vec<Value> = launched.received().iter().map(|message| message["method"].clone()).collect();
    let expected = json!([
        "initialize",
        "initialized",
        "thread/start",
        "turn/start",
        "initialize",
        "initialized",
        "thread/resume",
        null,
        "thread/unsubscribe",
    ]);
    assert_eq!(json!(methods), expected);
}

#[tokio::test]
async fn a_codex_approval_replayed_before_the_resume_response_is_kept() {
    let recorded = include_str!("fixtures/chats/codex/approval-restart.recorded.jsonl");
    let mut entries: Vec<Value> = recorded.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let resume = entries.iter().position(|entry| entry["stdin"]["method"] == "thread/resume").unwrap();
    let approval = entries.iter().enumerate().skip(resume).find_map(|(index, entry)| {
        (entry["stdout"]["method"] == "item/commandExecution/requestApproval").then_some(index)
    }).unwrap();
    let request = entries.remove(approval);
    entries.insert(resume + 1, request);
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("before-resume.recorded.jsonl");
    let fixture = entries.into_iter().map(|entry| entry.to_string() + "\n").collect::<String>();
    fs::write(&path, fixture).unwrap();
    restart("codex", path.to_str().unwrap(), json!({ "decision": "accept" })).await;
}

#[tokio::test]
async fn a_codex_subagents_request_is_answered_from_the_parent_chat() {
    let mut entries: Vec<Value> = include_str!("fixtures/chats/codex/approval.recorded.jsonl")
        .lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let mut child: Value = include_str!("fixtures/chats/codex/subagents.recorded.jsonl")
        .lines().map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|entry| entry["stdout"]["method"] == "item/completed" && entry["stdout"]["params"]["item"]["type"] == "subAgentActivity")
        .unwrap();
    let child_id = child["stdout"]["params"]["item"]["agentThreadId"].clone();
    let index = entries.iter().position(|entry| entry["stdout"]["method"] == "item/commandExecution/requestApproval").unwrap();
    child["stdout"]["params"]["threadId"] = entries[index]["stdout"]["params"]["threadId"].clone();
    entries[index]["stdout"]["params"]["threadId"] = child_id.clone();
    entries.insert(index, child);
    for entry in &mut entries {
        if entry["stdout"]["method"] == "serverRequest/resolved" {
            entry["stdout"]["params"]["threadId"] = child_id.clone();
        }
    }
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("child-approval.recorded.jsonl");
    fs::write(&path, entries.into_iter().map(|entry| entry.to_string() + "\n").collect::<String>()).unwrap();
    let launched = Launched::new("codex", path.to_str().unwrap()).await;
    let mut device = Device::open(launched.server.port, "parent", &launched.chat_id).await;
    device.send(COMPOUND).await;
    device.follow(|device| device.status == "waiting" && !device.requests.is_empty()).await;
    assert_eq!(device.items.iter().find(|item| item["kind"] == "subagent").unwrap()["status"], "waiting");
    let request = device.requests[0].clone();
    assert_eq!(request["kind"], "command");
    device.answer(&request, json!({ "allow": true })).await;
    device.follow(|device| device.status == "done").await;
    assert_eq!(device.statuses, ["working", "waiting", "working", "done"]);
    assert_eq!(device.items.iter().find(|item| item["kind"] == "subagent").unwrap()["status"], "working");
    assert_eq!(device.resolved, [request["id"].clone()]);
    close_chat(&mut device.socket, launched.server.root.path(), &launched.chat_id).await;
    assert_eq!(launched.answers(), [json!({ "decision": "accept" })]);
}

async fn first_answer_wins(agent: &str, allowed: Value) {
    let launched = Launched::new(agent, "approval").await;
    let chat_id = &launched.chat_id;
    let mut first = Device::open(launched.server.port, "first", chat_id).await;
    let mut second = Device::open(launched.server.port, "second", chat_id).await;

    first.send(COMPOUND).await;
    first
        .follow(|device| device.status == "waiting" && device.attention == "waiting" && !device.requests.is_empty())
        .await;
    second.follow(|device| !device.requests.is_empty()).await;
    assert_eq!(first.statuses, ["working", "waiting"]);
    let request = first.requests[0].clone();
    assert_eq!(request["kind"], "command");
    assert_eq!(request["words"], json!([["mkdir", "-p", "out"], ["touch", "out/approved.txt"]]));
    assert_eq!(second.requests, [request.clone()]);

    first.answer(&request, json!({ "allow": true })).await;
    first.follow(|device| !device.resolved.is_empty()).await;
    second.follow(|device| !device.resolved.is_empty()).await;
    second.answer(&request, json!({ "allow": false, "instead": "Too late" })).await;
    first.follow(|device| device.status == "done").await;
    second.follow(|device| device.status == "done").await;
    second.sync().await;

    assert_eq!(first.statuses, ["working", "waiting", "working", "done"]);
    for device in [&first, &second] {
        assert_eq!(device.resolved, [request["id"].clone()]);
        assert!(device.requests.is_empty());
    }
    assert_eq!(first.items.last().unwrap(), &json!({ "kind": "text", "text": "Done", "streaming": false }));
    close_chat(&mut first.socket, launched.server.root.path(), chat_id).await;
    assert_eq!(launched.answers(), [allowed]);
}

async fn denied(agent: &str, edit: &str, command: &str) -> (Launched, Device) {
    let launched = Launched::new(agent, "deny").await;
    let mut device = Device::open(launched.server.port, "device", &launched.chat_id).await;

    device.send(edit).await;
    device.follow(|device| !device.requests.is_empty()).await;
    let request = device.requests[0].clone();
    let path = launched.server.root.path().join("home/hello.txt");
    let expected = json!({ "kind": "edit", "id": request["id"], "path": path, "diff": "+hi\n" });
    assert_eq!(request, expected);
    device.answer(&request, json!({ "allow": false })).await;
    device.follow(|device| device.status == "done").await;
    assert_eq!(device.statuses, ["working", "waiting", "working", "done"]);

    device.statuses.clear();
    device.send(command).await;
    device.follow(|device| !device.requests.is_empty()).await;
    let request = device.requests[0].clone();
    assert_eq!(request["words"], json!([["touch", "approved.txt"]]));
    device.answer(&request, json!({ "allow": false, "instead": INSTEAD })).await;
    device.follow(|device| device.status == "done").await;
    assert_eq!(device.statuses, ["working", "waiting", "working", "done"]);
    assert_eq!(device.items.last().unwrap(), &json!({ "kind": "text", "text": "Skipped", "streaming": false }));
    (launched, device)
}

async fn restart(agent: &str, fixture: &str, allowed: Value) -> Launched {
    let mut launched = Launched::new(agent, fixture).await;
    let (server, chat_id) = (&mut launched.server, launched.chat_id.clone());
    let mut device = Device::open(server.port, "device", &chat_id).await;
    device.send(COMPOUND).await;
    device.follow(|device| device.status == "waiting" && !device.requests.is_empty()).await;
    let request = device.requests[0].clone();
    server.stop().await;
    server.restart().await;

    let mut device = Device::open(server.port, "device", &chat_id).await;
    assert_eq!(device.status, "waiting");
    assert_eq!(device.attention, "waiting");
    device.follow(|device| !device.requests.is_empty()).await;
    assert_eq!(device.requests, [request.clone()]);
    device.answer(&request, json!({ "allow": true })).await;
    device.follow(|device| device.status == "done").await;
    assert_eq!(device.statuses, ["working", "done"]);
    assert_eq!(device.resolved, [request["id"].clone()]);
    close_chat(&mut device.socket, server.root.path(), &chat_id).await;
    assert_eq!(launched.answers(), [allowed]);
    launched
}

struct Launched {
    agent: String,
    server: Server,
    chat_id: String,
    scratch: TempDir,
}

impl Launched {
    async fn new(agent: &str, fixture: &str) -> Self {
        let config = format!(
        r#"
[[rig]]
name = "rig"
agent = "{agent}"
command = "{agent}"
env = {{ OIKONOMOS_FAKE_FIXTURE = "{fixture}" }}
"#
    );
        let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
        let scratch = tempfile::tempdir().unwrap();
        let received = scratch.path().join("received.jsonl");
        let env = vec![("PATH", path.into()), ("OIKONOMOS_FAKE_RECEIVED", received.into())];
        let server = Server::with_env("chat-requests", &config, env).await;
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
        control(&mut socket, json!({ "type": "hello", "deviceId": "creator" })).await;
        next_control(&mut socket, "hello").await;
        control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
        let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
        control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "rig" })).await;
        let created = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.created")).await;
        let chat_id = created.expect("the chat was never created")["chat"]["id"].as_str().unwrap().to_owned();
        Self { agent: agent.to_owned(), server, chat_id, scratch }
    }

    fn received(&self) -> Vec<Value> {
        let log = match self.agent.as_str() {
            "codex" => self.server.root.path().join("home/.codex/fake-daemon/received.jsonl"),
            _ => self.scratch.path().join("received.jsonl"),
        };
        fs::read_to_string(log).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    fn answers(&self) -> Vec<Value> {
        let received = self.received().into_iter();
        match self.agent.as_str() {
            "codex" => received
                .filter(|message| message.get("method").is_none())
                .map(|message| message["result"].clone())
                .collect(),
            _ => received
                .filter(|message| message["type"] == "control_response")
                .map(|message| message["response"]["response"].clone())
                .collect(),
        }
    }
}

struct Device {
    socket: Socket,
    chat_id: String,
    status: Value,
    statuses: Vec<Value>,
    attention: Value,
    items: Vec<Value>,
    requests: Vec<Value>,
    resolved: Vec<Value>,
    listed: usize,
}

impl Device {
    async fn open(port: u16, name: &str, chat_id: &str) -> Self {
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
        control(&mut socket, json!({ "type": "hello", "deviceId": name })).await;
        let hello = next_control(&mut socket, "hello").await;
        let chat = hello["chats"].as_array().unwrap().iter().find(|chat| chat["id"] == chat_id).cloned();
        let status = chat.expect("the chat is not listed")["status"].clone();
        let mut device = Self {
            socket,
            chat_id: chat_id.to_owned(),
            status,
            statuses: Vec::new(),
            attention: Value::Null,
            items: Vec::new(),
            requests: Vec::new(),
            resolved: Vec::new(),
            listed: 0,
        };
        let attention = next_control(&mut device.socket, "attention.changed").await;
        device.attend(&attention);
        control(&mut device.socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
        device.items = next_control(&mut device.socket, "chat.items").await["items"].as_array().unwrap().clone();
        device
    }

    async fn send(&mut self, text: &str) {
        control(&mut self.socket, json!({ "type": "chat.send", "chatId": self.chat_id, "text": text })).await;
    }

    async fn answer(&mut self, request: &Value, answer: Value) {
        let message = json!({ "type": "chat.answer", "chatId": self.chat_id, "requestId": request["id"], "answer": answer });
        control(&mut self.socket, message).await;
    }

    async fn sync(&mut self) {
        let listed = self.listed;
        control(&mut self.socket, json!({ "type": "projects.list" })).await;
        self.follow(|device| device.listed > listed).await;
    }

    fn attend(&mut self, message: &Value) {
        let items = message["items"].as_array().unwrap();
        let item = items.iter().find(|item| item["chatId"] == *self.chat_id);
        self.attention = item.map_or(Value::Null, |item| item["status"].clone());
    }

    async fn follow(&mut self, until: impl Fn(&Self) -> bool) {
        timeout(Duration::from_secs(20), async {
            while !until(self) {
                let message = next(&mut self.socket).await;
                let ours = message["chatId"] == *self.chat_id;
                match message["type"].as_str().unwrap() {
                    "attention.changed" => self.attend(&message),
                    "projects.changed" => self.listed += 1,
                    "chat.status" if ours => {
                        self.status = message["status"].clone();
                        self.statuses.push(message["status"].clone());
                    }
                    "chat.item" if ours => {
                        let index = message["index"].as_u64().unwrap() as usize;
                        assert!(index <= self.items.len(), "{message}");
                        match self.items.get_mut(index) {
                            Some(item) => *item = message["item"].clone(),
                            None => self.items.push(message["item"].clone()),
                        }
                    }
                    "chat.request" if ours => {
                        self.requests.retain(|request| request["id"] != message["request"]["id"]);
                        self.requests.push(message["request"].clone());
                    }
                    "chat.resolved" if ours => {
                        self.requests.retain(|request| request["id"] != message["requestId"]);
                        self.resolved.push(message["requestId"].clone());
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!("timed out at {} {:?} with {:?} and {:?}", self.status, self.statuses, self.requests, self.items)
        });
    }
}
