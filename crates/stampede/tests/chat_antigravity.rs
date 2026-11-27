use crate::common;

use std::{env, fs, time::Duration};

use common::{Server, apply, chat_turn, close_chat, control, next, next_control};
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    time::{sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const AGY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake/agy");

async fn start(config: &str) -> (Server, Socket, Value) {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("antigravity", config, vec![("PATH", path.into())]).await;
    let mut socket = device(server.port).await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project = next_control(&mut socket, "project.opened").await["projectId"].clone();
    (server, socket, project)
}

async fn device(port: u16) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    socket
}

async fn open(socket: &mut Socket, project: &Value) -> Value {
    control(socket, json!({ "type": "chat.create", "projectId": project, "rig": "Antigravity" })).await;
    let chat = timeout(Duration::from_secs(20), next_control(socket, "chat.created")).await.expect("the Chat was not created")["chat"].clone();
    assert_eq!(chat["agent"], "antigravity");
    chat
}

fn rig(fixture: &str, extra: &str) -> String {
    format!("[[rig]]\nname = 'Antigravity'\nagent = 'antigravity'\n{extra}env = {{ STAMPEDE_FAKE_FIXTURE = '{fixture}', AGY_CLI_PATH = '{AGY}' }}\n")
}

fn received(server: &Server) -> Vec<Value> {
    fs::read_to_string(server.root.path().join("agy/received.jsonl")).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

/// Attaches to the Chat and returns its items, requests and capabilities.
async fn attached(socket: &mut Socket, chat: &Value) -> Value {
    control(socket, json!({ "type": "chat.attach", "chatId": chat["id"] })).await;
    let mut attached = next_control(socket, "chat.items").await;
    let mut requests = Vec::new();
    loop {
        let message = next(socket).await;
        match message["type"].as_str() {
            Some("chat.request") => requests.push(message["request"].clone()),
            Some("chat.capabilities") => { attached["capabilities"] = message["capabilities"].clone(); break }
            _ => {}
        }
    }
    attached["requests"] = json!(requests);
    attached
}

async fn done(socket: &mut Socket) {
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next(socket).await;
            if message["type"] == "chat.status" && message["status"] == "done" { break; }
        }
    }).await.unwrap();
}

/// The Antigravity Readings, without their read times, from the first `usage.changed` whose Gemini group used `gemini`.
async fn quota(socket: &mut Socket, gemini: f64) -> Value {
    loop {
        let message = next_control(socket, "usage.changed").await;
        let mut readings: Vec<Value> = message["readings"].as_array().unwrap().iter().filter(|reading| reading["agent"] == "antigravity").cloned().collect();
        if readings.iter().any(|reading| reading["group"]["short"] == "gemini" && reading["used"] == json!(gemini)) {
            readings.iter_mut().for_each(|reading| { reading.as_object_mut().unwrap().remove("readAt"); });
            return json!(readings);
        }
    }
}

async fn options(server: &Server, rig: &str) -> reqwest::Response {
    reqwest::get(format!("http://127.0.0.1:{}/api/rigs/options?agent=antigravity&rig={rig}", server.port)).await.unwrap()
}

#[tokio::test]
async fn antigravity_maps_updates_settings_and_permissions() {
    let (server, mut socket, project) = start(&rig("tools", "model = 'claude-opus-4-6-thinking'\neffort = 'low'\n")).await;
    let listed: Value = serde_json::from_str(&options(&server, "Antigravity").await.text().await.unwrap()).unwrap();
    assert_eq!(listed["models"][0], json!({ "value": "gemini-3.8-flash", "label": "Gemini 3.8 Flash", "efforts": ["low", "medium", "high"], "speeds": [], "ultracode": false }));
    assert_eq!(listed["modes"].as_array().unwrap().iter().map(|mode| mode["value"].as_str().unwrap()).collect::<Vec<_>>(), ["default", "accept-edits", "plan", "full-access"]);
    assert_eq!((&listed["settings"]["model"], &listed["settings"]["effort"]), (&json!("claude-opus-4-6-thinking"), &json!("low")));
    let chat = open(&mut socket, &project).await;
    let id = chat["id"].as_str().unwrap();
    let opened = attached(&mut socket, &chat).await;
    assert_eq!(opened["capabilities"]["forks"], false);
    let models = if !opened["capabilities"]["models"].as_array().unwrap().is_empty() {
        opened["capabilities"]["models"].clone()
    } else {
        timeout(Duration::from_secs(10), async {
            loop {
                let message = next_control(&mut socket, "chat.capabilities").await;
                if message["chatId"] == id && !message["capabilities"]["models"].as_array().unwrap().is_empty() {
                    break message["capabilities"]["models"].clone();
                }
            }
        }).await.expect("capabilities never arrived")
    };
    assert_eq!(models, json!(["gemini-3.8-flash", "claude-opus-4-6-thinking"]));
    for allow in [true, false] {
        control(&mut socket, json!({ "type": "chat.send", "chatId": id, "text": "Run the probe" })).await;
        let request = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.request")).await.unwrap()["request"].clone();
        assert_eq!((&request["kind"], &request["words"], &request["allowRules"]), (&json!("command"), &json!([["echo", "stampede-probe"]]), &json!(false)));
        control(&mut socket, json!({ "type": "chat.answer", "chatId": id, "requestId": request["id"], "answer": { "allow": allow } })).await;
        done(&mut socket).await;
    }
    let transcript = attached(&mut socket, &chat).await;
    assert_eq!(transcript["requests"], json!([]));
    assert_eq!(transcript["capabilities"]["commands"], json!([{ "name": "review", "description": "Review the changes", "hint": "path" }]));
    let items = transcript["items"].as_array().unwrap();
    assert_eq!(items[0], json!({ "kind": "user", "text": "Run the probe", "images": 0 }));
    assert_eq!(items[1]["text"], "Run the probe first.");
    assert_eq!(items[2], json!({ "kind": "command", "command": "echo stampede-probe", "status": "ok", "output": "Command exited with code 0.\n\nCommand output:\nstampede-probe\r\n" }));
    assert_eq!((&items[3]["kind"], &items[3]["path"], &items[3]["added"], &items[3]["removed"]), (&json!("edit"), &json!("file.txt"), &json!(1), &json!(1)));
    assert_eq!(items[4], json!({ "kind": "tools", "calls": [{ "kind": "read", "name": "Read README.md", "detail": "README.md" }] }));
    assert_eq!(items[5], json!({ "kind": "text", "text": "done", "streaming": false, "phase": "final" }));
    assert_eq!(items[8]["status"], "failed");
    let record: Value = serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{id}.json"))).unwrap()).unwrap();
    assert_eq!(record["title"], "Shell Command Execution Test");
    assert_eq!(record["context"], json!({ "used": 9123, "window": 1048576 }));
    assert_eq!(record["tokens"], json!({ "input": 22272, "output": 320 }));
    close_chat(&mut socket, server.root.path(), id).await;
    let received = received(&server);
    let method = |name: &'static str| received.iter().filter(move |message| message["method"] == name);
    let initialize = &method("initialize").next().unwrap()["params"];
    assert_eq!((&initialize["protocolVersion"], &initialize["clientInfo"]["name"], &initialize["clientCapabilities"]["terminal"]), (&json!(1), &json!("Stampede"), &json!(false)));
    let servers = &method("session/new").last().unwrap()["params"]["mcpServers"];
    assert_eq!((&servers[0]["name"], &servers[0]["args"][0]), (&json!("stampede_chat"), &json!("chat-tools")));
    assert_eq!(method("session/set_config_option").map(|message| (message["params"]["configId"].clone(), message["params"]["value"].clone())).collect::<Vec<_>>(),
        [(json!("model"), json!("claude-opus-4-6-thinking")), (json!("thought_level"), json!("low"))]);
    assert!(method("session/prompt").all(|message| message["params"]["prompt"] == json!([{ "type": "text", "text": "Run the probe" }])));
    let answers: Vec<_> = received.iter().filter(|message| message["id"].as_str().is_some_and(|id| id.starts_with("permission-"))).map(|message| message["result"].clone()).collect();
    assert_eq!(answers, [json!({ "outcome": { "outcome": "selected", "optionId": "allow" } }), json!({ "outcome": { "outcome": "selected", "optionId": "deny" } })]);
    assert_eq!(method("authenticate").count(), 0);
}

#[tokio::test]
async fn antigravity_reopens_history_after_restart_and_lists_project_sessions() {
    let (mut server, mut socket, project) = start(&rig("simple-turn", "")).await;
    let chat = open(&mut socket, &project).await;
    let id = chat["id"].as_str().unwrap();
    attached(&mut socket, &chat).await;
    let (_, transcript) = chat_turn(&mut socket, id, &chat["status"], "Hello").await;
    assert_eq!(transcript, [
        json!({ "kind": "user", "text": "Hello", "images": 0 }),
        json!({ "kind": "text", "text": "Welcome—**glad** you’re here.", "streaming": false, "phase": "final" }),
    ]);
    server.restart().await;
    let mut socket = device(server.port).await;
    assert_eq!(attached(&mut socket, &chat).await["items"], json!(transcript));
    control(&mut socket, json!({ "type": "chat.send", "chatId": id, "text": "Again" })).await;
    done(&mut socket).await;
    assert_eq!(attached(&mut socket, &chat).await["items"].as_array().unwrap().len(), 4);
    let record: Value = serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{id}.json"))).unwrap()).unwrap();
    assert_eq!(record["tokens"], json!({ "input": 22272, "output": 320 }));
    let session = received(&server).iter().find(|message| message["method"] == "session/load").expect("the Chat never loaded its session")["params"]["sessionId"].clone();
    close_chat(&mut socket, server.root.path(), id).await;
    control(&mut socket, json!({ "type": "sessions.list", "projectId": project })).await;
    let sessions = timeout(Duration::from_secs(15), async {
        loop {
            let listed = next_control(&mut socket, "sessions.listed").await["sessions"].clone();
            if listed.as_array().unwrap().iter().any(|row| row["key"]["agent"] == "antigravity") { break listed; }
        }
    }).await.unwrap();
    let listed: Vec<_> = sessions.as_array().unwrap().iter().filter(|session| session["key"]["agent"] == "antigravity").collect();
    assert_eq!(listed.len(), 1, "{sessions}");
    assert_eq!((&listed[0]["key"]["sessionId"], &listed[0]["title"], &listed[0]["updatedAt"]), (&session, &json!("Shell Command Execution Test"), &json!(1795066939000_u64)));
    assert_eq!(listed[0]["key"]["store"], json!(server.root.path().join("home/.gemini/antigravity-cli")));
    control(&mut socket, json!({ "type": "sessions.search", "searchId": "agy-reply", "projectId": project, "query": "glad" })).await;
    let found = timeout(Duration::from_secs(15), async {
        loop {
            let message = next_control(&mut socket, "sessions.found").await;
            if message["searchId"] == "agy-reply" && !message["results"].as_array().unwrap().is_empty() { break message; }
        }
    }).await.unwrap();
    assert_eq!(found["results"][0]["key"], listed[0]["key"]);
    let calls = received(&server);
    assert!(calls.iter().any(|call| call["method"] == "session/list" && call["params"]["cwd"].is_null() && call["params"]["cursor"] == "more"));
    assert_eq!(calls.iter().filter(|call| call["method"] == "session/prompt").count(), 2);
    control(&mut socket, json!({ "type": "chat.resume", "projectId": project, "key": listed[0]["key"] })).await;
    let resumed = timeout(Duration::from_secs(20), next_control(&mut socket, "chat.created")).await.unwrap()["chat"].clone();
    assert_eq!(attached(&mut socket, &resumed).await["items"].as_array().unwrap().len(), 4);
    close_chat(&mut socket, server.root.path(), resumed["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn antigravity_stays_working_when_a_setting_before_an_early_prompt_fails() {
    let (server, mut socket, project) = start(&rig("simple-turn", "")).await;
    let chat = open(&mut socket, &project).await;
    let mut items = attached(&mut socket, &chat).await["items"].as_array().unwrap().clone();
    control(&mut socket, json!({ "type": "chat.run", "chatId": chat["id"], "command": "model", "value": "unlisted" })).await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat["id"], "text": "Hello" })).await;
    let mut working = false;
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next(&mut socket).await;
            match message["type"].as_str() {
                Some("chat.item" | "chat.append") => apply(&mut items, &message),
                Some("chat.status") if message["status"] == "done" && working => break,
                Some("chat.status") => working = message["status"] == "working",
                _ => {}
            }
        }
    }).await.unwrap();
    assert!(items.iter().any(|item| item["text"] == "Welcome—**glad** you’re here."), "{items:?}");
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn antigravity_explains_missing_sign_in_and_install() {
    let missing = "[[rig]]\nname = 'Missing'\nagent = 'antigravity'\nenv = { AGY_CLI_PATH = '/nonexistent/agy' }\n";
    let (server, mut socket, project) = start(&format!("{}{missing}", rig("unauthenticated", ""))).await;
    let sign_in = "Google Antigravity isn't signed in. Run `agy` in a Terminal and sign in with Google.";
    let chat = open(&mut socket, &project).await;
    let opened = attached(&mut socket, &chat).await;
    let notice = json!({ "kind": "notice", "notice": "error", "text": sign_in });
    if opened["items"] != json!([notice]) {
        let item = timeout(Duration::from_secs(10), async {
            loop {
                let message = next_control(&mut socket, "chat.item").await;
                if message["chatId"] == chat["id"] { break message["item"].clone(); }
            }
        }).await.expect("sign-in error never arrived");
        assert_eq!(item, notice);
    }
    let response = options(&server, "Antigravity").await;
    assert_eq!(response.status(), 502);
    assert_eq!(response.text().await.unwrap(), sign_in);
    assert!(options(&server, "Missing").await.text().await.unwrap().contains("`agy`, isn't installed"));
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
    assert!(!received(&server).iter().any(|message| message["method"] == "authenticate"));
}

#[tokio::test]
async fn antigravity_quota_reads_each_group_window() {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("antigravity", &rig("simple-turn", ""), vec![("PATH", path.into())]).await;
    let reading = |window: &str, name: &str, short: &str, used: f64, resets_at: u64| json!({ "agent": "antigravity", "window": window, "group": { "name": name, "short": short }, "used": used, "resetsAt": resets_at });
    let five_hour = |name: &str, short: &str| reading("five-hour", name, short, 0.9, 1795084935000);
    let argon = reading("weekly", "Gemini 4 Argon", "argon", 0.0, 1795673400000);
    let mut socket = device(server.port).await;
    assert_eq!(timeout(Duration::from_secs(20), quota(&mut socket, 1.0 - 0.9892)).await.expect("no Antigravity usage"), json!([
        reading("weekly", "Gemini Models", "gemini", 1.0 - 0.9892, 1795671735000),
        five_hour("Gemini Models", "gemini"),
        reading("weekly", "Claude and GPT models", "other", 0.0, 1795672609000),
        five_hour("Claude and GPT models", "other"),
        argon,
    ]));
    drop(socket);
    let later = timeout(Duration::from_secs(20), async {
        loop {
            // Usage is read again only when a Device connects after none were.
            sleep(Duration::from_millis(200)).await;
            let mut socket = device(server.port).await;
            if let Ok(readings) = timeout(Duration::from_secs(2), quota(&mut socket, 0.25)).await { break readings; }
        }
    }).await.expect("a later read never arrived");
    assert_eq!(later, json!([
        reading("weekly", "Gemini Models", "gemini", 0.25, 1795671735000),
        five_hour("Gemini Models", "gemini"),
        reading("weekly", "Claude and GPT models", "other", 0.5, 1795672609000),
        five_hour("Claude and GPT models", "other"),
        argon,
    ]));
}
