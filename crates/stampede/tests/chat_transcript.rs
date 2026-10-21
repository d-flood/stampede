mod common;

use std::{env, fs, time::Duration};

use common::{Server, control, next, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const CLAUDE_PROMPT: &str = "Think it over briefly first. Then, in order: read notes.txt and todo.txt, search the web for \"IANA example domain\", run `wc -l notes.txt` with Bash, run `wc -l missing.txt` with Bash (it will fail), and read notes.txt once more. Finally answer in one short sentence.";
const CLAUDE_ANSWER: &str = "All steps are done: notes.txt has 3 lines and \"needle\" is on line 2, todo.txt also mentions the needle, `wc` failed on missing.txt because the file doesn't exist, and IANA keeps example.com/.org/.net for documentation.\n\nSources: [IANA Reserved Domains](https://res-dom.iana.org), [.example – Wikipedia](https://en.wikipedia.org/wiki/.example)";
const CODEX_PROMPT: &str = "Think it over briefly first. Then, in order: run `wc -l notes.txt`, run `wc -l missing.txt` (it will fail), and search the web for \"IANA example domain\". Finally answer in one short sentence.";
const CODEX_ANSWER: &str = "`notes.txt` has 3 lines, `missing.txt` was absent, and the search found [IANA’s Example Domains page](https://www.iana.org/help/example-domains).";
const DONE: &str = "Say done in one word.";
const CLAUDE_SUBAGENT_PROMPT: &str = "Read both /tmp/stampede-chat/notes.txt and /tmp/stampede-chat/todo.txt. Report which one (or both/neither) contains the word \"needle\". Reply with just the filename(s) and the matching line(s). Breadth: quick.";
const CLAUDE_EDITS: &str = "Make these changes in order: in notes.txt change `beta` to `BETA` with Edit, create hello.py containing `print(\"hello\")` with Write, and in analysis.ipynb replace the first cell's source with `print(2)` with NotebookEdit. Then answer in one short sentence.";
const CODEX_EDITS: &str = "Use apply_patch to make these changes: in notes.txt change `beta` to `BETA`, add hello.py containing `print(\"hello\")`, and delete old.txt. Then answer in one short sentence.";
const NOTES_DIFF: &str = "@@ -1,3 +1,3 @@\n alpha\n-beta\n+BETA\n gamma\n";
const HELLO_DIFF: &str = "@@ -0,0 +1,1 @@\n+print(\"hello\")\n";

#[derive(Default)]
struct View {
    items: Vec<Value>,
    statuses: Vec<Value>,
    todos: Vec<Value>,
    children: Vec<Value>,
}

#[tokio::test]
async fn claude_internal_notifications_never_become_user_messages() {
    let directory = tempfile::tempdir().unwrap();
    let recording = fs::read_to_string(format!("{}/tests/fixtures/chats/claude-code/tools.recorded.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut fixture = String::new();
    let mut inserted = false;
    let notification = "<task-notification>\n<task-id>background-agent</task-id>\n<status>completed</status>\n<summary>Agent finished</summary>\n</task-notification>";
    for line in recording.lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        if !inserted && entry["stdout"]["type"] == "result" {
            let session = entry["stdout"]["session_id"].clone();
            for (index, content) in [json!(notification), json!([{ "type": "text", "text": notification }])].into_iter().enumerate() {
                for (key, session_key) in [("stdout", "session_id"), ("session", "sessionId")] {
                    let mut message = json!({
                        "type": "user",
                        "message": { "role": "user", "content": content },
                        "uuid": format!("internal-notification-{index}"),
                        "parent_tool_use_id": null,
                        "origin": { "kind": "task-notification", "producer": "session-task" },
                        "promptSource": "system",
                        "turnOrigin": "task_notification",
                        "timestamp": "2026-10-17T05:47:19.559Z",
                    });
                    message[session_key] = session.clone();
                    fixture.push_str(&json!({ key: message }).to_string());
                    fixture.push('\n');
                }
            }
            for (index, attachment) in [
                json!({
                    "type": "queued_command",
                    "prompt": notification,
                    "commandMode": "task-notification",
                    "origin": { "kind": "task-notification", "producer": "session-task" },
                }),
                json!({ "type": "queued_command", "prompt": "Continue with the queued work", "commandMode": "prompt" }),
            ].into_iter().enumerate() {
                fixture.push_str(&json!({ "session": {
                    "type": "attachment",
                    "sessionId": session,
                    "uuid": format!("queued-attachment-{index}"),
                    "attachment": attachment,
                    "renderedRole": if index == 0 { "system" } else { "user" },
                } }).to_string());
                fixture.push('\n');
            }
            inserted = true;
        }
        fixture.push_str(line);
        fixture.push('\n');
    }
    assert!(inserted);
    let path = directory.path().join("notifications.jsonl");
    fs::write(&path, fixture).unwrap();
    let (mut server, mut socket, chat_id) = launch("claude-code", path.to_str().unwrap()).await;
    let mut view = View::default();
    view.turn(&mut socket, &chat_id, CLAUDE_PROMPT).await;
    let users = |view: &View| view.items.iter().filter(|item| item["kind"] == "user").cloned().collect::<Vec<_>>();
    assert_eq!(users(&view), [user(CLAUDE_PROMPT)], "live notifications");
    server.restart().await;
    let (mut socket, mut restored) = reattach(&server, &chat_id, view.items.len()).await;
    assert_eq!(users(&restored), [user(CLAUDE_PROMPT)], "holder replay");
    restored.turn(&mut socket, &chat_id, "/compact").await;
    restored.turn(&mut socket, &chat_id, DONE).await;
    server.restart().await;
    let (_, restored) = reattach(&server, &chat_id, restored.items.len()).await;
    assert_eq!(users(&restored), [user(CLAUDE_PROMPT), user("Continue with the queued work"), user("/compact"), user(DONE)], "session history");
}

#[tokio::test]
async fn commands_are_visible_before_their_results_arrive() {
    for agent in ["codex", "claude-code"] {
        let directory = tempfile::tempdir().unwrap();
        let recording = fs::read_to_string(format!("{}/tests/fixtures/chats/{agent}/tools.recorded.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let mut gated = String::new();
        let mut inserted = false;
        for line in recording.lines() {
            gated.push_str(line);
            gated.push('\n');
            let entry: Value = serde_json::from_str(line).unwrap();
            let message = &entry["stdout"];
            let command = if agent == "codex" {
                message["method"] == "item/started" && message["params"]["item"]["type"] == "commandExecution"
            } else {
                message["type"] == "assistant" && message["message"]["content"].as_array().is_some_and(|blocks| blocks.iter().any(|block| block["name"] == "Bash"))
            };
            if command && !inserted {
                gated.push_str("{\"gate\":\"command\"}\n");
                inserted = true;
            }
        }
        assert!(inserted, "{agent}: no command in the fixture");
        let fixture = directory.path().join("tools.jsonl");
        fs::write(&fixture, gated).unwrap();
        let (_server, mut socket, chat_id) = launch_with_gates(agent, fixture.to_str().unwrap(), Some(directory.path())).await;
        control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Run the tools" })).await;
        timeout(Duration::from_secs(5), async {
            while !directory.path().join("command").exists() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.unwrap();
        loop {
            let message = next_control(&mut socket, "chat.item").await;
            if message["item"]["kind"] == "command" && message["item"]["command"] == "wc -l notes.txt" { break; }
        }
        control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
        let snapshot = next_control(&mut socket, "chat.items").await;
        assert!(snapshot["items"].as_array().unwrap().iter().any(|item| item["kind"] == "command" && item["command"] == "wc -l notes.txt"), "{agent}: current command is missing before completion: {snapshot}");
        fs::remove_file(directory.path().join("command")).unwrap();
    }
}

#[tokio::test]
async fn claude_shells_survive_reattachment_but_stop_when_the_holder_is_lost() {
    let directory = tempfile::tempdir().unwrap();
    let recording = fs::read_to_string(format!("{}/tests/fixtures/chats/claude-code/tools.recorded.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let fixture: Vec<_> = recording.lines().map(|line| {
        let mut entry: Value = serde_json::from_str(line).unwrap();
        for path in ["/stdout/tool_use_result", "/session/toolUseResult"] {
            if let Some(result) = entry.pointer_mut(path).filter(|result| result["stdout"] == "3 notes.txt") {
                result["backgroundTaskId"] = json!("background-shell");
            }
        }
        entry.to_string()
    }).collect();
    let path = directory.path().join("shells.jsonl");
    fs::write(&path, fixture.join("\n")).unwrap();
    let (mut server, mut socket, chat_id) = launch("claude-code", path.to_str().unwrap()).await;
    let mut view = View::default();
    view.turn(&mut socket, &chat_id, CLAUDE_PROMPT).await;
    let command = |view: &View| view.items.iter().find(|item| item["command"] == "wc -l notes.txt").unwrap().clone();
    assert_eq!(command(&view)["status"], "running");
    server.restart().await;
    let (_, view) = reattach(&server, &chat_id, view.items.len()).await;
    assert_eq!(command(&view)["status"], "running");
    server.stop().await;
    common::reboot(server.root.path());
    server.restart().await;
    let (_, view) = reattach(&server, &chat_id, view.items.len()).await;
    assert_eq!(command(&view)["status"], "failed");
    assert_eq!(command(&view)["output"], "3 notes.txt");
}

#[tokio::test]
async fn resumed_shells_are_checked_against_live_processes() {
    let directory = tempfile::tempdir().unwrap();
    let recording = fs::read_to_string(format!("{}/tests/fixtures/chats/codex/tools.recorded.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut fixture = String::new();
    let mut live_id = String::new();
    for line in recording.lines() {
        let mut entry: Value = serde_json::from_str(line).unwrap();
        let resumed = entry.pointer_mut("/stdout/result/thread/turns").and_then(Value::as_array_mut).filter(|turns| !turns.is_empty());
        let listing = if let Some(turns) = resumed {
            for item in turns.iter_mut().flat_map(|turn| turn["items"].as_array_mut().unwrap()) {
                if item["type"] == "commandExecution" {
                    item["status"] = json!("inProgress");
                    item["exitCode"] = Value::Null;
                    live_id = item["id"].as_str().unwrap().to_owned();
                }
            }
            true
        } else { false };
        fixture.push_str(&entry.to_string());
        fixture.push('\n');
        if listing {
            for entry in [
                json!({ "stdin": { "method": "thread/backgroundTerminals/list", "id": 70 } }),
                json!({ "stdout": { "id": 70, "result": { "data": [], "nextCursor": "live" } } }),
                json!({ "stdin": { "method": "thread/backgroundTerminals/list", "id": 71 } }),
                json!({ "stdout": { "id": 71, "result": { "data": [{ "itemId": live_id, "processId": "live" }], "nextCursor": null } } }),
                json!({ "gate": "shell-live" }),
                json!({ "stdin": { "method": "thread/backgroundTerminals/list", "id": 72 } }),
                json!({ "stdout": { "id": 72, "result": { "data": [], "nextCursor": null } } }),
            ] {
                fixture.push_str(&entry.to_string());
                fixture.push('\n');
            }
        }
    }
    assert!(!live_id.is_empty());
    let path = directory.path().join("shells.jsonl");
    fs::write(&path, fixture).unwrap();
    let (mut server, mut socket, chat_id) = launch_with_gates("codex", path.to_str().unwrap(), Some(directory.path())).await;
    let mut view = View::default();
    view.turn(&mut socket, &chat_id, CODEX_PROMPT).await;
    server.restart().await;
    let (mut socket, mut view) = reattach(&server, &chat_id, 0).await;
    let commands: Vec<_> = view.items.iter().filter(|item| item["kind"] == "command").collect();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0]["status"], "failed", "a lost shell must leave the running list");
    assert_eq!(commands[0]["output"], "3 notes.txt\n");
    assert_eq!(commands[1]["status"], "running", "a live shell must survive reconnection");
    fs::remove_file(directory.path().join("shell-live")).unwrap();
    view.follow(&mut socket, &chat_id, |view| !view.items.iter().any(|item| item["kind"] == "command" && item["status"] == "running")).await;
    assert_eq!(view.items.iter().filter(|item| item["kind"] == "command").count(), 2);
    let requests: Vec<Value> = fs::read_to_string(server.root.path().join("home/.codex/fake-daemon/received.jsonl")).unwrap()
        .lines().map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|message| message["method"] == "thread/backgroundTerminals/list").collect();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1]["params"]["cursor"], "live");
}

#[tokio::test]
async fn current_activity_is_reported_before_a_step_completes() {
    for agent in ["codex", "claude-code"] {
        for thinking in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let recording = fs::read_to_string(format!("{}/tests/fixtures/chats/{agent}/tools.recorded.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let mut gated = String::new();
            let mut inserted = false;
            let mut turns = 0;
            for line in recording.lines() {
                gated.push_str(line);
                gated.push('\n');
                let entry: Value = serde_json::from_str(line).unwrap();
                let message = &entry["stdout"];
                if agent == "codex" && message["method"] == "turn/started" {
                    turns += 1;
                    if turns == 2 && !thinking { gated.push_str("{\"gate\":\"compacting\"}\n"); }
                }
                let step = if agent == "codex" {
                    message["method"] == "item/started" && message["params"]["item"]["type"] == if thinking { "reasoning" } else { "webSearch" }
                } else if thinking {
                    message["type"] == "stream_event" && message["event"]["type"] == "content_block_start" && message["event"]["content_block"]["type"] == "thinking"
                } else {
                    message["type"] == "assistant" && message["message"]["content"].as_array().is_some_and(|blocks| blocks.iter().any(|block| block["name"] == "Read"))
                };
                if step && !inserted {
                    gated.push_str("{\"gate\":\"activity\"}\n");
                    inserted = true;
                }
            }
            assert!(inserted, "{agent}: no matching step");
            let fixture = directory.path().join("tools.jsonl");
            fs::write(&fixture, gated).unwrap();
            let (_server, mut socket, chat_id) = launch_with_gates(agent, fixture.to_str().unwrap(), Some(directory.path())).await;
            control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Run the tools" })).await;
            timeout(Duration::from_secs(5), async {
                while !directory.path().join("activity").exists() {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }).await.unwrap();
            let expected = if thinking { "Thinking" } else if agent == "codex" { "Searching" } else { "Reading" };
            loop {
                let message = next_control(&mut socket, "chat.activity").await;
                if message["activity"].as_str().is_some_and(|activity| activity.starts_with(expected)) { break; }
            }
            control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
            let snapshot = next_control(&mut socket, "hello").await;
            let chat = snapshot["chats"].as_array().unwrap().iter().find(|chat| chat["id"] == chat_id).unwrap();
            assert!(chat["activity"].as_str().is_some_and(|activity| activity.starts_with(expected)), "{agent}: expected {expected} before completion: {chat}");
            fs::remove_file(directory.path().join("activity")).unwrap();
            if agent == "codex" && !thinking {
                timeout(Duration::from_secs(5), async {
                    while !directory.path().join("compacting").exists() {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                }).await.unwrap();
                loop {
                    let message = next_control(&mut socket, "chat.status").await;
                    if message["status"] == "working" { break; }
                }
                control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
                let snapshot = next_control(&mut socket, "hello").await;
                let chat = snapshot["chats"].as_array().unwrap().iter().find(|chat| chat["id"] == chat_id).unwrap();
                assert_eq!(chat["status"], "working", "a turn started after completion must reactivate the tab");
                fs::remove_file(directory.path().join("compacting")).unwrap();
            }
            loop {
                let status = next_control(&mut socket, "chat.status").await;
                if status["status"] == "done" { break; }
            }
            control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
            let snapshot = next_control(&mut socket, "hello").await;
            let chat = snapshot["chats"].as_array().unwrap().iter().find(|chat| chat["id"] == chat_id).unwrap();
            assert!(chat["activity"].is_null(), "{agent}: completed activity is still current: {chat}");
        }
    }
}

#[tokio::test]
async fn a_codex_subagent_reports_its_current_command_before_completion() {
    let directory = tempfile::tempdir().unwrap();
    let recording = fs::read_to_string(format!("{}/tests/fixtures/chats/codex/subagents.recorded.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut gated = String::new();
    let mut inserted = false;
    for line in recording.lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        if entry.get("gate").is_some() { continue; }
        gated.push_str(line);
        gated.push('\n');
        let message = &entry["stdout"];
        if !inserted && message["method"] == "item/started" && message["params"]["item"]["command"].as_str().is_some_and(|command| command.contains("cat notes.txt todo.txt")) {
            gated.push_str("{\"gate\":\"activity\"}\n");
            inserted = true;
        }
    }
    assert!(inserted);
    let fixture = directory.path().join("subagents.jsonl");
    fs::write(&fixture, gated).unwrap();
    let (_server, mut socket, chat_id) = launch_with_gates("codex", fixture.to_str().unwrap(), Some(directory.path())).await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Find needle using a subagent and track the steps." })).await;
    timeout(Duration::from_secs(5), async {
        while !directory.path().join("activity").exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    loop {
        let message = next_control(&mut socket, "chat.item").await;
        if message["item"]["kind"] == "subagent" && message["item"]["activity"] == "Running cat notes.txt todo.txt" { break; }
    }
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    let snapshot = next_control(&mut socket, "chat.items").await;
    let child = snapshot["items"].as_array().unwrap().iter().find(|item| item["kind"] == "subagent").unwrap();
    assert_eq!(child["status"], "working");
    assert_eq!(child["activity"], "Running cat notes.txt todo.txt");
    assert!(child["items"].as_array().unwrap().iter().any(|item| item["kind"] == "command" && item["status"] == "running"));
    fs::remove_file(directory.path().join("activity")).unwrap();
}

#[tokio::test]
async fn claude_tools_thinking_and_commands_normalize_alike_live_and_from_the_session_file() {
    let (mut server, mut socket, chat_id) = launch("claude-code", "tools").await;
    let mut view = View::default();
    for prompt in [CLAUDE_PROMPT, "/compact"] {
        view.turn(&mut socket, &chat_id, prompt).await;
    }
    let search = json!({ "query": "select:WebSearch", "max_results": 1 }).to_string();
    let expected = [
        user(CLAUDE_PROMPT),
        json!({ "kind": "thinking", "seconds": 2 }),
        json!({ "kind": "tools", "calls": [
            { "kind": "read", "name": "Read", "detail": "notes.txt" },
            { "kind": "read", "name": "Read", "detail": "todo.txt" },
            { "kind": "other", "name": "ToolSearch", "detail": search },
            { "kind": "web", "name": "WebSearch", "detail": "IANA example domain" },
        ] }),
        json!({ "kind": "command", "command": "wc -l notes.txt", "status": "ok", "output": "3 notes.txt" }),
        json!({
            "kind": "command",
            "command": "wc -l missing.txt",
            "status": "failed",
            "output": "Exit code 1\nwc: missing.txt: No such file or directory",
        }),
        json!({ "kind": "tools", "calls": [{ "kind": "read", "name": "Read", "detail": "notes.txt" }] }),
        json!({ "kind": "thinking", "seconds": 2 }),
        text(CLAUDE_ANSWER),
        user("/compact"),
        json!({ "kind": "notice", "notice": "compacted", "text": "Conversation compacted" }),
        user(DONE),
        text("Done."),
    ];
    assert_eq!(view.items, expected[..10]);

    server.restart().await;
    let (mut socket, mut view) = reattach(&server, &chat_id, 10).await;
    assert_eq!(view.items, expected[..10], "a compaction replayed by the holder");
    view.turn(&mut socket, &chat_id, DONE).await;
    assert_eq!(view.items, expected);

    server.restart().await;
    let (_, view) = reattach(&server, &chat_id, expected.len()).await;
    assert_eq!(view.items, expected, "a compaction read from the session file");
}

#[tokio::test]
async fn codex_tools_reasoning_and_commands_normalize_to_blocks_live_and_from_the_thread() {
    let (mut server, mut socket, chat_id) = launch("codex", "tools").await;
    let mut view = View::default();
    for prompt in [CODEX_PROMPT, DONE] {
        view.turn(&mut socket, &chat_id, prompt).await;
    }
    let mut expected = [
        user(CODEX_PROMPT),
        json!({ "kind": "thinking", "seconds": 3 }),
        said("I’ll run both counts in order, then search the web.\n"),
        json!({ "kind": "command", "command": "wc -l notes.txt", "status": "ok", "output": "3 notes.txt\n" }),
        json!({
            "kind": "command",
            "command": "wc -l missing.txt",
            "status": "failed",
            "output": "wc: missing.txt: No such file or directory\n",
        }),
        json!({ "kind": "tools", "calls": [{ "kind": "web", "name": "webSearch", "detail": "IANA example domain" }] }),
        text(CODEX_ANSWER),
        json!({ "kind": "notice", "notice": "compacted", "text": "Conversation compacted" }),
        user(DONE),
        text("Done"),
    ];
    assert_eq!(view.items, expected);

    server.restart().await;
    let (_, view) = reattach(&server, &chat_id, expected.len()).await;
    expected[1] = json!({ "kind": "thinking", "seconds": 0 });
    assert_eq!(view.items, expected, "thread/resume carries no item times");
}

#[tokio::test]
async fn claude_edits_writes_and_notebook_edits_become_edit_items_live_and_from_the_session_file() {
    let (mut server, mut socket, chat_id) = launch("claude-code", "edits").await;
    let mut view = View::default();
    view.turn(&mut socket, &chat_id, CLAUDE_EDITS).await;
    let search = json!({ "query": "select:NotebookEdit", "max_results": 1 }).to_string();
    let notebook = [
        "@@ -6,9 +6,7 @@",
        r#"    "id": "a1b2c3d4","#,
        r#"    "metadata": {},"#,
        r#"    "outputs": [],"#,
        r#"-   "source": ["#,
        r#"-    "print(1)""#,
        "-   ]",
        r#"+   "source": "print(2)""#,
        "   }",
        "  ],",
        r#"  "metadata": {"#,
        "",
    ];
    let expected = [
        user(CLAUDE_EDITS),
        json!({ "kind": "thinking", "seconds": 0 }),
        json!({ "kind": "tools", "calls": [
            { "kind": "read", "name": "Read", "detail": "notes.txt" },
            { "kind": "read", "name": "Read", "detail": "analysis.ipynb" },
            { "kind": "other", "name": "ToolSearch", "detail": search },
        ] }),
        json!({ "kind": "thinking", "seconds": 1 }),
        edit("notes.txt", NOTES_DIFF, 1, 1),
        written("hello.py", HELLO_DIFF, 1),
        edit("analysis.ipynb", &notebook.join("\n"), 1, 3),
        text("I changed `beta` to `BETA` in notes.txt, created hello.py with `print(\"hello\")`, and set the first cell of analysis.ipynb to `print(2)`. ✅"),
        user(DONE),
        text("Done."),
    ];
    assert_eq!(view.items, expected[..8]);

    server.restart().await;
    let (mut socket, mut view) = reattach(&server, &chat_id, 8).await;
    assert_eq!(view.items, expected[..8], "edits replayed by the holder");
    view.turn(&mut socket, &chat_id, DONE).await;
    assert_eq!(view.items, expected);

    server.restart().await;
    let (_, view) = reattach(&server, &chat_id, expected.len()).await;
    assert_eq!(view.items, expected, "edits read from the session file");
}

#[tokio::test]
async fn each_file_of_a_codex_file_change_becomes_an_edit_item_live_and_from_the_thread() {
    let (mut server, mut socket, chat_id) = launch("codex", "edits").await;
    let mut view = View::default();
    view.turn(&mut socket, &chat_id, CODEX_EDITS).await;
    let expected = [
        user(CODEX_EDITS),
        said("I’ll apply the three file changes."),
        json!({
            "kind": "command",
            "command": "cat notes.txt old.txt",
            "status": "ok",
            "output": "alpha\nbeta\ngamma\nobsolete\n",
        }),
        written("hello.py", HELLO_DIFF, 1),
        edit("notes.txt", NOTES_DIFF, 1, 1),
        edit("old.txt", "@@ -1,1 +0,0 @@\n-obsolete\n", 0, 1),
        text("Changed `beta` to `BETA` in `notes.txt`, added `hello.py` containing `print(\"hello\")`, and deleted `old.txt`."),
    ];
    assert_eq!(view.items, expected);

    server.restart().await;
    let (_, view) = reattach(&server, &chat_id, expected.len()).await;
    assert_eq!(view.items, expected, "edits from thread/resume");
}

#[tokio::test]
async fn legacy_multiedit_and_unified_file_headers_preserve_edit_counts() {
    for (agent, prompt) in [("claude-code", CLAUDE_EDITS), ("codex", CODEX_EDITS)] {
        let fixture = fs::read_to_string(format!("{}/tests/fixtures/chats/{agent}/edits.recorded.jsonl", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let fixture = if agent == "claude-code" {
            fixture.replace(r#""name": "Edit""#, r#""name": "MultiEdit""#)
        } else {
            fixture.replace("@@ -1,3 +1,3 @@", "--- a/notes.txt\\n+++ b/notes.txt\\n@@ -1,3 +1,3 @@")
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("edits.jsonl");
        fs::write(&path, fixture).unwrap();
        let (_server, mut socket, chat_id) = launch(agent, path.to_str().unwrap()).await;
        let mut view = View::default();
        view.turn(&mut socket, &chat_id, prompt).await;
        let edits: Vec<_> = view.items.iter().filter(|item| item["kind"] == "edit").collect();
        assert_eq!(edits.len(), 3, "{agent}");
        let notes = edits.iter().find(|item| item["path"] == "notes.txt").unwrap();
        assert_eq!(notes["added"], 1, "{agent}");
        assert_eq!(notes["removed"], 1, "{agent}");
    }
}

#[tokio::test]
async fn api_errors_become_error_notices() {
    for (agent, error) in [
        ("claude-code", "There's an issue with the selected model (claude-nonexistent-9). It may not exist or you may not have access to it."),
        ("codex", "The 'gpt-nonexistent-9' model is not supported when using Codex with a ChatGPT account."),
    ] {
        let (_server, mut socket, chat_id) = launch(agent, "error").await;
        let mut view = View::default();
        view.turn(&mut socket, &chat_id, "Say hi.").await;
        assert_eq!(view.items, [user("Say hi."), json!({ "kind": "notice", "notice": "error", "text": error })], "{agent}");
    }
}

#[tokio::test]
async fn claude_subagent_activity_stays_nested_and_todos_replace_live_and_after_restart() {
    subagents("claude-code").await;
}

#[tokio::test]
async fn codex_subagent_activity_stays_nested_and_todos_replace_live_and_after_restart() {
    subagents("codex").await;
}

async fn subagents(agent: &str) {
    let (mut server, mut socket, chat_id) = launch(agent, "subagents").await;
    let mut view = View::default();
    view.turn(&mut socket, &chat_id, "Find needle using a subagent and track the steps.").await;
    assert_eq!(view.children.first().unwrap()["status"], "working");
    assert_eq!(view.children.first().unwrap()["items"], json!([]));
    assert!(view.children.iter().any(|child| child["status"] == "working" && child["items"].as_array().is_some_and(|items| !items.is_empty())));
    let index = view.items.iter().position(|item| item["kind"] == "subagent").unwrap();
    assert_eq!(view.items[index]["elided"], true);
    control(&mut socket, json!({ "type": "chat.expand", "chatId": chat_id, "index": index })).await;
    view.follow(&mut socket, &chat_id, |view| view.items[index].get("elided").is_none()).await;
    let child = view.items[index].clone();
    assert_eq!(child["status"], "done");
    assert!(child["id"].as_str().is_some_and(|id| !id.is_empty()));
    assert!(child["startedAt"].is_u64());
    let home = server.root.path().join("home").display().to_string();
    let finished = |child: &Value, restored: bool| {
        assert_eq!(child["outcome"], "completed");
        if agent == "claude-code" {
            assert_eq!(child["name"], "Explore");
            assert_eq!(child["task"], "Find file mentioning needle");
            assert_eq!(child["prompt"], CLAUDE_SUBAGENT_PROMPT.replace("/tmp/stampede-chat", &home));
            let report = "Only notes.txt contains it.\n\n/tmp/stampede-chat/notes.txt, line 2: `needle in a haystack`";
            assert_eq!(child["report"], report.replace("/tmp/stampede-chat", &home));
            assert_eq!(child["model"], "claude-opus-5-5");
            assert_eq!(child["totals"], json!({ "durationMs": 3456, "tools": 2, "tokens": 14662 }));
        } else {
            assert_eq!(child.get("name"), restored.then(|| json!("Heisenberg")).as_ref());
            assert_eq!(child["task"], "find_needle");
            assert_eq!(child.get("prompt"), None);
            assert_eq!(child["report"], "notes.txt mentions “needle”; todo.txt does not.");
            assert_eq!(child["model"], "gpt-6.1-sol");
            let mut totals = json!({ "durationMs": 8358, "tools": 3 });
            if !restored { totals["tokens"] = json!(66760); }
            assert_eq!(child["totals"], totals);
        }
    };
    finished(&child, false);
    let mut expected = if agent == "claude-code" {
        json!([{ "kind": "tools", "calls": [
            { "kind": "read", "name": "Read", "detail": "notes.txt" },
            { "kind": "read", "name": "Read", "detail": "todo.txt" },
        ] }])
    } else {
        json!([
            { "kind": "command", "command": "cat notes.txt todo.txt", "status": "ok", "output": "alpha\nneedle in a haystack\nomega\nbuy milk\nwalk the dog\n" },
            { "kind": "thinking", "seconds": 2 },
            { "kind": "command", "command": "cat notes.txt", "status": "ok", "output": "alpha\nneedle in a haystack\nomega\n" },
            { "kind": "command", "command": "cat todo.txt", "status": "ok", "output": "buy milk\nwalk the dog\n" },
        ])
    };
    assert_eq!(child["items"], expected);
    assert_eq!(view.items.iter().filter(|item| item["kind"] == "subagent").count(), 1);
    assert!(!view.items.iter().any(|item| item["kind"] == "text" && item["text"] == "notes.txt mentions “needle”; todo.txt does not."));
    assert!(!view.items.iter().any(|item| item["kind"] == "command" && item["command"].as_str().unwrap().contains("cat ")));
    assert!(!view.items.iter().any(|item| item["kind"] == "tools" && item["calls"].as_array().unwrap().iter().any(|call| call["detail"] == "todo.txt")));
    let labels = if agent == "claude-code" {
        ["Find which file mentions \"needle\"", "Count lines in matching file", "Answer"]
    } else {
        ["Find which file mentions \"needle\"", "Count that file's lines", "Answer"]
    };
    let snapshot = |statuses: &[&str]| json!(labels.iter().zip(statuses).map(|(text, status)| json!({ "text": text, "status": status })).collect::<Vec<_>>());
    let mut todos = if agent == "claude-code" { vec![snapshot(&["pending"]), snapshot(&["pending", "pending"]), snapshot(&["pending", "pending", "pending"])] } else { Vec::new() };
    todos.push(snapshot(&["active", "pending", "pending"]));
    if agent == "claude-code" { todos.push(snapshot(&["done", "pending", "pending"])); }
    todos.push(snapshot(&["done", "active", "pending"]));
    if agent == "claude-code" { todos.push(snapshot(&["done", "done", "pending"])); }
    todos.push(snapshot(&["done", "done", "active"]));
    todos.push(snapshot(&["done", "done", "done"]));
    assert_eq!(view.todos, todos);
    view.turn(&mut socket, &chat_id, DONE).await;
    let before = view.items;
    server.restart().await;
    let (mut socket, mut restored) = reattach(&server, &chat_id, before.len()).await;
    let restored_child = |view: &View| view.items.iter().find(|item| item["kind"] == "subagent").unwrap().clone();
    let index = restored.items.iter().position(|item| item["kind"] == "subagent").unwrap();
    control(&mut socket, json!({ "type": "chat.expand", "chatId": chat_id, "index": index })).await;
    if agent == "codex" {
        expected[1]["seconds"] = json!(0);
        restored.follow(&mut socket, &chat_id, |view| restored_child(view)["items"] == expected).await;
        assert_eq!(restored_child(&restored)["startedAt"], 1792186663000u64);
    } else {
        restored.follow(&mut socket, &chat_id, |view| restored_child(view).get("elided").is_none()).await;
        assert_eq!(restored_child(&restored)["items"], expected);
        assert_eq!(restored_child(&restored)["startedAt"], 1792186800832u64);
    }
    assert_eq!(restored_child(&restored)["id"], child["id"]);
    finished(&restored_child(&restored), true);
    if agent == "claude-code" {
        restored.turn(&mut socket, &chat_id, "Set the first task back to pending with your task tool, then say done in one word.").await;
        assert_eq!(restored.todos.last(), Some(&snapshot(&["pending", "done", "done"])));
    }
    let record: Value = serde_json::from_slice(&std::fs::read(server.root.path().join("data/chats").join(format!("{chat_id}.json"))).unwrap()).unwrap();
    let expected_status = if agent == "claude-code" { "pending" } else { "done" };
    assert_eq!(record["todos"], snapshot(&[expected_status, "done", "done"]));
}

fn user(text: &str) -> Value {
    json!({ "kind": "user", "text": text, "images": 0 })
}

fn edit(path: &str, diff: &str, added: usize, removed: usize) -> Value {
    json!({ "kind": "edit", "path": path, "diff": diff, "added": added, "removed": removed })
}

fn written(path: &str, diff: &str, added: usize) -> Value {
    json!({ "kind": "edit", "path": path, "diff": diff, "added": added, "removed": 0, "write": true })
}

fn text(text: &str) -> Value {
    json!({ "kind": "text", "text": text, "streaming": false, "phase": "final" })
}

fn said(text: &str) -> Value {
    json!({ "kind": "text", "text": text, "streaming": false, "phase": "commentary" })
}

async fn launch(agent: &str, fixture: &str) -> (Server, Socket, String) {
    launch_with_gates(agent, fixture, None).await
}

async fn launch_with_gates(agent: &str, fixture: &str, gates: Option<&std::path::Path>) -> (Server, Socket, String) {
    let config = format!(
        r#"
[[rig]]
name = "rig"
agent = "{agent}"
command = "{agent}"
env = {{ STAMPEDE_FAKE_FIXTURE = "{fixture}" }}
"#
    );
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let mut environment = vec![("PATH", path.into())];
    if let Some(gates) = gates {
        environment.push(("STAMPEDE_FAKE_GATES", gates.as_os_str().to_owned()));
    }
    let server = Server::with_env("chat-transcript", &config, environment).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "rig" })).await;
    let created = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.created"));
    let chat_id = created.await.expect("the chat was never created")["chat"]["id"].as_str().unwrap().to_owned();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    assert_eq!(next_control(&mut socket, "chat.items").await["items"], json!([]));
    (server, socket, chat_id)
}

async fn reattach(server: &Server, chat_id: &str, len: usize) -> (Socket, View) {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    let items = next_control(&mut socket, "chat.items").await["items"].as_array().unwrap().clone();
    let mut view = View { items, ..View::default() };
    view.follow(&mut socket, chat_id, |view| view.items.len() >= len).await;
    (socket, view)
}

impl View {
    async fn turn(&mut self, socket: &mut Socket, chat_id: &str, text: &str) {
        control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": text })).await;
        let (statuses, items) = (self.statuses.len(), self.items.len());
        self.follow(socket, chat_id, |view| {
            view.statuses.len() > statuses && view.statuses.last().is_some_and(|status| status == "done")
                && view.items[items..].iter().any(|item| item["kind"] == "user")
        }).await;
        assert_eq!(self.statuses[statuses], "working");
    }

    async fn follow(&mut self, socket: &mut Socket, chat_id: &str, until: impl Fn(&Self) -> bool) {
        timeout(Duration::from_secs(20), async {
            while !until(self) {
                let message = next(socket).await;
                if message["chatId"] != chat_id {
                    continue;
                }
                match message["type"].as_str().unwrap() {
                    "chat.status" => self.statuses.push(message["status"].clone()),
                    "chat.todos" => self.todos.push(message["todos"].clone()),
                    "chat.item" => {
                        if message["item"]["kind"] == "subagent" {
                            self.children.push(message["item"].clone());
                        }
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
