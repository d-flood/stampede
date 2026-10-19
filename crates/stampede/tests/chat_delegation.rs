mod common;

use std::{env, fs, path::Path, process::Stdio, time::Duration};

use common::{Server, close_chat, control, next, next_control};
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, net::TcpStream, process::Command, time::{sleep, timeout}};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
const TASK: &str = "Review reviewed.txt without editing. Report incorrect values and the needed fix.";
const FINDING: &str = "reviewed.txt contains an incorrect value. Replace it with corrected value.";

#[tokio::test]
async fn claude_delegates_a_review_to_codex_then_fixes_the_finding() {
    review("claude-code", "codex").await;
}

#[tokio::test]
async fn codex_delegates_a_review_to_claude_then_fixes_the_finding() {
    review("codex", "claude-code").await;
}

async fn review(agent: &str, target: &str) {
    let (server, mut socket, source, _fixtures) = launch(agent, target, true).await;
    fs::write(server.root.path().join("home/reviewed.txt"), "incorrect value\n").unwrap();
    control(&mut socket, json!({ "type": "chat.send", "chatId": source["id"], "text": "Review this with the other rig and fix any issues found." })).await;
    gate(&server, "review").await;
    let live = timeout(Duration::from_secs(10), async {
        loop {
            let message = next(&mut socket).await;
            if message["type"] == "chat.item" && message["item"]["kind"] == "subagent" { break message["item"].clone(); }
        }
    }).await.unwrap();
    assert_eq!(live["name"], "target");
    assert_eq!(live["status"], "working");
    assert_eq!(live["prompt"], TASK);
    assert_eq!(fs::read_to_string(server.root.path().join("home/reviewed.txt")).unwrap(), "incorrect value\n");
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    assert_eq!(next_control(&mut socket, "hello").await["chats"].as_array().unwrap().len(), 1);
    let child: Value = serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{}.json", live["id"].as_str().unwrap()))).unwrap()).unwrap();
    assert_eq!(child["agent"], target);
    assert_eq!(child["cwd"], source["cwd"]);
    assert_eq!(child["projectId"], source["projectId"]);
    assert_eq!(child["parent"], source["id"]);
    if target == "codex" {
        let received = fs::read_to_string(server.root.path().join("home/.codex/fake-daemon/received.jsonl")).unwrap();
        let started = received.lines().map(|line| serde_json::from_str::<Value>(line).unwrap()).find(|message| message["method"] == "thread/start").unwrap();
        assert_eq!(started["params"]["model"], "gpt-6-luna");
        assert_eq!(started["params"]["config"]["model_reasoning_effort"], "low");
        assert_eq!(started["params"]["sandbox"], "read-only");
    }
    fs::remove_file(server.root.path().join("home/review")).unwrap();
    let mut received = Vec::new();
    let finished = timeout(Duration::from_secs(15), async {
        loop {
            let message = next(&mut socket).await;
            received.push(message.clone());
            if message["type"] == "chat.status" && message["chatId"] == source["id"] && message["status"] == "done" { break; }
        }
    }).await;
    assert!(finished.is_ok(), "messages: {received:?}; result: {:?}", fs::read_to_string(server.root.path().join("home/delegation-result.json")));
    let result: Value = serde_json::from_slice(&fs::read(server.root.path().join("home/delegation-result.json")).unwrap()).unwrap();
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["report"], FINDING);
    assert_eq!(fs::read_to_string(server.root.path().join("home/reviewed.txt")).unwrap(), "corrected value\n");
    control(&mut socket, json!({ "type": "chat.attach", "chatId": source["id"] })).await;
    let attached = next_control(&mut socket, "chat.items").await;
    let delegated: Vec<_> = attached["items"].as_array().unwrap().iter().filter(|item| item["kind"] == "subagent").collect();
    assert_eq!(delegated.len(), 1);
    assert_eq!(delegated[0]["status"], "done");
    assert_eq!(delegated[0]["outcome"], "completed");
    assert_eq!(delegated[0]["report"], FINDING);
    assert!(delegated[0]["items"].as_array().unwrap().iter().any(|item| item["kind"] == "text" && item["text"] == FINDING));
    let id = live["id"].as_str().unwrap();
    timeout(Duration::from_secs(10), async {
        while server.root.path().join(format!("data/chats/{id}.json")).exists() { sleep(Duration::from_millis(20)).await; }
    }).await.unwrap();
    close_chat(&mut socket, server.root.path(), source["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn starter_rigs_are_editable_discoverable_and_delegatable_and_survive_restart() {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let mut server = Server::with_env("starter-rigs", "", vec![
        ("PATH", path.into()), ("STAMPEDE_FAKE_FIXTURE", "simple-turn".into()),
    ]).await;
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/api/rigs", server.port);
    let rigs: Value = serde_json::from_slice(&client.get(&url).send().await.unwrap().bytes().await.unwrap()).unwrap();
    assert_eq!(rigs[0]["name"], "Claude Code");
    assert_eq!(rigs[1]["name"], "Codex");
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project, "rig": "Claude Code" })).await;
    let source = next_control(&mut socket, "chat.created").await["chat"].clone();
    let response = client.put(&url).header("Content-Type", "application/json")
        .body(json!({ "original": "Codex", "name": "Reviewer", "agent": "codex", "mode": "approve-for-me" }).to_string())
        .send().await.unwrap();
    assert_eq!(response.status(), 200, "{}", response.text().await.unwrap());
    let listed = call(&server, &source, "list_rigs", json!({})).await;
    let listed: Value = serde_json::from_str(listed["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(listed[1]["name"], "Reviewer");
    let result = call(&server, &source, "delegate_to_rig", json!({
        "rig": "Reviewer", "task": "Greet the reader in one short sentence with one word in **bold**. Use no names and no tools.",
    })).await;
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["outcome"], "completed");
    assert_eq!(result["structuredContent"]["rig"], "Reviewer");
    close_chat(&mut socket, server.root.path(), source["id"].as_str().unwrap()).await;
    let saved = fs::read_to_string(server.root.path().join("config.toml")).unwrap();
    server.restart().await;
    assert_eq!(fs::read_to_string(server.root.path().join("config.toml")).unwrap(), saved);
    let rigs: Value = serde_json::from_slice(&client.get(&url).send().await.unwrap().bytes().await.unwrap()).unwrap();
    assert_eq!(rigs.as_array().unwrap().len(), 2);
    assert_eq!(rigs[1]["name"], "Reviewer");
}

#[tokio::test]
async fn saved_rig_names_are_discoverable_and_can_be_used_for_delegation_after_renaming() {
    let (server, mut socket, source, _fixtures) = launch("claude-code", "codex", false).await;
    let client = reqwest::Client::new();
    for (original, name, agent, model, effort, mode) in [
        (Some("source"), "Writer", "claude-code", "opus", "high", "auto"),
        (Some("target"), "Careful Reviewer", "codex", "gpt-6-luna", "low", "read-only"),
        (None, "Quick Reviewer", "codex", "gpt-6-luna", "low", "read-only"),
    ] {
        let response = client.put(format!("http://127.0.0.1:{}/api/rigs", server.port))
            .header("Content-Type", "application/json")
            .body(json!({ "original": original, "name": name, "agent": agent, "model": model, "effort": effort, "mode": mode }).to_string())
            .send().await.unwrap();
        assert_eq!(response.status(), 200, "{}", response.text().await.unwrap());
    }
    let record: Value = serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{}.json", source["id"].as_str().unwrap()))).unwrap()).unwrap();
    assert_eq!(record["rig"], "Writer");
    let rigs = call(&server, &source, "list_rigs", json!({})).await;
    let rigs: Value = serde_json::from_str(rigs["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(rigs[0]["name"], "Writer");
    assert_eq!(rigs[1]["name"], "Careful Reviewer");
    assert_eq!(rigs[2]["name"], "Quick Reviewer");
    let worker = tool(&server, &source, "delegate_to_rig", json!({ "rig": "Careful Reviewer", "task": TASK })).await;
    gate(&server, "review").await;
    fs::remove_file(server.root.path().join("home/review")).unwrap();
    let output = timeout(Duration::from_secs(10), worker.wait_with_output()).await.unwrap().unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["result"]["isError"], false);
    assert_eq!(result["result"]["structuredContent"]["outcome"], "completed");
    assert_eq!(result["result"]["structuredContent"]["report"], FINDING);
    close_chat(&mut socket, server.root.path(), source["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn delegation_discovers_rigs_and_rejects_invalid_tasks_without_starting_children() {
    let (server, mut socket, source, _fixtures) = launch("claude-code", "codex", false).await;
    let rigs = call(&server, &source, "list_rigs", json!({})).await;
    let rigs: Value = serde_json::from_str(rigs["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(rigs[0]["name"], "source");
    assert_eq!(rigs[1]["agent"], "codex");
    assert!(rigs.as_array().unwrap().iter().all(|rig| rig.get("env").is_none()));
    for arguments in [json!({ "rig": "missing", "task": TASK }), json!({ "rig": "target", "task": " " }),
        json!({ "rig": "target", "task": TASK, "extra": true }), json!({ "rig": "target", "task": "x".repeat(65537) })]
    {
        assert_eq!(call(&server, &source, "delegate_to_rig", arguments).await["isError"], true);
    }
    assert_eq!(fs::read_dir(server.root.path().join("data/chats")).unwrap().filter(|entry| entry.as_ref().unwrap().path().extension().is_some_and(|extension| extension == "json")).count(), 1);
    close_chat(&mut socket, server.root.path(), source["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn a_delegated_approval_is_answered_from_the_calling_chat() {
    let (server, mut socket, source, fixtures) = launch("codex", "claude-code", false).await;
    let path = fixtures.path().join("target.jsonl");
    let mut entries: Vec<Value> = fs::read_to_string(&path).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let at = entries.iter().position(|entry| entry.get("gate").is_some()).unwrap();
    entries.splice(at + 1..at + 1, [
        json!({ "stdout": { "type": "control_request", "request_id": "review-approval", "request": {
            "subtype": "can_use_tool", "tool_name": "Bash", "input": { "command": "cat reviewed.txt" }, "tool_use_id": "read-reviewed",
        } } }),
        json!({ "stdin": { "type": "control_response", "response": { "request_id": "review-approval" } } }),
    ]);
    fs::write(&path, entries.iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
    let worker = tool(&server, &source, "delegate_to_rig", json!({ "rig": "target", "task": TASK })).await;
    gate(&server, "review").await;
    fs::remove_file(server.root.path().join("home/review")).unwrap();
    let asked = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.request")).await.unwrap();
    assert_eq!(asked["chatId"], source["id"]);
    assert_eq!(asked["request"]["kind"], "command");
    let child = asked["request"]["subagent"].as_str().unwrap();
    assert!(asked["request"]["id"].as_str().unwrap().starts_with(&format!("{child}/")));
    control(&mut socket, json!({ "type": "chat.answer", "chatId": source["id"], "requestId": asked["request"]["id"], "answer": { "allow": true } })).await;
    let output = timeout(Duration::from_secs(10), worker.wait_with_output()).await.unwrap().unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["result"]["isError"], false);
    let received = fs::read_to_string(server.root.path().join("home/target-received.jsonl")).unwrap();
    let answer = received.lines().map(|line| serde_json::from_str::<Value>(line).unwrap()).find(|message| message["type"] == "control_response" && message["response"]["request_id"] == "review-approval").unwrap();
    assert_eq!(answer["response"]["response"]["behavior"], "allow");
    let resolved = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.resolved")).await.unwrap();
    assert_eq!(resolved["requestId"], asked["request"]["id"]);
    control(&mut socket, json!({ "type": "chat.attach", "chatId": source["id"] })).await;
    next_control(&mut socket, "chat.items").await;
    loop {
        let message = next(&mut socket).await;
        assert_ne!(message["type"], "chat.request", "{message}");
        if message["type"] == "chat.capabilities" { break; }
    }
    close_chat(&mut socket, server.root.path(), source["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn a_failed_review_returns_an_error_and_keeps_its_partial_report() {
    let (server, mut socket, source, fixtures) = launch("codex", "claude-code", false).await;
    let path = fixtures.path().join("target.jsonl");
    let mut entries: Vec<Value> = fs::read_to_string(&path).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let result = entries.iter_mut().find(|entry| entry["stdout"]["type"] == "result").unwrap();
    result["stdout"]["subtype"] = json!("error_max_turns");
    result["stdout"]["is_error"] = json!(true);
    result["stdout"]["errors"] = json!(["Review turn limit reached."]);
    fs::write(&path, entries.iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
    let worker = tool(&server, &source, "delegate_to_rig", json!({ "rig": "target", "task": TASK })).await;
    gate(&server, "review").await;
    fs::remove_file(server.root.path().join("home/review")).unwrap();
    let output = timeout(Duration::from_secs(10), worker.wait_with_output()).await.unwrap().unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["result"]["isError"], true);
    assert_eq!(result["result"]["structuredContent"]["outcome"], "failed");
    assert!(result["result"]["structuredContent"]["report"].as_str().unwrap().contains(FINDING));
    close_chat(&mut socket, server.root.path(), source["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn stopping_the_caller_stops_a_delegated_review() {
    let (server, mut socket, source, _fixtures) = launch("claude-code", "codex", false).await;
    let worker = tool(&server, &source, "delegate_to_rig", json!({ "rig": "target", "task": TASK })).await;
    gate(&server, "review").await;
    control(&mut socket, json!({ "type": "chat.close", "chatId": source["id"] })).await;
    let output = timeout(Duration::from_secs(15), worker.wait_with_output()).await.unwrap().unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["result"]["isError"], true);
    assert_eq!(result["result"]["structuredContent"]["outcome"], "stopped");
    fs::remove_file(server.root.path().join("home/review")).unwrap();
}

async fn launch(agent: &str, target: &str, delegates: bool) -> (Server, Socket, Value, tempfile::TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.jsonl");
    let child = directory.path().join("target.jsonl");
    fixture(&source, agent, if delegates { Some(json!({ "rig": "target", "task": TASK })) } else { None }, false, "Review complete; fixed the issue.");
    fixture(&child, target, None, true, FINDING);
    let (model, mode) = if target == "codex" { ("gpt-6-luna", "read-only") } else { ("sonnet", "default") };
    if target == "claude-code" {
        let mut entries = fs::read_to_string(&child).unwrap();
        entries.push_str(&format!("{}\n", json!({ "args": ["--model", model, "--effort", "low", "--permission-mode", mode] })));
        fs::write(&child, entries).unwrap();
    }
    let config = format!(r#"
[[rig]]
name = "source"
agent = "{agent}"
env = {{ STAMPEDE_FAKE_FIXTURE = "{}", STAMPEDE_FAKE_RECEIVED = "source-received.jsonl" }}
[[rig]]
name = "target"
agent = "{target}"
model = "{model}"
effort = "low"
mode = "{mode}"
env = {{ STAMPEDE_FAKE_FIXTURE = "{}", STAMPEDE_FAKE_GATES = ".", STAMPEDE_FAKE_RECEIVED = "target-received.jsonl" }}
"#, source.display(), child.display());
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("chat-delegation", &config, vec![("PATH", path.into())]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project, "rig": "source" })).await;
    let source = next_control(&mut socket, "chat.created").await["chat"].clone();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": source["id"] })).await;
    next_control(&mut socket, "chat.items").await;
    (server, socket, source, directory)
}

fn fixture(path: &Path, agent: &str, delegation: Option<Value>, gated: bool, answer: &str) {
    let recorded = format!("{}/tests/fixtures/chats/{agent}/handoff.synthetic.jsonl", env!("CARGO_MANIFEST_DIR"));
    let entries: Vec<Value> = fs::read_to_string(recorded).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let mut output;
    if agent == "claude-code" {
        output = entries[..4].to_vec();
        if gated { output.push(json!({ "gate": "review" })); }
        if let Some(arguments) = delegation {
            output.push(json!({ "stdout": { "type": "assistant", "message": { "id": "delegate-message", "content": [{ "type": "tool_use", "id": "rig-review", "name": "mcp__stampede_chat__delegate_to_rig", "input": arguments }] } } }));
            output.push(json!({ "delegate": arguments }));
        }
        for event in [
            json!({ "type": "message_start", "message": { "id": "final", "content": [] } }),
            json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": answer } }),
            json!({ "type": "content_block_stop", "index": 0 }),
            json!({ "type": "message_stop" }),
        ] { output.push(json!({ "stdout": { "type": "stream_event", "event": event } })); }
        output.push(json!({ "stdout": { "type": "assistant", "message": { "id": "final", "content": [{ "type": "text", "text": answer }] } } }));
        output.push(json!({ "stdout": { "type": "result", "subtype": "success" } }));
    } else {
        output = entries[..11].to_vec();
        let thread = entries[8]["stdout"]["params"]["threadId"].clone();
        let turn = entries[8]["stdout"]["params"]["turn"]["id"].clone();
        if gated { output.push(json!({ "gate": "review" })); }
        if let Some(arguments) = delegation {
            output.push(json!({ "stdout": { "method": "item/started", "params": { "threadId": thread, "turnId": turn, "item": {
                "type": "mcpToolCall", "id": "rig-review", "server": "stampede_chat", "tool": "delegate_to_rig", "arguments": arguments, "status": "inProgress",
            } } } }));
            output.push(json!({ "delegate": arguments, "thread": thread, "turn": turn }));
        }
        output.push(json!({ "stdout": { "method": "item/completed", "params": { "threadId": thread, "turnId": turn, "item": { "type": "agentMessage", "id": "final", "phase": "final_answer", "text": answer } } } }));
        output.push(json!({ "stdout": { "method": "turn/completed", "params": { "threadId": thread, "turn": { "id": turn, "status": "completed", "items": [] } } } }));
        output.push(json!({ "stdin": { "method": "thread/unsubscribe", "id": 90 } }));
        output.push(json!({ "stdout": { "id": 90, "result": {} } }));
    }
    fs::write(path, output.iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
}

async fn gate(server: &Server, name: &str) {
    timeout(Duration::from_secs(10), async {
        while !server.root.path().join("home").join(name).exists() { sleep(Duration::from_millis(20)).await; }
    }).await.unwrap();
}

async fn tool(server: &Server, source: &Value, name: &str, arguments: Value) -> tokio::process::Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stampede"))
        .arg("chat-tools").arg("--data-dir").arg(server.root.path().join("data"))
        .arg("--chat-id").arg(source["id"].as_str().unwrap())
        .stdin(Stdio::piped()).stdout(Stdio::piped()).kill_on_drop(true).spawn().unwrap();
    child.stdin.take().unwrap().write_all(format!("{}\n", json!({ "id": 1, "method": "tools/call", "params": { "name": name, "arguments": arguments } })).as_bytes()).await.unwrap();
    child
}

async fn call(server: &Server, source: &Value, name: &str, arguments: Value) -> Value {
    let child = tool(server, source, name, arguments).await;
    let output = timeout(Duration::from_secs(10), child.wait_with_output()).await.unwrap().unwrap();
    assert!(output.status.success());
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"].clone()
}
