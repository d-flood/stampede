mod common;

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use common::{Server, close_chat, control, leftovers, next, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const COLOR: &str = "9afa077a-3173-4e49-9197-7ed54344b24e";
const GRILL: &str = "4c1f0d2e-5b7a-4e8c-9a3d-2f6b8e1c7a90";
const MODEL: &str = "7e2a9b14-3c5d-4f61-8a07-b9d4c3e2f158";
const THREAD: &str = "01a1466b-fe86-7280-931d-fb5403fa449b";
const EXEC: &str = "01a1466c-11c9-7151-ac0b-539796acb463";

const RIGS: &str = r#"
[[rig]]
name = "codex"
agent = "codex"
command = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "resume", OIKONOMOS_FAKE_THREADS = "threads" }

[[rig]]
name = "claude"
agent = "claude-code"
command = "claude"
env = { OIKONOMOS_FAKE_FIXTURE = "resume" }

[[rig]]
name = "other claude"
agent = "claude-code"
command = "claude"
env = { OIKONOMOS_FAKE_FIXTURE = "simple-turn" }
"#;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn past_claude_questions_are_recovered_without_duplicating_saved_answers() {
    let (mut server, mut socket, project_id) = start(RIGS, Vec::new()).await;
    let questions = json!([
        { "header": "Color", "question": "Choose a color", "options": [{ "label": "Red", "description": "Warm", "preview": "Red   warm\nBlue  cool" }], "multiSelect": false },
        { "header": "Shapes", "question": "Choose shapes", "options": [{ "label": "Circle, round", "description": "Round" }, { "label": "Square", "description": "Four sides" }], "multiSelect": true }
    ]);
    let result = json!({ "questions": questions, "answers": { "Choose a color": "Red", "Choose shapes": "Circle, round, Square, Triangle" } });
    let entries = [
        json!({ "type": "user", "uuid": "prompt", "parentUuid": null, "message": { "role": "user", "content": "Ask me" } }),
        json!({ "type": "assistant", "uuid": "question", "parentUuid": "prompt", "message": { "role": "assistant", "content": [{ "type": "tool_use", "id": "past-question", "name": "AskUserQuestion", "input": { "questions": questions } }] } }),
        json!({ "type": "assistant", "uuid": "parallel-tool", "parentUuid": "question", "message": { "role": "assistant", "content": [{ "type": "tool_use", "id": "read", "name": "Read", "input": { "file_path": "notes.txt" } }] } }),
        json!({ "type": "user", "uuid": "answer", "parentUuid": "question", "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "past-question", "content": "Answered" }] }, "toolUseResult": result }),
        json!({ "type": "user", "uuid": "parallel-result", "parentUuid": "parallel-tool", "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "read", "content": "Notes" }] } }),
    ];
    fs::write(sessions(&server).join(format!("{COLOR}.jsonl")), entries.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap();
    let chat = resume(&mut socket, &project_id, "claude-code", COLOR).await;
    let chat_id = chat["id"].as_str().unwrap();
    attach(&mut socket, chat_id).await;
    let path = server.root.path().join("data/chats").join(format!("{chat_id}.json"));
    let mut saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let history = saved["answeredQuestions"].as_array().unwrap();
    assert_eq!(history.len(), 1, "old native questions must be available without answering again");
    assert_eq!(history[0]["questions"][0]["options"][0]["preview"], "Red   warm\nBlue  cool");
    assert_eq!(history[0]["answers"], json!([["Red"], ["Circle, round", "Square", "Triangle"]]));
    saved["answeredQuestions"][0].as_object_mut().unwrap().remove("source");
    fs::write(&path, saved.to_string()).unwrap();
    for _ in 0..2 {
        server.restart().await;
        let (mut restored, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
        control(&mut restored, json!({ "type": "hello", "deviceId": "recovered" })).await;
        next_control(&mut restored, "hello").await;
        attach(&mut restored, chat_id).await;
        let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["answeredQuestions"].as_array().unwrap().len(), 1);
        assert_eq!(saved["answeredQuestions"][0]["source"], "past-question");
        socket = restored;
    }
    close_chat(&mut socket, server.root.path(), chat_id).await;
}

#[tokio::test]
async fn both_agents_past_sessions_are_listed_newest_first() {
    let (server, mut socket, project_id) = start(RIGS, Vec::new()).await;
    let elsewhere = server.root.path().join("home/.claude/projects/-tmp-oikonomos-elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::copy(sessions(&server).join(format!("{COLOR}.jsonl")), elsewhere.join("0d9c4b7e-2f41-4a8e-9b3c-6e5d1a7f8c20.jsonl"))
        .unwrap();
    touch(&elsewhere.join("0d9c4b7e-2f41-4a8e-9b3c-6e5d1a7f8c20.jsonl"), 1_792_182_880);

    assert_eq!(
        list(&mut socket, &project_id).await,
        json!([
            { "agent": "claude-code", "sessionId": GRILL, "firstPrompt": "/grill the past sessions menu", "updatedAt": 1_792_182_860_000u64 },
            { "agent": "codex", "sessionId": EXEC, "firstPrompt": "Reply with exactly: ok", "updatedAt": 1_792_182_853_000u64 },
            { "agent": "claude-code", "sessionId": COLOR, "firstPrompt": "Name a color in one word.", "updatedAt": 1_792_182_850_000u64 },
            { "agent": "codex", "sessionId": THREAD, "firstPrompt": "Say hi in three words.", "updatedAt": 1_792_182_848_000u64 },
        ])
    );
    let methods: Vec<Value> = received(&server).iter().map(|message| message["method"].clone()).collect();
    assert_eq!(methods, ["initialize", "initialized", "thread/list"]);
}

#[tokio::test]
async fn a_past_claude_session_opens_as_a_chat_from_the_first_claude_rig() {
    let (server, mut socket, project_id) = start(RIGS, Vec::new()).await;
    let chat = resume(&mut socket, &project_id, "claude-code", COLOR).await;
    let chat_id = chat["id"].as_str().unwrap();
    assert_eq!(chat["rig"], "claude");
    assert_eq!(chat["agentSession"], COLOR);
    let color = [
        json!({ "kind": "user", "text": "Name a color in one word.", "images": 0 }),
        json!({ "kind": "text", "text": "Crimson", "streaming": false, "phase": "final" }),
    ];
    assert_eq!(attach(&mut socket, chat_id).await, color);
    let running = leftovers(server.root.path());
    assert!(running.iter().any(|process| process.contains(&format!(" --resume {COLOR}"))), "{running:?}");

    let listed = list(&mut socket, &project_id).await;
    let ids: Vec<&Value> = listed.as_array().unwrap().iter().map(|session| &session["sessionId"]).collect();
    assert_eq!(ids, [GRILL, EXEC, THREAD]);

    assert_eq!(turn(&mut socket, chat_id, "Now name a fruit in one word.").await, ["working", "done"]);
    let fruit = [
        json!({ "kind": "user", "text": "Now name a fruit in one word.", "images": 0 }),
        json!({ "kind": "text", "text": "Mango", "streaming": false, "phase": "final" }),
    ];
    assert_eq!(attach(&mut socket, chat_id).await, [color, fruit].concat());
    let record: Value =
        serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{chat_id}.json"))).unwrap()).unwrap();
    assert_eq!(record["agentSession"], COLOR);
    close_chat(&mut socket, server.root.path(), chat_id).await;
}

#[tokio::test]
async fn a_past_codex_thread_opens_as_a_chat_from_the_first_codex_rig() {
    let (server, mut socket, project_id) = start(RIGS, Vec::new()).await;
    let chat = resume(&mut socket, &project_id, "codex", THREAD).await;
    let chat_id = chat["id"].as_str().unwrap();
    assert_eq!(chat["rig"], "codex");
    assert_eq!(chat["agentSession"], THREAD);
    let hi = [
        json!({ "kind": "user", "text": "Say hi in three words.", "images": 0 }),
        json!({ "kind": "thinking", "seconds": 0 }),
        json!({ "kind": "text", "text": "Hi there, friend!", "streaming": false, "phase": "final" }),
    ];
    assert_eq!(attach(&mut socket, chat_id).await, hi);

    assert_eq!(turn(&mut socket, chat_id, "Now say bye in two words.").await, ["working", "done"]);
    let bye = [
        json!({ "kind": "user", "text": "Now say bye in two words.", "images": 0 }),
        json!({ "kind": "text", "text": "Bye, friend!", "streaming": false, "phase": "final" }),
    ];
    assert_eq!(attach(&mut socket, chat_id).await, [&hi[..], &bye].concat());
    close_chat(&mut socket, server.root.path(), chat_id).await;
    let received = received(&server);
    let methods: Vec<Value> = received.iter().map(|message| message["method"].clone()).collect();
    assert_eq!(methods, ["initialize", "initialized", "thread/resume", "model/list", "skills/list", "turn/start", "thread/unsubscribe"]);
    assert_eq!(received[2]["params"], json!({ "threadId": THREAD, "config": {
        "tools.update_plan": { "enabled": true },
        "mcp_servers.oikonomos_chat": { "command": env!("CARGO_BIN_EXE_oikonomos"), "args": ["chat-tools"] },
    } }));
}

#[tokio::test]
async fn without_a_rig_a_session_opens_with_its_agents_defaults() {
    let (server, mut socket, project_id) = start("", vec![("OIKONOMOS_FAKE_FIXTURE", "resume".into())]).await;
    for (agent, session, rig) in [("claude-code", COLOR, "Claude Code"), ("codex", THREAD, "Codex")] {
        let chat = resume(&mut socket, &project_id, agent, session).await;
        assert_eq!((&chat["rig"], &chat["agentSession"]), (&json!(rig), &json!(session)));
        close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
    }
}

/// A server whose home holds the fixture Claude sessions, with a Device on the home Project.
async fn start(config: &str, mut env: Vec<(&'static str, OsString)>) -> (Server, Socket, Value) {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    env.push(("PATH", path.into()));
    let server = Server::with_env("chat", config, env).await;
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chats/claude-code/sessions");
    let sessions = sessions(&server);
    fs::create_dir_all(&sessions).unwrap();
    for (session, seconds) in [(COLOR, 1_792_182_850), (GRILL, 1_792_182_860), (MODEL, 1_792_182_870)] {
        let name = format!("{session}.jsonl");
        fs::copy(fixtures.join(&name), sessions.join(&name)).unwrap();
        touch(&sessions.join(&name), seconds);
    }
    fs::create_dir(sessions.join(COLOR)).unwrap();

    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    (server, socket, project_id)
}

/// Where Claude keeps the sessions of the home Project.
fn sessions(server: &Server) -> PathBuf {
    let home = fs::canonicalize(server.root.path().join("home")).unwrap();
    let name: String = home.to_str().unwrap().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    server.root.path().join("home/.claude/projects").join(name)
}

fn touch(path: &Path, seconds: u64) {
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)).unwrap();
}

async fn list(socket: &mut Socket, project_id: &Value) -> Value {
    control(socket, json!({ "type": "sessions.list", "projectId": project_id })).await;
    let listed = timeout(Duration::from_secs(10), next_control(socket, "sessions.listed")).await;
    let listed = listed.expect("the sessions were never listed");
    assert_eq!(listed["projectId"], *project_id);
    listed["sessions"].clone()
}

async fn resume(socket: &mut Socket, project_id: &Value, agent: &str, session: &str) -> Value {
    control(socket, json!({ "type": "chat.resume", "projectId": project_id, "agent": agent, "sessionId": session })).await;
    let created = timeout(Duration::from_secs(10), next_control(socket, "chat.created")).await;
    let chat = created.expect("the chat was never created")["chat"].clone();
    assert_eq!(chat["agent"], agent);
    chat
}

async fn attach(socket: &mut Socket, chat_id: &str) -> Vec<Value> {
    control(socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    next_control(socket, "chat.items").await["items"].as_array().unwrap().clone()
}

/// Sends `text` and gathers the Chat's statuses until it is done again.
async fn turn(socket: &mut Socket, chat_id: &str, text: &str) -> Vec<Value> {
    control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": text })).await;
    let mut statuses = Vec::new();
    timeout(Duration::from_secs(20), async {
        while statuses.last().is_none_or(|status| status != "done") {
            let message = next(socket).await;
            if message["type"] == "chat.status" && message["chatId"] == chat_id {
                statuses.push(message["status"].clone());
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the turn never ended: {statuses:?}"));
    statuses
}

/// What the fake daemon was sent, in order.
fn received(server: &Server) -> Vec<Value> {
    let log = fs::read_to_string(server.root.path().join("home/.codex/fake-daemon/received.jsonl")).unwrap();
    log.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}
