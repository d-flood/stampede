mod common;

use std::{env, ffi::OsString, fs, path::Path, time::Duration};

use common::{Server, control, next, next_control};
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn a_claude_chat_reports_its_context_and_tokens() {
    let (server, mut socket, chat) = launch("claude-code", "simple-turn", Vec::new()).await;
    let chat_id = chat["id"].as_str().unwrap();
    assert_eq!(meters(&chat), json!({ "context": null, "tokens": null }));

    let expected = json!({
        "context": { "used": 21640, "window": 1000000 },
        "tokens": { "input": 22834, "output": 32 },
    });
    assert_eq!(turn(&mut socket, chat_id, &chat).await, expected);
    assert_eq!(meters(&record(&server, chat_id)), expected);
    assert_eq!(reconnect(server.port, chat_id).await.1, expected);
}

#[tokio::test]
async fn a_codex_chat_reports_its_context_and_tokens() {
    let (server, mut socket, chat) = launch("codex", "simple-turn", Vec::new()).await;
    let chat_id = chat["id"].as_str().unwrap();
    assert_eq!(meters(&chat), json!({ "context": null, "tokens": null }));

    let expected = json!({
        "context": { "used": 22058, "window": 475000 },
        "tokens": { "input": 22044, "output": 14 },
    });
    assert_eq!(turn(&mut socket, chat_id, &chat).await, expected);
    assert_eq!(meters(&record(&server, chat_id)), expected);
    assert_eq!(reconnect(server.port, chat_id).await.1, expected);
}

#[tokio::test]
async fn a_claude_chat_keeps_its_meters_through_a_restart_mid_turn() {
    let scratch = tempfile::tempdir().unwrap();
    let gates = scratch.path().join("gates");
    fs::create_dir(&gates).unwrap();
    let fixture = scratch.path().join("call-gated.recorded.jsonl");
    fs::write(&fixture, gated_before_second_call()).unwrap();
    let env = vec![("OIKONOMOS_FAKE_GATES", gates.clone().into())];
    let (mut server, mut socket, chat) = launch("claude-code", fixture.to_str().unwrap(), env).await;
    let chat_id = chat["id"].as_str().unwrap();

    let greeted = json!({
        "context": { "used": 21240, "window": 1000000 },
        "tokens": { "input": 22415, "output": 25 },
    });
    assert_eq!(turn(&mut socket, chat_id, &chat).await, greeted);
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Count" })).await;
    reached(&gates, "call").await;
    server.stop().await;
    fs::remove_file(gates.join("call")).unwrap();
    reached(&gates, "restart").await;
    server.restart().await;

    let (mut socket, restored) = reconnect(server.port, chat_id).await;
    let counting = json!({
        "context": { "used": 23322, "window": 1000000 },
        "tokens": { "input": 22415, "output": 25 },
    });
    assert_eq!(follow(&mut socket, chat_id, restored, |meters, _| *meters == counting).await, counting);

    fs::remove_file(gates.join("restart")).unwrap();
    let counted = json!({
        "context": { "used": 23322, "window": 1000000 },
        "tokens": { "input": 45737, "output": 260 },
    });
    assert_eq!(follow(&mut socket, chat_id, counting, |_, status| status == "done").await, counted);
}

#[tokio::test]
async fn a_codex_chat_keeps_its_meters_through_restarts() {
    let gates = tempfile::tempdir().unwrap();
    let env = vec![("OIKONOMOS_FAKE_GATES", gates.path().into())];
    let (mut server, mut socket, chat) = launch("codex", "restart-mid-turn", env).await;
    let chat_id = chat["id"].as_str().unwrap();
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Count" })).await;
    reached(gates.path(), "stop").await;
    server.stop().await;
    fs::remove_file(gates.path().join("stop")).unwrap();
    server.restart().await;

    let (mut socket, restored) = reconnect(server.port, chat_id).await;
    assert_eq!(restored, json!({ "context": null, "tokens": null }));
    reached(gates.path(), "resumed").await;
    fs::remove_file(gates.path().join("resumed")).unwrap();
    let counted = json!({
        "context": { "used": 22150, "window": 475000 },
        "tokens": { "input": 22027, "output": 123 },
    });
    assert_eq!(follow(&mut socket, chat_id, restored, |_, status| status == "done").await, counted);

    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Say Done" })).await;
    reached(gates.path(), "stop again").await;
    server.stop().await;
    fs::remove_file(gates.path().join("stop again")).unwrap();
    server.restart().await;
    assert_eq!(reconnect(server.port, chat_id).await.1, counted);
}

/// The recorded `long-turn`, gated before its second turn's API call and after its 20th delta instead of its 8th.
fn gated_before_second_call() -> String {
    let recorded = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/chats/claude-code/long-turn.recorded.jsonl");
    let mut turns = 0;
    let mut fixture = String::new();
    for line in fs::read_to_string(recorded).unwrap().lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        turns += usize::from(entry["stdin"]["type"] == "user");
        if turns == 2 && entry["stdout"]["event"]["type"] == "message_start" {
            fixture.push_str("{\"gate\": \"call\"}\n");
        }
        if entry["gate"] != "stop" {
            fixture.push_str(line);
            fixture.push('\n');
        }
    }
    fixture
}

async fn launch(agent: &str, fixture: &str, env: Vec<(&'static str, OsString)>) -> (Server, Socket, Value) {
    let config = format!(
        r#"
[[rig]]
name = "meters"
agent = "{agent}"
command = "{agent}"
env = {{ OIKONOMOS_FAKE_FIXTURE = "{fixture}" }}
"#
    );
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let env = [vec![("PATH", path.into())], env].concat();
    let server = Server::with_env("chat-meters", &config, env).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "meters" })).await;
    let created = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.created"));
    let chat = created.await.expect("the chat was never created")["chat"].clone();
    (server, socket, chat)
}

/// Sends a message and follows the Chat's meters until its turn is done.
async fn turn(socket: &mut Socket, chat_id: &str, chat: &Value) -> Value {
    control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Greet the reader" })).await;
    let mut working = false;
    follow(socket, chat_id, meters(chat), |_, status| {
        working |= status == "working";
        working && status == "done"
    })
    .await
}

/// Applies `chat.meters` to `meters` until `until` holds of them and the latest status.
async fn follow(
    socket: &mut Socket,
    chat_id: &str,
    mut meters: Value,
    mut until: impl FnMut(&Value, &str) -> bool,
) -> Value {
    let mut status = String::new();
    timeout(Duration::from_secs(20), async {
        while !until(&meters, &status) {
            let message = next(socket).await;
            if message["chatId"] != chat_id {
                continue;
            }
            match message["type"].as_str().unwrap() {
                "chat.meters" => meters = json!({ "context": message["context"], "tokens": message["tokens"] }),
                "chat.status" => status = message["status"].as_str().unwrap().to_owned(),
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out at {status:?} with {meters}"));
    meters
}

fn meters(chat: &Value) -> Value {
    json!({ "context": chat["context"], "tokens": chat["tokens"] })
}

fn record(server: &Server, chat_id: &str) -> Value {
    let path = server.root.path().join(format!("data/chats/{chat_id}.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// Connects a Device and returns the Chat's meters as `hello` lists them.
async fn reconnect(port: u16, chat_id: &str) -> (Socket, Value) {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "another" })).await;
    let hello = next_control(&mut socket, "hello").await;
    let chat = hello["chats"].as_array().unwrap().iter().find(|chat| chat["id"] == chat_id);
    let meters = meters(chat.expect("the chat is not listed"));
    (socket, meters)
}

async fn reached(gates: &Path, name: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !gates.join(name).exists() {
        assert!(Instant::now() < deadline, "the fake never reached gate {name}");
        sleep(Duration::from_millis(20)).await;
    }
}
