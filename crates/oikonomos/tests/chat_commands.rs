mod common;

use std::{env, ffi::OsString, fs, path::Path, time::Duration};

use common::{Server, close_chat, control, next, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::{sleep, timeout}};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const OK: &str = "Reply with the single word ok.";
const SKILL: &str = "/openai-docs Reply with the single word ok. Use no tools.";
const CLAUDE_MODES: &[[&str; 2]] = &[
    ["default", "default"],
    ["acceptEdits", "accept edits"],
    ["plan", "plan"],
    ["auto", "auto"],
    ["dontAsk", "dont ask"],
    ["bypassPermissions", "bypass permissions"],
];
const CODEX_MODES: &[[&str; 2]] =
    &[["read-only", "read only"], ["default", "default"], ["full-access", "full access"], ["plan", "plan"]];

const MANAGEMENT: &[&str] = &[
    "mcp", "hooks", "memory", "permissions", "plugin", "plugins", "statusline", "theme", "export", "add-dir",
    "agents", "login", "logout", "config", "heapdump",
];

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Default)]
struct View {
    settings: Vec<Value>,
    capabilities: Value,
    statuses: Vec<Value>,
    items: Vec<Value>,
}

#[tokio::test]
async fn claude_built_ins_reach_the_agent_as_control_requests_and_user_text() {
    let config = tempfile::tempdir().unwrap();
    let env = vec![("CLAUDE_CONFIG_DIR", config.path().into())];
    let directory = tempfile::tempdir().unwrap();
    let fixture = clear_fixture("claude-code", directory.path());
    let (server, mut socket, chat_id, mut view) = launch_fixture("claude-code", env, &fixture).await;
    view.follow(&mut socket, &chat_id, |view| view.setting("effort").is_some()).await;
    assert_eq!(view.last(), json!({ "model": "opus", "effort": "xhigh", "mode": "default", "pending": [] }));
    let initialize = &recorded("claude-code")[3]["stdout"]["response"]["response"];
    let commands: Vec<Value> = initialize["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|command| !MANAGEMENT.contains(&command["name"].as_str().unwrap()) && !command["name"].as_str().unwrap().starts_with("__"))
        .map(|command| {
            json!({ "name": command["name"], "description": command["description"], "hint": command["argumentHint"] })
        })
        .collect();
    let models: Vec<&Value> = initialize["models"].as_array().unwrap().iter().map(|model| &model["value"]).collect();
    assert_eq!(view.capabilities["commands"], json!(commands));
    assert!(view.capabilities["commands"].as_array().unwrap().iter().all(|command| {
        let name = command["name"].as_str().unwrap();
        !MANAGEMENT.contains(&name) && !name.starts_with("__")
    }));
    assert_eq!(view.capabilities["models"], json!(models));
    assert_eq!(view.capabilities["efforts"], json!(["low", "medium", "high", "xhigh", "max"]));
    assert_eq!(view.capabilities["modes"], modes(CLAUDE_MODES));

    for (command, value) in [("model", "sonnet"), ("effort", "low"), ("mode", "plan")] {
        let seen = view.settings.len();
        run(&mut socket, &chat_id, json!({ "command": command, "value": value })).await;
        let settled = |view: &View| view.settings.len() > seen && view.last()["pending"] == json!([]);
        view.follow(&mut socket, &chat_id, settled).await;
        assert_eq!(view.settings[seen][command], value);
        assert_eq!(view.settings[seen]["pending"], json!([command]), "{command} is pending until Claude answers");
    }
    assert_eq!(view.last(), json!({ "model": "sonnet", "effort": "low", "mode": "plan", "pending": [] }));

    view.turn(&mut socket, &chat_id, json!({ "type": "chat.send", "chatId": chat_id, "text": OK })).await;
    assert_eq!(
        view.items,
        [
            json!({ "kind": "user", "text": OK, "images": 0 }),
            json!({ "kind": "text", "text": "ok", "streaming": false })
        ]
    );
    assert_eq!(record(&server, &chat_id)["todos"], json!([{ "text": "Old session task", "status": "pending" }]));
    assert!(record(&server, &chat_id)["context"].is_object());
    assert!(record(&server, &chat_id)["tokens"].is_object());
    let first = record(&server, &chat_id)["agentSession"].clone();
    assert_eq!(first, "3aaa0747-0136-4d46-af51-0ec0de032562");
    view.turn(&mut socket, &chat_id, json!({ "type": "chat.run", "chatId": chat_id, "command": "compact" })).await;
    view.turn(&mut socket, &chat_id, json!({ "type": "chat.run", "chatId": chat_id, "command": "clear" })).await;
    assert_eq!(view.items, Vec::<Value>::new(), "/clear leaves an empty transcript");
    assert_eq!(record(&server, &chat_id)["agentSession"], "6fc0d30d-f1f5-4ff0-93f6-de0071507543");
    assert_eq!(record(&server, &chat_id)["context"], Value::Null);
    assert_eq!(record(&server, &chat_id)["tokens"], Value::Null);
    assert_eq!(record(&server, &chat_id)["todos"], json!([]));

    let received: Vec<Value> = fs::read_to_string(config.path().join("fake-received.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .map(|message| match message["type"].as_str() {
            Some("control_request") => message["request"].clone(),
            _ => message["message"]["content"].clone(),
        })
        .collect();
    assert_eq!(
        received,
        [
            json!({ "subtype": "initialize" }),
            json!({ "subtype": "get_settings" }),
            json!({ "subtype": "set_model", "model": "sonnet" }),
            json!({ "subtype": "apply_flag_settings", "settings": { "effortLevel": "low" } }),
            json!({ "subtype": "set_permission_mode", "mode": "plan" }),
            json!(OK),
            json!("/compact"),
            json!("/clear"),
        ]
    );
    close_chat(&mut socket, server.root.path(), &chat_id).await;
}

#[tokio::test]
async fn codex_built_ins_ride_the_next_turn_or_reach_the_daemon_as_requests() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = clear_fixture("codex", directory.path());
    let (server, mut socket, chat_id, mut view) = launch_fixture("codex", Vec::new(), &fixture).await;
    let declared = |view: &View| {
        view.setting("model").is_some()
            && view.capabilities["models"][0].is_string()
            && view.capabilities["commands"][0].is_object()
    };
    view.follow(&mut socket, &chat_id, declared).await;
    assert_eq!(view.last(), json!({ "model": "gpt-6.1-sol", "effort": "high", "mode": "read-only", "pending": [] }));
    let fixture = recorded("codex");
    let listed = |id| {
        let answer = fixture.iter().find(|entry| entry["stdout"]["id"] == id && entry["stdout"]["result"].is_object());
        &answer.unwrap()["stdout"]["result"]["data"]
    };
    let models: Vec<&Value> = listed(2).as_array().unwrap().iter().map(|model| &model["model"]).collect();
    let skills: Vec<&Value> = listed(3)[0]["skills"].as_array().unwrap().iter().map(|skill| &skill["name"]).collect();
    assert_eq!(view.capabilities["models"], json!(models));
    let commands = view.capabilities["commands"].as_array().unwrap();
    let commands: Vec<&Value> = commands.iter().map(|command| &command["name"]).collect();
    assert_eq!(commands, skills);
    assert_eq!(view.capabilities["efforts"], json!(["low", "medium", "high", "xhigh", "max", "ultra"]));
    assert_eq!(view.capabilities["modes"], modes(CODEX_MODES));

    for (command, value) in [("model", "gpt-6-luna"), ("effort", "low"), ("mode", "plan")] {
        let seen = view.settings.len();
        run(&mut socket, &chat_id, json!({ "command": command, "value": value })).await;
        view.follow(&mut socket, &chat_id, |view| view.settings.len() > seen).await;
    }
    assert_eq!(
        view.last(),
        json!({ "model": "gpt-6-luna", "effort": "low", "mode": "plan", "pending": ["model", "effort", "mode"] })
    );
    assert_eq!(view.capabilities["efforts"], json!(["low", "medium", "high", "xhigh", "max"]));
    assert_eq!(methods(&server), ["initialize", "initialized", "thread/start", "model/list", "skills/list"]);

    view.turn(&mut socket, &chat_id, json!({ "type": "chat.send", "chatId": chat_id, "text": OK })).await;
    assert_eq!(view.last()["pending"], json!([]), "the overrides went out with the turn");
    run(&mut socket, &chat_id, json!({ "command": "mode", "value": "default" })).await;
    view.follow(&mut socket, &chat_id, |view| view.last()["mode"] == "default").await;
    assert_eq!(view.last()["pending"], json!(["mode"]));
    view.turn(&mut socket, &chat_id, json!({ "type": "chat.send", "chatId": chat_id, "text": SKILL })).await;
    assert_eq!(view.items[2], json!({ "kind": "user", "text": SKILL, "images": 0 }));
    assert_eq!(record(&server, &chat_id)["todos"], json!([{ "text": "Old session task", "status": "pending" }]));
    assert!(record(&server, &chat_id)["context"].is_object());
    assert!(record(&server, &chat_id)["tokens"].is_object());
    view.turn(&mut socket, &chat_id, json!({ "type": "chat.run", "chatId": chat_id, "command": "compact" })).await;
    run(&mut socket, &chat_id, json!({ "command": "clear" })).await;
    view.follow(&mut socket, &chat_id, |view| view.items.is_empty() && view.last()["pending"] != json!([])).await;
    let collaboration = |mode| {
        let settings = json!({ "model": "gpt-6-luna", "reasoning_effort": "low", "developer_instructions": null });
        json!({ "mode": mode, "settings": settings })
    };
    let first = "01a14675-2f6f-7231-8ebf-ac1b6988103c";
    let second = "01a14675-45f7-75f1-b408-8825bb0affc5";
    let cwd = server.root.path().join("home");

    let log = received(&server);
    let methods: Vec<&Value> = log.iter().map(|message| &message["method"]).collect();
    assert_eq!(
        methods,
        [
            "initialize",
            "initialized",
            "thread/start",
            "model/list",
            "skills/list",
            "turn/start",
            "turn/start",
            "thread/compact/start",
            "thread/start",
            "thread/unsubscribe",
        ]
    );
    let params: Vec<&Value> = log.iter().map(|message| &message["params"]).collect();
    assert_eq!(
        params[5],
        &json!({
            "threadId": first,
            "input": [{ "type": "text", "text": OK, "text_elements": [] }],
            "model": "gpt-6-luna",
            "effort": "low",
            "collaborationMode": collaboration("plan"),
        })
    );
    let path = "/home/user/.codex/skills/.system/openai-docs/SKILL.md";
    let skill = json!({ "type": "skill", "name": "openai-docs", "path": path });
    assert_eq!(
        params[6],
        &json!({
            "threadId": first,
            "input": [{ "type": "text", "text": SKILL, "text_elements": [] }, skill],
            "model": "gpt-6-luna",
            "effort": "low",
            "collaborationMode": collaboration("default"),
            "approvalPolicy": "on-request",
            "sandboxPolicy": { "type": "workspaceWrite" },
        })
    );
    assert_eq!(params[7], &json!({ "threadId": first }));
    assert_eq!(params[8], &json!({ "cwd": cwd, "config": { "tools.update_plan": { "enabled": true } } }));
    assert_eq!(params[9], &json!({ "threadId": first }));
    assert_eq!(record(&server, &chat_id)["agentSession"], second);
    assert_eq!(record(&server, &chat_id)["context"], Value::Null);
    assert_eq!(record(&server, &chat_id)["tokens"], Value::Null);
    assert_eq!(record(&server, &chat_id)["todos"], json!([]));
    assert_eq!(
        view.last(),
        json!({ "model": "gpt-6-luna", "effort": "low", "mode": "default", "pending": ["model", "effort", "mode"] }),
        "the new thread starts on Codex's defaults, so the chosen settings wait for its first turn"
    );

    close_chat(&mut socket, server.root.path(), &chat_id).await;
    let closed = received(&server).pop().unwrap();
    assert_eq!((&closed["method"], &closed["params"]), (&json!("thread/unsubscribe"), &json!({ "threadId": second })));
}

#[tokio::test]
async fn a_codex_choice_made_while_a_turn_starts_stays_pending_for_the_next_turn() {
    let directory = tempfile::tempdir().unwrap();
    let mut entries = recorded("codex")[..43].to_vec();
    entries.insert(13, json!({ "gate": "starting" }));
    entries.push(json!({ "stdin": { "method": "thread/unsubscribe", "id": 90 } }));
    entries.push(json!({ "stdout": { "id": 90, "result": {} } }));
    let fixture = write_fixture(directory.path(), &entries);
    let env = vec![("OIKONOMOS_FAKE_GATES", directory.path().into())];
    let (server, mut socket, chat_id, mut view) = launch_fixture("codex", env, &fixture).await;
    view.follow(&mut socket, &chat_id, |view| view.capabilities["models"][0].is_string()).await;
    run(&mut socket, &chat_id, json!({ "command": "effort", "value": "low" })).await;
    view.follow(&mut socket, &chat_id, |view| view.last()["effort"] == "low").await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": OK })).await;
    gate(directory.path(), "starting").await;
    run(&mut socket, &chat_id, json!({ "command": "effort", "value": "high" })).await;
    view.follow(&mut socket, &chat_id, |view| view.last()["effort"] == "high").await;
    fs::remove_file(directory.path().join("starting")).unwrap();
    view.follow(&mut socket, &chat_id, |view| view.statuses.last().is_some_and(|status| status == "done")).await;
    assert_eq!(view.last()["pending"], json!(["effort"]));
    view.turn(&mut socket, &chat_id, json!({ "type": "chat.send", "chatId": chat_id, "text": SKILL })).await;
    assert_eq!(view.last()["effort"], "high");
    assert_eq!(view.last()["pending"], json!([]));
    assert_eq!(received(&server)[5]["params"]["effort"], "low");
    assert_eq!(received(&server)[6]["params"]["effort"], "high");
    close_chat(&mut socket, server.root.path(), &chat_id).await;
}

#[tokio::test]
async fn an_earlier_claude_setting_failure_preserves_a_newer_choice_and_its_pending_mark() {
    let directory = tempfile::tempdir().unwrap();
    let source = recorded("claude-code");
    let mut entries = source[..6].to_vec();
    let first = source[8].clone();
    let mut second = first.clone();
    second["stdin"]["request_id"] = json!("newer");
    second["stdin"]["request"]["settings"]["effortLevel"] = json!("high");
    entries.extend([first.clone(), second]);
    entries.push(json!({ "stdout": { "type": "control_response", "response": {
        "subtype": "error", "request_id": first["stdin"]["request_id"], "error": "Rejected old choice"
    } } }));
    entries.push(json!({ "stdout": { "type": "stream_event", "event": {
        "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "Old reply processed" }
    } } }));
    entries.push(json!({ "gate": "earlier" }));
    let mut answer = source[9].clone();
    answer["stdout"]["response"]["request_id"] = json!("newer");
    entries.push(answer);
    let fixture = write_fixture(directory.path(), &entries);
    let env = vec![("OIKONOMOS_FAKE_GATES", directory.path().into())];
    let (server, mut socket, chat_id, mut view) = launch_fixture("claude-code", env, &fixture).await;
    view.follow(&mut socket, &chat_id, |view| view.setting("effort").is_some()).await;
    for value in ["low", "high"] {
        run(&mut socket, &chat_id, json!({ "command": "effort", "value": value })).await;
        view.follow(&mut socket, &chat_id, |view| view.last()["effort"] == value).await;
    }
    gate(directory.path(), "earlier").await;
    view.follow(&mut socket, &chat_id, |view| !view.items.is_empty()).await;
    assert_eq!(view.last()["effort"], "high");
    assert_eq!(view.last()["pending"], json!(["effort"]));
    fs::remove_file(directory.path().join("earlier")).unwrap();
    view.follow(&mut socket, &chat_id, |view| view.last()["pending"] == json!([])).await;
    assert_eq!(view.last()["effort"], "high");
    close_chat(&mut socket, server.root.path(), &chat_id).await;
}

fn write_fixture(directory: &Path, entries: &[Value]) -> String {
    let path = directory.join("commands.jsonl");
    fs::write(&path, entries.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap();
    path.to_str().unwrap().to_owned()
}

async fn gate(directory: &Path, name: &str) {
    timeout(Duration::from_secs(10), async {
        while !directory.join(name).exists() {
            sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("the fake never reached its gate");
}

fn clear_fixture(agent: &str, directory: &Path) -> String {
    let mut entries = recorded(agent);
    let marker = if agent == "claude-code" {
        json!({ "stdout": { "type": "assistant", "message": { "content": [{
            "type": "tool_use", "id": "old-todos", "name": "TodoWrite", "input": { "todos": [
                { "content": "Old session task", "status": "pending", "activeForm": "Old session task" }
            ] }
        }] }, "parent_tool_use_id": null } })
    } else {
        json!({ "stdout": { "method": "turn/plan/updated", "params": {
            "threadId": "01a14675-2f6f-7231-8ebf-ac1b6988103c",
            "plan": [{ "step": "Old session task", "status": "pending" }]
        } } })
    };
    let index = entries.iter().position(|entry| {
        entry["stdout"]["type"] == "result" || entry["stdout"]["method"] == "turn/completed"
    }).unwrap();
    entries.insert(index, marker);
    write_fixture(directory, &entries)
}

async fn launch_fixture(agent: &str, env: Vec<(&'static str, OsString)>, fixture: &str) -> (Server, Socket, String, View) {
    let config = format!(
        r#"
[[rig]]
name = "agent"
agent = "{agent}"
command = "{agent}"
env = {{ OIKONOMOS_FAKE_FIXTURE = "{fixture}" }}
"#
    );
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let mut env = env;
    env.push(("PATH", path.into()));
    let server = Server::with_env("chat-commands", &config, env).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "agent" })).await;
    let mut view = View::default();
    let created = timeout(Duration::from_secs(10), async {
        loop {
            let message = next(&mut socket).await;
            if message["type"] == "chat.created" {
                break message;
            }
            view.absorb(&message);
        }
    });
    let chat_id = created.await.expect("the chat was never created")["chat"]["id"].as_str().unwrap().to_owned();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    (server, socket, chat_id, view)
}

async fn run(socket: &mut Socket, chat_id: &str, builtin: Value) {
    let mut message = json!({ "type": "chat.run", "chatId": chat_id });
    message.as_object_mut().unwrap().extend(builtin.as_object().unwrap().clone());
    control(socket, message).await;
}

impl View {
    fn last(&self) -> Value {
        self.settings.last().cloned().unwrap_or_default()
    }

    fn setting(&self, name: &str) -> Option<&str> {
        self.settings.last()?[name].as_str()
    }

    async fn turn(&mut self, socket: &mut Socket, chat_id: &str, message: Value) {
        let seen = self.statuses.len();
        control(socket, message).await;
        self.follow(socket, chat_id, |view| view.statuses[seen..] == ["working", "done"]).await;
    }

    async fn follow(&mut self, socket: &mut Socket, chat_id: &str, until: impl Fn(&Self) -> bool) {
        timeout(Duration::from_secs(20), async {
            while !until(self) {
                let message = next(socket).await;
                if message["chatId"] == chat_id {
                    self.absorb(&message);
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out with {:?} {:?} {:?}", self.statuses, self.settings.last(), self.items));
    }

    fn absorb(&mut self, message: &Value) {
        match message["type"].as_str().unwrap() {
            "chat.settings" => {
                let mut settings = message["settings"].clone();
                settings.as_object_mut().unwrap().entry("pending").or_insert(json!([]));
                self.settings.push(settings);
            }
            "chat.capabilities" => self.capabilities = message["capabilities"].clone(),
            "chat.status" => self.statuses.push(message["status"].clone()),
            "chat.items" => self.items = message["items"].as_array().unwrap().clone(),
            "chat.item" => {
                let index = message["index"].as_u64().unwrap() as usize;
                match self.items.get_mut(index) {
                    Some(item) => *item = message["item"].clone(),
                    None => self.items.push(message["item"].clone()),
                }
            }
            _ => {}
        }
    }
}

fn modes(modes: &[[&str; 2]]) -> Value {
    modes.iter().map(|[value, label]| json!({ "value": value, "label": label })).collect()
}

fn recorded(agent: &str) -> Vec<Value> {
    let fixture = format!("tests/fixtures/chats/{agent}/commands.recorded.jsonl");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
    fs::read_to_string(path).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

fn record(server: &Server, chat_id: &str) -> Value {
    serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{chat_id}.json"))).unwrap()).unwrap()
}

fn received(server: &Server) -> Vec<Value> {
    let log = fs::read_to_string(server.root.path().join("home/.codex/fake-daemon/received.jsonl")).unwrap();
    log.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

fn methods(server: &Server) -> Vec<Value> {
    received(server).into_iter().map(|message| message["method"].clone()).collect()
}
