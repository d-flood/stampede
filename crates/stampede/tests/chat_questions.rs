use crate::common;

use std::{env, fs, process::Stdio, time::Duration};

use common::{Server, close_chat, control, next, next_control};
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, net::TcpStream, process::Command, time::{sleep, timeout}};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn claude_receives_rich_answers_by_id_and_saves_them() { answers("claude-code").await; }

#[tokio::test]
async fn codex_receives_rich_answers_by_id_and_saves_them() { answers("codex").await; }

async fn answers(agent: &str) {
    let (server, mut socket, chat, request) = launch(agent).await;
    assert_eq!(request["questions"].as_array().unwrap().len(), 6);
    assert_eq!(request["questions"][0]["type"], "single");
    assert_eq!(request["questions"][0]["options"][0]["recommended"], true);
    let asset = request["questions"][0]["image"]["asset"].as_str().unwrap();
    fs::remove_file(server.root.path().join("home/preview.png")).unwrap();
    assert!(server.root.path().join("data/media").join(asset).is_file());
    let responses = json!({
        "storage": { "selected": ["local"], "text": "Keep offline support.", "skipped": false },
        "tools": { "selected": ["read", "write"], "text": "Ask before deleting.", "skipped": false },
        "notes": { "selected": [], "text": "First line\nSecond line", "skipped": false },
        "alternative": { "selected": [], "text": "A custom alternative", "skipped": false },
        "optional": { "selected": [], "text": "", "skipped": true },
        "finish": { "selected": [], "text": "Done", "skipped": false },
    });
    for invalid in [
        { let mut value = responses.clone(); value["storage"]["selected"] = json!(["local", "remote"]); value },
        { let mut value = responses.clone(); value["tools"]["selected"] = json!(["read", "read"]); value },
        { let mut value = responses.clone(); value["tools"]["selected"] = json!(["unknown"]); value },
        { let mut value = responses.clone(); value["notes"]["selected"] = json!(["read"]); value },
        { let mut value = responses.clone(); value["optional"]["text"] = json!("not skipped"); value },
        { let mut value = responses.clone(); value.as_object_mut().unwrap().remove("finish"); value },
    ] {
        control(&mut socket, json!({ "type": "chat.answer", "chatId": chat["id"], "requestId": request["id"], "answer": { "responses": invalid } })).await;
        control(&mut socket, json!({ "type": "chat.attach", "chatId": chat["id"] })).await;
        next_control(&mut socket, "chat.items").await;
        assert_eq!(next_control(&mut socket, "chat.request").await["request"]["id"], request["id"]);
    }
    control(&mut socket, json!({ "type": "chat.answer", "chatId": chat["id"], "requestId": request["id"], "answer": { "responses": responses } })).await;
    done(&mut socket, &chat).await;
    let result = result(&server);
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"], json!({ "answers": responses, "cancelled": false }));
    control(&mut socket, json!({ "type": "chat.answer", "chatId": chat["id"], "requestId": request["id"], "answer": { "allow": false } })).await;
    let saved: Value = serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{}.json", chat["id"].as_str().unwrap()))).unwrap()).unwrap();
    assert_eq!(saved["answeredQuestions"].as_array().unwrap().len(), 1);
    assert_eq!(saved["answeredQuestions"][0]["responses"], responses);
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn cancelling_questions_returns_no_answers_or_history() {
    for agent in ["claude-code", "codex"] {
        let (server, mut socket, chat, request) = launch(agent).await;
        control(&mut socket, json!({ "type": "chat.answer", "chatId": chat["id"], "requestId": request["id"], "answer": { "allow": false } })).await;
        done(&mut socket, &chat).await;
        assert_eq!(result(&server)["structuredContent"], json!({ "answers": {}, "cancelled": true }));
        control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
        assert_eq!(next_control(&mut socket, "hello").await["chats"][0]["answeredQuestions"], json!([]));
        close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
    }
}

#[tokio::test]
async fn a_replacement_instruction_reaches_the_agent_without_becoming_an_answer() {
    let (server, mut socket, chat, request) = launch("codex").await;
    control(&mut socket, json!({ "type": "chat.answer", "chatId": chat["id"], "requestId": request["id"],
        "answer": { "allow": false, "instead": "Use the existing defaults." } })).await;
    done(&mut socket, &chat).await;
    assert_eq!(result(&server)["structuredContent"], json!({ "answers": {}, "cancelled": true, "instruction": "Use the existing defaults." }));
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn invalid_batches_do_not_add_requests_and_a_disconnected_tool_is_removed() {
    let (server, mut socket, chat, request) = launch("claude-code").await;
    let question = json!({ "id": "one", "header": "One", "question": "Choose", "type": "single", "options": [{ "id": "a", "label": "A" }] });
    for arguments in [
        json!({ "questions": [] }),
        json!({ "questions": [question.clone(), question.clone()] }),
        json!({ "questions": [{ "id": "text", "header": "Text", "question": "Text", "type": "text", "options": [{ "id": "a", "label": "A" }] }] }),
        json!({ "questions": [{ "id": "choice", "header": "Choice", "question": "Choice", "type": "single" }] }),
        json!({ "questions": [question.clone()], "sections": [] }),
        json!({ "questions": [question.clone()], "extra": "x".repeat(256 * 1024) }),
    ] {
        let child = tool(&server, &chat, arguments).await;
        let output = timeout(Duration::from_secs(10), child.wait_with_output()).await.unwrap().unwrap();
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["result"]["isError"], true);
    }
    let mut child = tool(&server, &chat, json!({ "questions": [question] })).await;
    let second = timeout(Duration::from_secs(10), async {
        loop {
            let message = next(&mut socket).await;
            if message["type"] == "chat.request" && message["request"]["id"] != request["id"] { break message["request"].clone(); }
        }
    }).await.unwrap();
    child.kill().await.unwrap();
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next(&mut socket).await;
            if message["type"] == "chat.resolved" && message["requestId"] == second["id"] { break; }
        }
    }).await.unwrap();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat["id"] })).await;
    next_control(&mut socket, "chat.items").await;
    assert_eq!(next_control(&mut socket, "chat.request").await["request"]["id"], request["id"]);
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn interrupting_questions_cancels_the_waiting_tool() {
    let (server, mut socket, chat, _) = launch("claude-code").await;
    control(&mut socket, json!({ "type": "chat.interrupt", "chatId": chat["id"] })).await;
    timeout(Duration::from_secs(10), async {
        while !server.root.path().join("home/questions-result.json").exists() { sleep(Duration::from_millis(20)).await; }
    }).await.unwrap();
    assert_eq!(result(&server)["structuredContent"]["cancelled"], true);
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

async fn launch(agent: &str) -> (Server, Socket, Value, Value) {
    let fixture = format!("{}/tests/fixtures/chats/{agent}/rich-questions.synthetic.jsonl", env!("CARGO_MANIFEST_DIR"));
    let config = format!("[[rig]]\nname = \"test\"\nagent = \"{agent}\"\nenv = {{ STAMPEDE_FAKE_FIXTURE = \"{fixture}\" }}\n");
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("chat-questions", &config, vec![("PATH", path.into())]).await;
    fs::copy(format!("{}/../../web/static/icons/icon-512.png", env!("CARGO_MANIFEST_DIR")), server.root.path().join("home/preview.png")).unwrap();
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project, "rig": "test" })).await;
    let chat = next_control(&mut socket, "chat.created").await["chat"].clone();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat["id"] })).await;
    next_control(&mut socket, "chat.items").await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat["id"], "text": "Ask me questions." })).await;
    let request = next_control(&mut socket, "chat.request").await["request"].clone();
    (server, socket, chat, request)
}

async fn tool(server: &Server, chat: &Value, arguments: Value) -> tokio::process::Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stampede"))
        .arg("chat-tools").arg("--data-dir").arg(server.root.path().join("data"))
        .arg("--chat-id").arg(chat["id"].as_str().unwrap())
        .stdin(Stdio::piped()).stdout(Stdio::piped()).kill_on_drop(true).spawn().unwrap();
    child.stdin.take().unwrap().write_all(format!("{}\n", json!({ "id": 1, "method": "tools/call", "params": { "name": "ask_questions", "arguments": arguments } })).as_bytes()).await.unwrap();
    child
}

fn result(server: &Server) -> Value {
    serde_json::from_slice(&fs::read(server.root.path().join("home/questions-result.json")).unwrap()).unwrap()
}

async fn done(socket: &mut Socket, chat: &Value) {
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next(socket).await;
            if message["type"] == "chat.status" && message["chatId"] == chat["id"] && message["status"] == "done" { break; }
        }
    }).await.unwrap();
}
