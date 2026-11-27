use crate::common;

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
env = { STAMPEDE_FAKE_FIXTURE = "resume", STAMPEDE_FAKE_THREADS = "threads" }

[[rig]]
name = "claude"
agent = "claude-code"
command = "claude"
env = { STAMPEDE_FAKE_FIXTURE = "resume" }

[[rig]]
name = "other claude"
agent = "claude-code"
command = "claude"
env = { STAMPEDE_FAKE_FIXTURE = "simple-turn" }
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
    let elsewhere = server.root.path().join("home/.claude/projects/-tmp-stampede-elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(elsewhere.join("0d9c4b7e-2f41-4a8e-9b3c-6e5d1a7f8c20.jsonl"), fs::read_to_string(sessions(&server).join(format!("{COLOR}.jsonl"))).unwrap().replace(server.root.path().join("home").to_str().unwrap(), "/tmp/stampede-elsewhere")).unwrap();
    touch(&elsewhere.join("0d9c4b7e-2f41-4a8e-9b3c-6e5d1a7f8c20.jsonl"), 1_792_182_880);

    assert_eq!(
        projected(list(&mut socket, &project_id, |rows| rows.len() == 4).await),
        json!([
            { "agent": "claude-code", "sessionId": GRILL, "title": "/grill the past sessions menu", "updatedAt": 1_792_182_860_000u64 },
            { "agent": "codex", "sessionId": EXEC, "title": "Reply with exactly: ok", "updatedAt": 1_792_182_853_000u64 },
            { "agent": "claude-code", "sessionId": COLOR, "title": "Color naming request", "updatedAt": 1_792_182_850_000u64 },
            { "agent": "codex", "sessionId": THREAD, "title": "Say hi in three words.", "updatedAt": 1_792_182_848_000u64 },
        ])
    );
    assert!(server.root.path().join("home/.codex/fake-daemon/list-received.jsonl").is_file());
    let home = server.root.path().join("home");
    let mut watcher = inotify::Inotify::init().unwrap();
    watcher.watches().add(sessions(&server).join(format!("{COLOR}.jsonl")), inotify::WatchMask::OPEN).unwrap();
    let hold = home.join(".hold-session-list");
    fs::write(&hold, "").unwrap();
    let warm = timeout(Duration::from_secs(1), list(&mut socket, &project_id, |rows| rows.len() == 4)).await.expect("warm list waited on an Agent");
    assert_eq!(warm.as_array().unwrap().len(), 4);
    timeout(Duration::from_secs(10), async {
        while !home.join(".listing-held").exists() { tokio::time::sleep(Duration::from_millis(20)).await; }
    }).await.unwrap();
    let grill = sessions(&server).join(format!("{GRILL}.jsonl"));
    let content = fs::read_to_string(&grill).unwrap();
    fs::write(&grill, format!("{content}\n{}\n", json!({ "customTitle": "Changed session" }))).unwrap();
    let updated = list(&mut socket, &project_id, |rows| rows.iter().any(|row| row["title"] == "Changed session")).await;
    assert_eq!(updated.as_array().unwrap().len(), 4);
    let mut events = [0u8; 4096];
    assert!(matches!(watcher.read_events(&mut events), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock), "unchanged transcript was opened");
    assert!(list(&mut socket, &json!("not-open"), |rows| rows.is_empty()).await.as_array().unwrap().is_empty());
    fs::remove_file(hold).unwrap();

}

#[tokio::test]
async fn codex_sessions_prefer_names_and_fall_back_to_previews() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("threads.synthetic.json");
    let mut threads: Value = serde_json::from_str(include_str!("fixtures/chats/codex/threads.recorded.json")).unwrap();
    threads[0]["name"] = json!("A named Codex session");
    threads[0].as_object_mut().unwrap().remove("preview");
    threads[1]["name"] = json!(" \n ");
    fs::write(&path, threads.to_string()).unwrap();
    let config = RIGS.replace("STAMPEDE_FAKE_THREADS = \"threads\"", &format!("STAMPEDE_FAKE_THREADS = {:?}", path));
    let (_server, mut socket, project_id) = start(&config, Vec::new()).await;
    let listed = list(&mut socket, &project_id, |rows| rows.iter().filter(|row| row["key"]["agent"] == "codex").count() == 2).await;
    let codex: Vec<_> = listed.as_array().unwrap().iter().filter(|session| session["key"]["agent"] == "codex").map(|session| &session["title"]).collect();
    assert_eq!(codex, ["A named Codex session", "Say hi in three words."]);
}

#[tokio::test]
async fn claude_sessions_prefer_native_metadata_in_long_transcripts() {
    let (server, mut socket, project_id) = start(RIGS, Vec::new()).await;
    let path = sessions(&server).join(format!("{COLOR}.jsonl"));
    let prompt = json!({ "type": "user", "message": { "content": "Original prompt" } }).to_string();
    let padding = json!({ "type": "assistant", "message": { "content": "漢".repeat(800_000) } }).to_string();
    for (head, tail, expected) in [
        (json!({ "customTitle": "Manual title in the head" }), json!({ "aiTitle": "Generated title" }), "Manual title in the head"),
        (json!({ "customTitle": "Old title" }), json!({ "customTitle": "Renamed session", "aiTitle": "Generated title" }), "Renamed session"),
        (json!({ "aiTitle": "Old generated title" }), json!({ "customTitle": "  ", "aiTitle": "New generated title" }), "New generated title"),
        (json!({ "aiTitle": "Generated title in the head" }), json!({ "lastPrompt": "Recent prompt" }), "Generated title in the head"),
        (json!({}), json!({ "lastPrompt": "Recent prompt", "summary": "Older summary" }), "Recent prompt"),
        (json!({}), json!({ "summary": "Conversation summary" }), "Conversation summary"),
        (json!({}), json!({ "customTitle": "", "aiTitle": " ", "lastPrompt": "", "summary": "" }), "Original prompt"),
    ] {
        fs::write(&path, format!("{head}\n{prompt}\n{padding}\n{tail}\n")).unwrap();
        let listed = list(&mut socket, &project_id, |rows| rows.iter().any(|row| row["key"]["sessionId"] == COLOR && row["title"] == expected)).await;
        let session = listed.as_array().unwrap().iter().find(|session| session["key"]["sessionId"] == COLOR).unwrap();
        assert_eq!(session["title"], expected);
    }
    fs::write(&path, json!({ "type": "custom-title", "customTitle": "Title without a prompt" }).to_string()).unwrap();
    let listed = list(&mut socket, &project_id, |rows| rows.iter().any(|row| row["key"]["sessionId"] == COLOR && row["title"] == "Title without a prompt")).await;
    let session = listed.as_array().unwrap().iter().find(|session| session["key"]["sessionId"] == COLOR).unwrap();
    assert_eq!(session["title"], "Title without a prompt");
}

#[tokio::test]
async fn a_past_claude_session_opens_as_a_chat_from_the_first_claude_rig() {
    let second = tempfile::tempdir().unwrap();
    let config = RIGS.replace("STAMPEDE_FAKE_FIXTURE = \"simple-turn\"", &format!("STAMPEDE_FAKE_FIXTURE = \"resume\", CLAUDE_CONFIG_DIR = {:?}", second.path()));
    let (mut server, mut socket, project_id) = start(&config, Vec::new()).await;
    let home = server.root.path().join("home");
    let worktree = server.root.path().join("data/worktrees/_branches").join(project_id.as_str().unwrap()).join("branch");
    fs::create_dir_all(&worktree).unwrap();
    let slug: String = worktree.to_str().unwrap().chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' }).collect();
    let directory = second.path().join("projects").join(slug);
    fs::create_dir_all(&directory).unwrap();
    let transcript = fs::read_to_string(sessions(&server).join(format!("{COLOR}.jsonl"))).unwrap().replace(home.to_str().unwrap(), worktree.to_str().unwrap());
    fs::write(directory.join(format!("{COLOR}.jsonl")), transcript).unwrap();
    let second_store = fs::canonicalize(second.path()).unwrap();
    let rows = list(&mut socket, &project_id, |rows| rows.iter().any(|row| row["key"]["store"] == json!(second_store))).await;
    let key = &rows.as_array().unwrap().iter().find(|row| row["key"]["store"] == json!(second_store)).unwrap()["key"];
    control(&mut socket, json!({ "type": "chat.resume", "projectId": project_id, "key": key })).await;
    let other = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.created")).await.unwrap()["chat"].clone();
    assert_eq!(other["rig"], "other claude");
    assert_eq!(other["cwd"], json!(worktree));
    assert_eq!(other["sessionStore"], json!(second_store));
    close_chat(&mut socket, server.root.path(), other["id"].as_str().unwrap()).await;
    fs::remove_file(directory.join(format!("{COLOR}.jsonl"))).unwrap();
    // Wait for the deleted fixture to leave the catalogue before checking the default-store resume.
    list(&mut socket, &project_id, |rows| rows.iter().all(|row| row["key"]["store"] != json!(second_store)) && rows.iter().any(|row| row["key"]["sessionId"] == COLOR)).await;
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
    let record = server.root.path().join(format!("data/chats/{chat_id}.json"));
    let context = timeout(Duration::from_secs(10), async {
        loop {
            let chat: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
            if !chat["context"].is_null() { break chat["context"].clone(); }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    let cache = json!({ "expires": 1792186875163u64, "then": "cold" });
    assert_eq!(context.await.expect("the resumed chat never reported its context"), json!({ "used": 4436, "window": 1000000, "cache": cache }), "cold an hour after its last response");

    let listed = list(&mut socket, &project_id, |rows| rows.len() == 3 && rows.iter().all(|row| row["key"]["sessionId"] != COLOR)).await;
    let ids: Vec<&Value> = listed.as_array().unwrap().iter().map(|session| &session["key"]["sessionId"]).collect();
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

    let root = server.root.path();
    let repo = common::init(root, "repo");
    control(&mut socket, json!({ "type": "project.open", "path": repo })).await;
    let repo_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    common::Cli { data_dir: root.join("data") }.ok(&repo, &["epic", "add", "history", "--title", "History", "--brief", "-"], "# History");
    for (name, path) in [
        ("ticket", root.join("data/worktrees/history/1")),
        ("reviewer", root.join("data/worktrees/history/reviewer-1")),
        ("resolver", root.join("data/worktrees/history/integration")),
        ("branch", root.join("data/worktrees/_branches").join(repo_id.as_str().unwrap()).join("branch")),
    ] {
        let slug: String = path.to_str().unwrap().chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' }).collect();
        let directory = second.path().join("projects").join(slug);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(format!("{name}.jsonl")), format!("{}\n{}\n", json!({ "type": "queue-operation" }), json!({ "cwd": path, "customTitle": name }))).unwrap();
    }
    let rows = list(&mut socket, &repo_id, |rows| rows.iter().filter(|row| row["key"]["store"] == json!(second_store)).count() == 4).await;
    assert!(rows.as_array().unwrap().iter().all(|row| row["project"] == json!({ "id": repo_id, "path": repo })));
    control(&mut socket, json!({ "type": "project.close", "projectId": repo_id })).await;
    timeout(Duration::from_secs(10), async {
        loop {
            let projects = next_control(&mut socket, "projects.changed").await;
            if projects["projects"].as_array().unwrap().iter().all(|project| project["id"] != repo_id) { break; }
        }
    }).await.unwrap();
    list(&mut socket, &repo_id, |rows| rows.is_empty()).await;
    let databases = root.join("data/sessions");
    for delete in [false, true] {
        server.stop().await;
        let archive = rusqlite::Connection::open(databases.join("archive.sqlite")).unwrap();
        let path: String = archive.query_row("SELECT path FROM projects WHERE id = ?", [repo_id.as_str().unwrap()], |row| row.get(0)).unwrap();
        assert_eq!(path, repo.to_str().unwrap());
        drop(archive);
        let index = databases.join("index.sqlite");
        if delete {
            fs::remove_file(&index).unwrap();
        } else {
            let db = rusqlite::Connection::open(&index).unwrap();
            db.pragma_update(None, "user_version", 99).unwrap();
        }
        server.restart().await;
        timeout(Duration::from_secs(15), async {
            loop {
                if let Ok(db) = rusqlite::Connection::open_with_flags(&index, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
                    let count = db.query_row("SELECT count(*) FROM sessions WHERE project = ? AND store = ?", [repo_id.as_str().unwrap(), second_store.to_str().unwrap()], |row| row.get::<_, i64>(0));
                    let version = db.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0));
                    if matches!((count, version), (Ok(4), Ok(1))) { break; }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.unwrap();
    }
}

#[tokio::test]
async fn chats_take_their_agents_title_and_otherwise_their_first_prompt() {
    let (server, mut socket, project_id) = start(RIGS, Vec::new()).await;
    let path = sessions(&server).join(format!("{COLOR}.jsonl"));
    let session = fs::read_to_string(&path).unwrap() + &json!({ "type": "ai-title", "aiTitle": "Naming colors", "sessionId": COLOR }).to_string();
    fs::write(&path, session + "\n").unwrap();
    let resumed = resume(&mut socket, &project_id, "claude-code", COLOR).await;
    let resumed = resumed["id"].as_str().unwrap();
    titled(&server, resumed, "Naming colors").await;
    close_chat(&mut socket, server.root.path(), resumed).await;

    control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "other claude" })).await;
    let chat = next_control(&mut socket, "chat.created").await["chat"].clone();
    let chat_id = chat["id"].as_str().unwrap();
    assert_eq!(turn(&mut socket, chat_id, "Say hi\n  in three words.").await, ["working", "done"]);
    titled(&server, chat_id, "Say hi in three words.").await;
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
    assert_eq!(methods, [
        "initialize", "initialized", "thread/resume", "thread/items/list", "thread/items/list", "thread/turns/list",
        "model/list", "skills/list", "turn/start", "thread/unsubscribe",
    ]);
    assert_eq!(common::without_storage(&received[2]["params"]), json!({ "threadId": THREAD, "cwd": server.root.path().join("home"), "excludeTurns": true, "config": {
        "tools.update_plan": { "enabled": true },
        "tools.experimental_request_user_input": { "enabled": false },
        "mcp_servers.stampede_chat": { "command": env!("CARGO_BIN_EXE_stampede"), "args": ["chat-tools", "--data-dir", server.root.path().join("data"), "--chat-id", chat_id], "tool_timeout_sec": 86400 },
    } }));
}

#[tokio::test]
async fn a_codex_thread_larger_than_a_websocket_message_limit_opens() {
    let (server, mut socket, project_id) = start(RIGS, vec![("STAMPEDE_FAKE_PADDING", (65 << 20).to_string().into())]).await;
    let chat = resume(&mut socket, &project_id, "codex", THREAD).await;
    let chat_id = chat["id"].as_str().unwrap();
    assert_eq!(attach(&mut socket, chat_id).await[2]["text"], "Hi there, friend!");
    close_chat(&mut socket, server.root.path(), chat_id).await;
}

#[tokio::test]
async fn notifications_already_in_a_paged_codex_history_are_shown_once() {
    let mut entries: Vec<Value> = include_str!("fixtures/chats/codex/resume.recorded.jsonl").lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let resumed = entries.iter().position(|entry| entry["stdout"]["result"]["thread"]["turns"][0].is_object()).unwrap();
    let turn = entries[resumed]["stdout"]["result"]["thread"]["turns"][0].clone();
    let items = turn["items"].as_array().unwrap().iter().map(|item| {
        json!({ "stdout": { "method": "item/completed", "params": { "threadId": THREAD, "turnId": turn["id"], "item": item } } })
    });
    let echoes: Vec<Value> = items.chain([json!({ "stdout": { "method": "turn/completed", "params": { "threadId": THREAD, "turn": turn } } })]).collect();
    entries.splice(resumed + 1..resumed + 1, echoes);
    let scratch = tempfile::tempdir().unwrap();
    let fixture = scratch.path().join("echoes.recorded.jsonl");
    fs::write(&fixture, entries.iter().map(|entry| entry.to_string() + "\n").collect::<String>()).unwrap();

    let (server, mut socket, project_id) = start("", vec![("STAMPEDE_FAKE_FIXTURE", fixture.into()), ("STAMPEDE_FAKE_THREADS", "threads".into())]).await;
    let chat = resume(&mut socket, &project_id, "codex", THREAD).await;
    let chat_id = chat["id"].as_str().unwrap();
    assert_eq!(attach(&mut socket, chat_id).await, [
        json!({ "kind": "user", "text": "Say hi in three words.", "images": 0 }),
        json!({ "kind": "thinking", "seconds": 0 }),
        json!({ "kind": "text", "text": "Hi there, friend!", "streaming": false, "phase": "final" }),
    ]);
    close_chat(&mut socket, server.root.path(), chat_id).await;
}

#[tokio::test]
async fn without_a_rig_a_session_opens_with_its_agents_defaults() {
    let (server, mut socket, project_id) = start("", vec![("STAMPEDE_FAKE_FIXTURE", "resume".into()), ("STAMPEDE_FAKE_THREADS", "threads".into())]).await;
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
        fs::write(sessions.join(&name), fs::read_to_string(fixtures.join(&name)).unwrap().replace("/tmp/stampede-chat", server.root.path().join("home").to_str().unwrap())).unwrap();
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

fn projected(rows: Value) -> Value {
    json!(rows.as_array().unwrap().iter().map(|row| json!({
        "agent": row["key"]["agent"], "sessionId": row["key"]["sessionId"], "title": row["title"], "updatedAt": row["updatedAt"],
    })).collect::<Vec<_>>())
}

async fn list(socket: &mut Socket, project_id: &Value, expected: impl Fn(&[Value]) -> bool) -> Value {
    control(socket, json!({ "type": "sessions.list", "projectId": project_id })).await;
    timeout(Duration::from_secs(15), async {
        loop {
            let listed = next_control(socket, "sessions.listed").await;
            if listed["projectId"] == *project_id && expected(listed["sessions"].as_array().unwrap()) { break listed["sessions"].clone(); }
        }
    }).await.expect("the expected Sessions were never listed")
}

async fn resume(socket: &mut Socket, project_id: &Value, agent: &str, session: &str) -> Value {
    let listed = list(socket, project_id, |rows| rows.iter().any(|row| row["key"]["agent"] == agent && row["key"]["sessionId"] == session)).await;
    let key = &listed.as_array().unwrap().iter().find(|row| row["key"]["agent"] == agent && row["key"]["sessionId"] == session).unwrap()["key"];
    control(socket, json!({ "type": "chat.resume", "projectId": project_id, "key": key })).await;
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

async fn titled(server: &Server, chat_id: &str, title: &str) {
    let record = server.root.path().join(format!("data/chats/{chat_id}.json"));
    let titled = timeout(Duration::from_secs(10), async {
        while serde_json::from_slice::<Value>(&fs::read(&record).unwrap()).unwrap()["title"] != title {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    titled.await.unwrap_or_else(|_| panic!("the chat was never titled {title:?}"));
}

/// What the fake daemon was sent, in order.
fn received(server: &Server) -> Vec<Value> {
    let log = fs::read_to_string(server.root.path().join("home/.codex/fake-daemon/received.jsonl")).unwrap();
    log.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}
