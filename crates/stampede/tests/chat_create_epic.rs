use crate::common::{Server, close_chat, control, next, next_control};

use std::{env, fs, time::Duration};

use serde_json::{Value, json};
use tokio::{net::TcpStream, time::{sleep, timeout}};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn claude_creates_an_epic_in_two_turns_before_sending_queued_messages() {
    creates_epic("claude-code").await;
}

#[tokio::test]
async fn codex_creates_an_epic_in_two_turns_before_sending_queued_messages() {
    creates_epic("codex").await;
}

async fn creates_epic(agent: &str) {
    let (_fixture, server, mut socket, chat) = launch(agent, None).await;
    let templates: Value = serde_json::from_str(&reqwest::get(format!("http://127.0.0.1:{}/api/templates", server.port)).await.unwrap().text().await.unwrap()).unwrap();
    let names: Vec<_> = templates.as_array().unwrap().iter().map(|template| template["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["grill", "create-epic"]);

    control(&mut socket, json!({ "type": "chat.send", "chatId": chat, "text": "/create-epic Use the existing conversation." })).await;
    gate(&server, "brief").await;
    let sent = prompts(&server, agent);
    assert_eq!(sent.len(), 1);
    assert!(sent[0].ends_with("\n\nUse the existing conversation."));
    assert!(sent[0].contains(&server.root.path().join("data").display().to_string()));
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat, "text": "After the Epic.", "queue": true })).await;
    next_control(&mut socket, "chat.queued").await;
    fs::remove_file(server.root.path().join("home/brief")).unwrap();
    gate(&server, "handoff").await;
    let sent = prompts(&server, agent);
    assert_eq!(sent.len(), 2);
    assert_ne!(sent[0], sent[1]);
    assert!(sent[1].contains(&server.root.path().join("data").display().to_string()));
    assert!(!sent[1].contains("Use the existing conversation."));
    assert_ne!(sent[1], "After the Epic.");
    fs::remove_file(server.root.path().join("home/handoff")).unwrap();
    done(&mut socket, &chat).await;
    assert_eq!(prompts(&server, agent).last().unwrap(), "After the Epic.");
    assert_eq!(prompts(&server, agent).len(), 3);
    close_chat(&mut socket, server.root.path(), &chat).await;
}

#[tokio::test]
async fn a_failed_brief_does_not_start_tickets() {
    failed_brief("error").await;
}

#[tokio::test]
async fn a_usage_limit_does_not_start_tickets() {
    failed_brief("rate_limit").await;
}

#[tokio::test]
async fn stopping_the_brief_cancels_ticket_creation() {
    failed_brief("interrupt").await;
}

#[tokio::test]
async fn a_restart_preserves_the_pending_ticket_turn_without_repeating_it() {
    let (_fixture, mut server, mut socket, chat) = launch("claude-code", None).await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat, "text": "/create-epic" })).await;
    gate(&server, "brief").await;
    server.restart().await;
    drop(reconnect(&server, &chat).await);
    fs::remove_file(server.root.path().join("home/brief")).unwrap();
    gate(&server, "handoff").await;
    assert_eq!(prompts(&server, "claude-code").len(), 2);
    socket = reconnect(&server, &chat).await;
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat })).await;
    let items = next_control(&mut socket, "chat.items").await["items"].as_array().unwrap().clone();
    assert_eq!(items.iter().filter(|item| item["kind"] == "user").count(), 1);
    assert!(items.iter().any(|item| item["kind"] == "notice" && item["notice"] == "tickets"));
    server.restart().await;
    socket = reconnect(&server, &chat).await;
    fs::remove_file(server.root.path().join("home/handoff")).unwrap();
    done(&mut socket, &chat).await;
    assert_eq!(prompts(&server, "claude-code").len(), 2);
    close_chat(&mut socket, server.root.path(), &chat).await;
}

#[tokio::test]
async fn a_queued_create_epic_command_runs_both_turns() {
    let (_fixture, server, mut socket, chat) = launch("claude-code", None).await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat, "text": "Discuss the work." })).await;
    gate(&server, "brief").await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat, "text": "/create-epic Keep this requirement.", "queue": true })).await;
    next_control(&mut socket, "chat.queued").await;
    fs::remove_file(server.root.path().join("home/brief")).unwrap();
    gate(&server, "handoff").await;
    let sent = prompts(&server, "claude-code");
    assert_eq!(sent.len(), 2);
    assert!(sent[1].ends_with("\n\nKeep this requirement."));
    fs::remove_file(server.root.path().join("home/handoff")).unwrap();
    done(&mut socket, &chat).await;
    assert_eq!(prompts(&server, "claude-code").len(), 3);
    close_chat(&mut socket, server.root.path(), &chat).await;
}

async fn reconnect(server: &Server, chat: &str) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat })).await;
    next_control(&mut socket, "chat.items").await;
    socket
}

async fn failed_brief(failure: &str) {
    let (_fixture, server, mut socket, chat) = launch("claude-code", Some(failure)).await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat, "text": "/create-epic" })).await;
    gate(&server, "brief").await;
    if failure == "interrupt" {
        control(&mut socket, json!({ "type": "chat.interrupt", "chatId": chat })).await;
    }
    fs::remove_file(server.root.path().join("home/brief")).unwrap();
    done(&mut socket, &chat).await;
    assert_eq!(prompts(&server, "claude-code").len(), 1);
    close_chat(&mut socket, server.root.path(), &chat).await;
}

async fn launch(agent: &str, failure: Option<&str>) -> (tempfile::TempDir, Server, Socket, String) {
    let source = format!("{}/tests/fixtures/chats/{agent}/handoff.synthetic.jsonl", env!("CARGO_MANIFEST_DIR"));
    let mut entries: Vec<Value> = fs::read_to_string(source).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    if let Some(failure) = failure {
        if failure == "interrupt" {
            let result = entries.iter().position(|entry| entry["stdout"]["type"] == "result").unwrap();
            entries.splice(result..result, [
                json!({ "stdin": { "type": "control_request", "request_id": "stop", "request": { "subtype": "interrupt", "cancel_queued": true } } }),
                json!({ "stdout": { "type": "control_response", "response": { "subtype": "success", "request_id": "stop", "response": {} } } }),
            ]);
        } else if failure == "error" {
            let result = entries.iter_mut().find(|entry| entry["stdout"]["type"] == "result").unwrap();
            result["stdout"]["is_error"] = json!(true);
        } else {
            let answer = entries.iter_mut().find(|entry| entry["stdout"]["type"] == "assistant").unwrap();
            answer["stdout"]["error"] = json!(failure);
        }
    }
    let first = entries.iter().position(|entry| entry["stdin"]["type"] == "user" || entry["stdin"]["method"] == "turn/start").unwrap();
    entries.insert(first + 1, json!({ "gate": "brief" }));
    let third = if agent == "codex" {
        let start = entries.iter().position(|entry| entry["stdin"]["method"] == "turn/start").unwrap();
        let end = entries.iter().position(|entry| entry["stdout"]["method"] == "turn/completed").unwrap();
        let mut turn = entries[start..=end].to_vec();
        turn.retain(|entry| entry.get("gate").is_none());
        let text = serde_json::to_string(&turn).unwrap().replace("handoff-turn-0", "handoff-turn-2").replace("user-0", "user-2").replace("answer-0", "answer-2").replace("comment-0", "comment-2");
        serde_json::from_str::<Vec<Value>>(&text).unwrap()
    } else {
        entries[first..=entries.iter().position(|entry| entry["stdout"]["type"] == "result").unwrap()].iter().filter(|entry| entry.get("gate").is_none()).cloned().collect()
    };
    let end = entries.iter().position(|entry| entry["stdin"]["method"] == "thread/unsubscribe").unwrap_or(entries.len());
    entries.splice(end..end, third);
    let directory = tempfile::tempdir().unwrap();
    let fixture = directory.path().join("create-epic.jsonl");
    fs::write(&fixture, entries.iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
    let config = format!(r#"
[[rig]]
name = "agent"
agent = "{agent}"
env = {{ STAMPEDE_FAKE_FIXTURE = "{}", STAMPEDE_FAKE_GATES = ".", STAMPEDE_FAKE_RECEIVED = "received.jsonl" }}
"#, fixture.display());
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("chat-create-epic", &config, vec![("PATH", path.into())]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project, "rig": "agent" })).await;
    let chat = next_control(&mut socket, "chat.created").await["chat"]["id"].as_str().unwrap().to_owned();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat })).await;
    next_control(&mut socket, "chat.items").await;
    (directory, server, socket, chat)
}

fn prompts(server: &Server, agent: &str) -> Vec<String> {
    let log = if agent == "codex" { "home/.codex/fake-daemon/received.jsonl" } else { "home/received.jsonl" };
    fs::read_to_string(server.root.path().join(log)).unwrap_or_default().lines()
        .filter_map(|line| {
            let message: Value = serde_json::from_str(line).unwrap();
            if agent == "codex" { message["params"]["input"][0]["text"].as_str().map(str::to_owned) }
            else { message["message"]["content"].as_str().map(str::to_owned) }
        }).collect()
}

async fn gate(server: &Server, name: &str) {
    timeout(Duration::from_secs(10), async {
        while !server.root.path().join("home").join(name).exists() { sleep(Duration::from_millis(20)).await; }
    }).await.unwrap_or_else(|_| panic!("the agent never reached {name}"));
}

async fn done(socket: &mut Socket, chat: &str) {
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next(socket).await;
            if message["type"] == "chat.status" && message["chatId"] == chat && message["status"] == "done" { break; }
        }
    }).await.unwrap();
}
