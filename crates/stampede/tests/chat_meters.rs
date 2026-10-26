use crate::common;

use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use common::{Server, chat_turn, control, leftovers, next, next_control};
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Windows = BTreeMap<(String, String), (f64, u64)>;

const USAGE: &str = r#"
[[rig]]
name = "claude"
agent = "claude-code"
command = "claude"
env = { STAMPEDE_FAKE_FIXTURE = "simple-turn" }

[[rig]]
name = "codex"
agent = "codex"
command = "codex"
env = { STAMPEDE_FAKE_FIXTURE = "simple-turn" }
"#;

#[tokio::test]
async fn codex_resets_refresh_all_devices_and_retries_do_not_spend_twice() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("resets.json");
    let mut usage = serde_json::from_str::<Value>(include_str!("fixtures/chats/codex/usage.recorded.jsonl").lines().nth(1).unwrap()).unwrap()["stdout"]["result"].clone();
    usage["rateLimitResetCredits"] = json!({ "availableCount": 2, "credits": [
        { "id": "later", "resetType": "codexRateLimits", "status": "available", "expiresAt": 1901000000 },
        { "id": "next", "resetType": "codexRateLimits", "status": "available", "expiresAt": 1900000000 }
    ] });
    usage.as_object_mut().unwrap().remove("rateLimitsByLimitId");
    fs::write(&path, json!({ "usage": usage }).to_string()).unwrap();
    let fake_path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("resets", USAGE, vec![("PATH", fake_path.into()), ("STAMPEDE_FAKE_RESETS", path.clone().into())]).await;
    let mut watcher = device(server.port).await;
    let initial = timeout(Duration::from_secs(10), async {
        loop {
            let snapshot = next_control(&mut watcher, "usage.changed").await;
            if snapshot["codexResets"]["availableCount"] == 2 { break snapshot; }
        }
    }).await.unwrap();
    assert_eq!(initial["codexResets"], json!({ "availableCount": 2, "expiresAt": 1900000000000u64, "creditId": "next" }));
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/api/usage/codex/reset", server.port);
    let request = json!({ "idempotencyKey": "same-attempt", "creditId": "next" });
    for outcome in ["reset", "alreadyRedeemed"] {
        let response = client.post(&url).header("Content-Type", "application/json").body(request.to_string()).send().await.unwrap();
        assert!(response.status().is_success(), "{}", response.text().await.unwrap());
        assert_eq!(serde_json::from_str::<Value>(&response.text().await.unwrap()).unwrap(), outcome);
        let updated = timeout(Duration::from_secs(10), next_control(&mut watcher, "usage.changed")).await.unwrap();
        assert_eq!(updated["codexResets"], json!({ "availableCount": 1, "expiresAt": 1901000000000u64, "creditId": "later" }));
        assert_eq!(parse(&updated)[&key("codex", "five-hour")].0, 0.0);
    }
    let mut late = device(server.port).await;
    let latest = next_control(&mut late, "usage.changed").await;
    assert_eq!(latest["codexResets"]["availableCount"], 1);
    for outcome in ["nothingToReset", "noCredit"] {
        let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        state["outcome"] = json!(outcome);
        fs::write(&path, state.to_string()).unwrap();
        let response = client.post(&url).header("Content-Type", "application/json").body(json!({ "idempotencyKey": outcome }).to_string()).send().await.unwrap();
        assert_eq!(serde_json::from_str::<Value>(&response.text().await.unwrap()).unwrap(), outcome);
    }
    assert_eq!(client.post(&url).header("Content-Type", "application/json").body(json!({ "idempotencyKey": "" }).to_string()).send().await.unwrap().status(), 400);
    let state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(state["requests"].as_array().unwrap().len(), 4);
    assert_eq!(state["usage"]["rateLimitResetCredits"]["availableCount"], 1);
}

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
    let env = vec![("STAMPEDE_FAKE_GATES", gates.clone().into())];
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
    let env = vec![("STAMPEDE_FAKE_GATES", gates.path().into())];
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

#[tokio::test]
async fn devices_receive_the_four_usage_windows_and_an_update_replaces_a_reading() {
    let started = now();
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("meters", USAGE, vec![("PATH", path.into())]).await;
    let mut watcher = device(server.port).await;
    let mut socket = device(server.port).await;

    let polled = windows(&mut watcher, |windows| windows.len() == 4).await;
    assert_eq!(
        polled,
        table([
            ("claude-code", "five-hour", 0.14, 1792200599951),
            ("claude-code", "weekly", 0.7, 1792213199951),
            ("codex", "five-hour", 0.91, 1792195200000),
            ("codex", "weekly", 0.24, 1792664113000),
        ])
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while !leftovers(server.root.path()).is_empty() {
        assert!(Instant::now() < deadline, "left running: {:?}", leftovers(server.root.path()));
        sleep(Duration::from_millis(50)).await;
    }

    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    finish_a_turn(&mut socket, &project_id, "claude").await;
    let claude = windows(&mut watcher, |windows| windows[&key("claude-code", "five-hour")].0 != 0.14).await;
    assert_eq!(
        claude,
        table([
            ("claude-code", "five-hour", 0.11, 1792182600000),
            ("claude-code", "weekly", 0.63, 1792213200000),
            ("codex", "five-hour", 0.91, 1792195200000),
            ("codex", "weekly", 0.24, 1792664113000),
        ])
    );

    finish_a_turn(&mut socket, &project_id, "codex").await;
    let codex = windows(&mut watcher, |windows| windows[&key("codex", "weekly")].0 != 0.24).await;
    assert_eq!(
        codex,
        table([
            ("claude-code", "five-hour", 0.11, 1792182600000),
            ("claude-code", "weekly", 0.63, 1792213200000),
            ("codex", "five-hour", 0.91, 1792195200000),
            ("codex", "weekly", 0.26, 1792664113000),
        ])
    );

    let mut late = device(server.port).await;
    let latest = timeout(Duration::from_secs(5), next_control(&mut late, "usage.changed")).await.expect("no windows on hello");
    assert_eq!(parse(&latest), codex);
    let read: Vec<u64> = latest["readings"].as_array().unwrap().iter().map(|reading| reading["readAt"].as_u64().unwrap()).collect();
    assert!(read.iter().all(|&read| (started..=now()).contains(&read)), "{latest}");
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
env = {{ STAMPEDE_FAKE_FIXTURE = "{fixture}" }}
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

async fn device(port: u16) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    socket
}

async fn finish_a_turn(socket: &mut Socket, project_id: &Value, rig: &str) {
    control(socket, json!({ "type": "chat.create", "projectId": project_id, "rig": rig })).await;
    let chat = timeout(Duration::from_secs(10), next_control(socket, "chat.created")).await.unwrap()["chat"].clone();
    let (statuses, _) = chat_turn(socket, chat["id"].as_str().unwrap(), &chat["status"], "Greet the reader").await;
    assert_eq!(statuses, ["done", "working", "done"]);
}

/// Each window's share used and reset time, from the first `usage.changed` that satisfies `until`.
async fn windows(socket: &mut Socket, until: impl Fn(&Windows) -> bool) -> Windows {
    timeout(Duration::from_secs(20), async {
        loop {
            let message = next(socket).await;
            if message["type"] == "usage.changed" {
                let windows = parse(&message);
                if until(&windows) {
                    return windows;
                }
            }
        }
    })
    .await
    .expect("the windows never arrived")
}

fn parse(message: &Value) -> Windows {
    let readings = message["readings"].as_array().unwrap();
    let windows: Windows = readings
        .iter()
        .map(|reading| {
            let key = key(reading["agent"].as_str().unwrap(), reading["window"].as_str().unwrap());
            (key, (reading["used"].as_f64().unwrap(), reading["resetsAt"].as_u64().unwrap()))
        })
        .collect();
    assert_eq!(windows.len(), readings.len(), "a window was read twice: {message}");
    windows
}

fn table(windows: [(&str, &str, f64, u64); 4]) -> Windows {
    windows.into_iter().map(|(agent, window, used, resets_at)| (key(agent, window), (used, resets_at))).collect()
}

fn key(agent: &str, window: &str) -> (String, String) {
    (agent.to_owned(), window.to_owned())
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}
