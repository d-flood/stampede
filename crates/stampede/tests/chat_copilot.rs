mod common;

use std::{env, fs, time::Duration};

use common::{Server, chat_turn, close_chat, control, next, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn device(port: u16) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "usage-watcher" })).await;
    next_control(&mut socket, "hello").await;
    socket
}

async fn billing_usage(socket: &mut Socket) -> Value {
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next_control(socket, "usage.changed").await;
            if let Some(reading) = message["readings"].as_array().unwrap().iter().find(|reading| reading["agent"] == "copilot") {
                return reading.clone();
            }
        }
    }).await.expect("Copilot billing usage never arrived")
}

#[tokio::test]
async fn copilot_polls_billing_period_consumption_without_a_model_call() {
    for fixture in ["simple-turn", "usage-zero", "usage-requests", "usage-unavailable", "usage-missing"] {
        let config = format!("[[rig]]\nname = 'Copilot'\nagent = 'copilot'\nenv = {{ STAMPEDE_FAKE_FIXTURE = '{fixture}' }}\n");
        let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
        let server = Server::with_env("copilot-usage", &config, vec![("PATH", path.into())]).await;
        let mut socket = device(server.port).await;
        let reading = billing_usage(&mut socket).await;
        assert_eq!(reading["window"], "monthly");
        assert!(reading.get("used").is_none());
        if fixture == "usage-unavailable" || fixture == "usage-missing" {
            assert!(reading.get("consumption").is_none());
        } else {
            assert_eq!(reading["consumption"], json!({ "amount": if fixture == "usage-zero" { 0.0 } else { 1234.5 }, "unit": if fixture == "usage-requests" { "requests" } else { "credits" } }));
        }
        if fixture != "usage-missing" {
            assert_eq!(reading["startsAt"], 1790812800000_u64);
            assert_eq!(reading["resetsAt"], 1793491200000_u64);
        }
        let calls = received(&server);
        assert!(calls.iter().any(|call| call["method"] == "session.quota.refresh"));
        assert!(!calls.iter().any(|call| call["method"] == "session.send"));
    }
}

#[tokio::test]
async fn copilot_updates_billing_consumption_and_retains_it_for_new_devices() {
    let (server, mut socket, chat) = launch("tools", "default").await;
    let mut watcher = device(server.port).await;
    assert_eq!(billing_usage(&mut watcher).await["consumption"]["amount"], 1234.5);
    chat_turn(&mut socket, chat["id"].as_str().unwrap(), &chat["status"], "Use tools").await;
    let updated = billing_usage(&mut watcher).await;
    assert_eq!(updated["consumption"], json!({ "amount": 1250.75, "unit": "credits" }));
    assert_eq!(updated["window"], "monthly");
    assert_eq!(updated["resetsAt"], 1793491200000_u64);
    assert_eq!(billing_usage(&mut device(server.port).await).await, updated);
}

async fn launch(fixture: &str, mode: &str) -> (Server, Socket, Value) {
    let config = format!("[[rig]]\nname = 'Copilot'\nagent = 'copilot'\nmode = '{mode}'\nenv = {{ STAMPEDE_FAKE_FIXTURE = '{fixture}' }}\n");
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("copilot", &config, vec![("PATH", path.into())]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project, "rig": "Copilot" })).await;
    let chat = timeout(Duration::from_secs(20), next_control(&mut socket, "chat.created")).await.expect("Copilot chat was not created")["chat"].clone();
    assert_eq!(chat["agent"], "copilot");
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat["id"] })).await;
    next_control(&mut socket, "chat.items").await;
    (server, socket, chat)
}

fn received(server: &Server) -> Vec<Value> {
    fs::read_to_string(server.root.path().join("copilot/received.jsonl")).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

async fn done(socket: &mut Socket) {
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next(socket).await;
            if message["type"] == "chat.status" && message["status"] == "done" { break; }
        }
    }).await.unwrap();
}

async fn items(socket: &mut Socket, chat: &Value) -> Value {
    control(socket, json!({ "type": "chat.attach", "chatId": chat["id"] })).await;
    let mut attached = next_control(socket, "chat.items").await;
    let mut requests = Vec::new();
    loop {
        let message = next(socket).await;
        match message["type"].as_str() {
            Some("chat.request") => requests.push(message["request"].clone()),
            Some("chat.capabilities") => break,
            _ => {},
        }
    }
    attached["requests"] = json!(requests);
    attached
}

#[tokio::test]
async fn copilot_streams_using_existing_cli_auth_and_closes_its_process() {
    let (server, mut socket, chat) = launch("simple-turn", "default").await;
    let id = chat["id"].as_str().unwrap();
    let (statuses, transcript) = chat_turn(&mut socket, id, &chat["status"], "Hello").await;
    assert_eq!(statuses, ["done", "working", "done"]);
    assert_eq!(transcript, [
        json!({ "kind": "user", "text": "Hello", "images": 0 }),
        json!({ "kind": "text", "text": "Welcome—**glad** you’re here.", "streaming": false, "phase": "final" }),
    ]);
    assert_eq!(items(&mut socket, &chat).await["items"], json!(transcript));
    close_chat(&mut socket, server.root.path(), id).await;
    let received = received(&server);
    let create = received.iter().find(|message| message["method"] == "session.create" && message["params"]["streaming"] == true).unwrap();
    assert_eq!(create["params"]["streaming"], true);
    assert_eq!(create["params"]["requestPermission"], true);
    assert_eq!(create["params"]["workingDirectory"], server.root.path().join("home").display().to_string());
    assert!(create["params"]["mcpServers"].is_object());
    assert!(received.iter().any(|message| message["method"] == "session.detach"));
}

#[tokio::test]
async fn copilot_permissions_wait_for_allow_or_deny() {
    for allow in [true, false] {
        let (server, mut socket, chat) = launch("approval", "default").await;
        control(&mut socket, json!({ "type": "chat.send", "chatId": chat["id"], "text": "Check git" })).await;
        let request = next_control(&mut socket, "chat.request").await["request"].clone();
        assert_eq!(request["kind"], "command");
        assert_eq!(request["allowRules"], false);
        assert_eq!(request["words"], json!([["git", "status"]]));
        control(&mut socket, json!({ "type": "chat.answer", "chatId": chat["id"], "requestId": request["id"], "answer": { "allow": allow } })).await;
        done(&mut socket).await;
        let transcript = items(&mut socket, &chat).await;
        assert_eq!(transcript["requests"], json!([]));
        assert_eq!(transcript["items"][1]["status"], if allow { "ok" } else { "failed" });
        assert_eq!(transcript["items"][2]["text"], if allow { "Approved." } else { "Denied." });
        close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
    }
}

#[tokio::test]
async fn copilot_full_access_applies_the_selected_permission_policy() {
    let (server, mut socket, chat) = launch("approval", "full-access").await;
    let (_, transcript) = chat_turn(&mut socket, chat["id"].as_str().unwrap(), &chat["status"], "Check git").await;
    assert_eq!(transcript[1]["status"], "ok");
    assert_eq!(transcript[2]["text"], "Approved.");
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn copilot_maps_commands_edits_and_context_usage() {
    let (server, mut socket, chat) = launch("tools", "default").await;
    let (_, transcript) = chat_turn(&mut socket, chat["id"].as_str().unwrap(), &chat["status"], "Use tools").await;
    assert_eq!(transcript[1], json!({ "kind": "command", "command": "git status", "status": "ok", "output": "clean" }));
    assert_eq!(transcript[2]["kind"], "edit");
    assert_eq!(transcript[2]["path"], "file.txt");
    assert_eq!(transcript[2]["added"], 1);
    assert_eq!(transcript[2]["removed"], 1);
    let record: Value = serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{}.json", chat["id"].as_str().unwrap()))).unwrap()).unwrap();
    assert_eq!(record["tokens"], json!({ "input": 120, "output": 25 }));
    assert_eq!(record["context"], json!({ "used": 145, "window": 200000 }));
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn copilot_restores_history_after_a_server_restart() {
    let (mut server, mut socket, chat) = launch("simple-turn", "default").await;
    let (_, transcript) = chat_turn(&mut socket, chat["id"].as_str().unwrap(), &chat["status"], "Hello").await;
    server.restart().await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    assert_eq!(items(&mut socket, &chat).await["items"], json!(transcript));
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat["id"], "text": "Again" })).await;
    done(&mut socket).await;
    assert_eq!(items(&mut socket, &chat).await["items"].as_array().unwrap().len(), 4);
    assert!(received(&server).iter().any(|message| message["method"] == "session.resume"));
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn copilot_interrupts_streaming_and_pending_permissions() {
    for fixture in ["interrupt", "approval"] {
        let (server, mut socket, chat) = launch(fixture, "default").await;
        control(&mut socket, json!({ "type": "chat.send", "chatId": chat["id"], "text": "Work" })).await;
        if fixture == "approval" { next_control(&mut socket, "chat.request").await; }
        else {
            timeout(Duration::from_secs(10), async { while next(&mut socket).await["item"]["text"] != "Still working" {} }).await.unwrap();
        }
        control(&mut socket, json!({ "type": "chat.interrupt", "chatId": chat["id"] })).await;
        done(&mut socket).await;
        let transcript = items(&mut socket, &chat).await;
        assert_eq!(transcript["requests"], json!([]));
        assert!(transcript["items"].as_array().unwrap().iter().any(|item| item["notice"] == "interrupted"));
        close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
    }
}

#[tokio::test]
async fn copilot_sends_images_and_reports_missing_login() {
    let (mut server, mut socket, chat) = launch("simple-turn", "default").await;
    let image = json!({ "mediaType": "image/png", "data": "aW1hZ2U=" });
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat["id"], "text": "Describe", "images": [image] })).await;
    done(&mut socket).await;
    let sent = received(&server).into_iter().find(|message| message["method"] == "session.send").unwrap();
    assert_eq!(sent["params"]["attachments"][0]["type"], "blob");
    assert_eq!(sent["params"]["attachments"][0]["mimeType"], "image/png");
    assert_eq!(items(&mut socket, &chat).await["items"][0]["images"], 1);
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
    let config_path = server.root.path().join("config.toml");
    let config = fs::read_to_string(&config_path).unwrap().replace("simple-turn", "unauthenticated");
    fs::write(config_path, config).unwrap();
    server.restart().await;
    let response = reqwest::Client::new().get(format!("http://127.0.0.1:{}/api/rigs/options?agent=copilot&rig=Copilot", server.port)).send().await.unwrap();
    assert_eq!(response.status(), 502);
    assert!(response.text().await.unwrap().contains("copilot login"));
}

#[tokio::test]
async fn copilot_questions_return_the_user_selection() {
    let (server, mut socket, chat) = launch("question", "default").await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat["id"], "text": "Choose a format" })).await;
    let request = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.request")).await.unwrap()["request"].clone();
    assert_eq!(request["questions"][0]["question"], "Which format?");
    control(&mut socket, json!({ "type": "chat.answer", "chatId": chat["id"], "requestId": request["id"], "answer": { "answers": [["CSV"]] } })).await;
    done(&mut socket).await;
    assert_eq!(items(&mut socket, &chat).await["items"][1]["text"], "Selected format.");
    let answer = received(&server).into_iter().find(|message| message["id"] == 900001).unwrap();
    assert_eq!(answer["result"], json!({ "answer": "CSV", "wasFreeform": false }));
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn copilot_lists_models_and_saved_sessions() {
    let (server, mut socket, chat) = launch("sessions", "default").await;
    let response = reqwest::Client::new().get(format!("http://127.0.0.1:{}/api/rigs/options?agent=copilot&rig=Copilot", server.port)).send().await.unwrap();
    assert!(response.status().is_success(), "{}", response.text().await.unwrap());
    let response: Value = serde_json::from_str(&response.text().await.unwrap()).unwrap();
    assert_eq!(response["models"][0]["value"], "claude-sonnet-4.6");
    assert_eq!(response["models"][0]["efforts"], json!(["low", "medium", "high"]));
    assert_eq!(response["settings"]["model"], "claude-sonnet-4.6");
    control(&mut socket, json!({ "type": "sessions.list", "projectId": chat["projectId"] })).await;
    let sessions = timeout(Duration::from_secs(15), next_control(&mut socket, "sessions.listed")).await.unwrap()["sessions"].clone();
    let saved = sessions.as_array().unwrap().iter().find(|session| session["agent"] == "copilot").unwrap();
    assert_eq!(saved["sessionId"], "saved-copilot-session");
    assert_eq!(saved["firstPrompt"], "A saved Copilot conversation");
    assert_eq!(saved["updatedAt"], 1792411200000_u64);
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn copilot_settings_compaction_rewind_and_clear_use_sdk_operations() {
    let (server, mut socket, chat) = launch("simple-turn", "default").await;
    let id = chat["id"].as_str().unwrap();
    chat_turn(&mut socket, id, &chat["status"], "Hello").await;
    control(&mut socket, json!({ "type": "chat.run", "chatId": id, "command": "effort", "value": "high" })).await;
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next(&mut socket).await;
            if message["type"] == "chat.settings" && message["settings"]["effort"] == "high" { break; }
        }
    }).await.unwrap();
    control(&mut socket, json!({ "type": "chat.run", "chatId": id, "command": "compact" })).await;
    timeout(Duration::from_secs(10), async { while next(&mut socket).await["item"]["notice"] != "compacted" {} }).await.unwrap();
    control(&mut socket, json!({ "type": "chat.run", "chatId": id, "command": "rewind", "value": 0 })).await;
    timeout(Duration::from_secs(10), next_control(&mut socket, "chat.items")).await.unwrap();
    assert_eq!(items(&mut socket, &chat).await["items"], json!([]));
    control(&mut socket, json!({ "type": "chat.send", "chatId": id, "text": "Again" })).await;
    done(&mut socket).await;
    let before: Value = serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{id}.json"))).unwrap()).unwrap();
    control(&mut socket, json!({ "type": "chat.run", "chatId": id, "command": "clear" })).await;
    timeout(Duration::from_secs(10), next_control(&mut socket, "chat.items")).await.unwrap();
    assert_eq!(items(&mut socket, &chat).await["items"], json!([]));
    let after: Value = serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{id}.json"))).unwrap()).unwrap();
    assert_ne!(before["agentSession"], after["agentSession"]);
    close_chat(&mut socket, server.root.path(), id).await;
}

#[tokio::test]
async fn copilot_uses_the_fresh_session_catalog_in_rigs_and_chat_models() {
    let (server, mut socket, chat) = launch("simple-turn", "default").await;
    let capabilities = next_control(&mut socket, "chat.capabilities").await["capabilities"].clone();
    let response = reqwest::Client::new().get(format!("http://127.0.0.1:{}/api/rigs/options?agent=copilot&rig=Copilot", server.port)).send().await.unwrap();
    let response: Value = serde_json::from_str(&response.text().await.unwrap()).unwrap();
    let models: Vec<&Value> = response["models"].as_array().unwrap().iter().map(|model| &model["value"]).collect();
    assert_eq!(models, [&json!("claude-sonnet-4.6"), &json!("gemini-3.8-flash")]);
    assert_eq!(response["models"][1]["efforts"], json!(["low", "high"]));
    assert_eq!(capabilities["models"], json!(["claude-sonnet-4.6", "gemini-3.8-flash"]));
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}
