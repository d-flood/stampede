use crate::common;

use std::{env, fs, path::PathBuf, process::Command, time::Duration};

use common::{Server, control, next_control};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const GRACE: Duration = Duration::from_secs(2);

#[tokio::test]
async fn a_type_error_arrives_over_the_channel() {
    let (server, project) = start("lsp-pipe").await;
    let (mut socket, project_id) = open(&server, &project).await;
    let channel = format!("lsp:{project_id}:typescript");
    switch(&mut socket, &project_id, true).await;
    control(&mut socket, json!({ "type": "lsp.open", "channel": channel })).await;
    assert_eq!(state(&mut socket, &channel).await, "starting");
    assert_eq!(state(&mut socket, &channel).await, "on");

    let root = format!("file://{}", project.display());
    let initialize = json!({ "processId": null, "rootUri": root, "capabilities": { "textDocument": { "diagnostic": {} } } });
    request(&mut socket, &channel, 1, "initialize", initialize).await;
    notify(&mut socket, &channel, "initialized", json!({})).await;
    let document = json!({ "uri": format!("{root}/broken.ts"), "languageId": "typescript", "version": 0, "text": BROKEN });
    notify(&mut socket, &channel, "textDocument/didOpen", json!({ "textDocument": document })).await;
    let report =
        request(&mut socket, &channel, 2, "textDocument/diagnostic", json!({ "textDocument": { "uri": format!("{root}/broken.ts") } }))
            .await;

    let messages: Vec<_> = report["items"].as_array().unwrap().iter().map(|item| &item["message"]).collect();
    assert_eq!(messages, ["Type 'string' is not assignable to type 'number'."]);
}

#[tokio::test]
async fn a_server_starts_only_for_its_language_and_stops_after_the_grace() {
    let (server, project) = start("lsp-lifecycle").await;
    let (mut socket, project_id) = open(&server, &project).await;
    let channel = format!("lsp:{project_id}:typescript");

    control(&mut socket, json!({ "type": "lsp.open", "channel": channel })).await;
    switch(&mut socket, &project_id, true).await;
    control(&mut socket, json!({ "type": "lsp.open", "channel": format!("lsp:{project_id}:markdown") })).await;
    sleep(Duration::from_millis(500)).await;
    assert!(!running(&server), "a server started without an editor showing its language");

    control(&mut socket, json!({ "type": "lsp.open", "channel": channel })).await;
    assert_eq!(state(&mut socket, &channel).await, "starting");
    assert_eq!(state(&mut socket, &channel).await, "on");
    assert!(running(&server));

    control(&mut socket, json!({ "type": "lsp.close", "channel": channel })).await;
    sleep(GRACE / 2).await;
    control(&mut socket, json!({ "type": "lsp.open", "channel": channel })).await;
    sleep(GRACE).await;
    assert!(running(&server), "the server stopped though an editor reopened within the grace");

    control(&mut socket, json!({ "type": "lsp.close", "channel": channel })).await;
    let closed = Instant::now();
    assert_eq!(state(&mut socket, &channel).await, "off");
    assert!(closed.elapsed() >= GRACE, "the server stopped before the grace ended");
    stopped(&server).await;

    control(&mut socket, json!({ "type": "lsp.open", "channel": channel })).await;
    assert_eq!(state(&mut socket, &channel).await, "starting");
    assert_eq!(state(&mut socket, &channel).await, "on");
    switch(&mut socket, &project_id, false).await;
    assert_eq!(state(&mut socket, &channel).await, "off");
    stopped(&server).await;

    switch(&mut socket, &project_id, true).await;
    control(&mut socket, json!({ "type": "lsp.open", "channel": channel })).await;
    assert_eq!(state(&mut socket, &channel).await, "starting");
    assert_eq!(state(&mut socket, &channel).await, "on");
    control(&mut socket, json!({ "type": "project.close", "projectId": project_id })).await;
    assert_eq!(state(&mut socket, &channel).await, "off");
    stopped(&server).await;
}

const BROKEN: &str = "const n: number = \"x\";\n";

async fn start(prefix: &str) -> (Server, PathBuf) {
    let cache = env::var_os("STAMPEDE_LSP_CACHE")
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lsp-cache").into());
    let grace = GRACE.as_millis().to_string().into();
    let server = Server::with_env(prefix, "", vec![("STAMPEDE_LSP_CACHE", cache), ("STAMPEDE_LSP_GRACE_MS", grace)]).await;
    let project = server.root.path().join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("broken.ts"), BROKEN).unwrap();
    let project = fs::canonicalize(project).unwrap();
    (server, project)
}

async fn open(server: &Server, project: &PathBuf) -> (Socket, String) {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "lsp" })).await;
    control(&mut socket, json!({ "type": "project.open", "path": project })).await;
    let opened = next_control(&mut socket, "project.opened").await;
    (socket, opened["projectId"].as_str().unwrap().to_owned())
}

async fn switch(socket: &mut Socket, project_id: &str, on: bool) {
    control(socket, json!({ "type": "lsp.switch", "projectId": project_id, "on": on })).await;
    loop {
        let switched = next_control(socket, "lsp.switch").await;
        if switched["projectId"] == project_id && switched["on"] == on {
            return;
        }
    }
}

async fn state(socket: &mut Socket, channel: &str) -> String {
    loop {
        let message = next(socket, |message| message["type"] == "lsp.state").await;
        if message["channel"] == channel {
            return message["state"].as_str().unwrap().to_owned();
        }
    }
}

async fn notify(socket: &mut Socket, channel: &str, method: &str, params: Value) {
    let message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
    control(socket, json!({ "type": "lsp", "channel": channel, "message": message.to_string() })).await;
}

async fn request(socket: &mut Socket, channel: &str, id: u64, method: &str, params: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    control(socket, json!({ "type": "lsp", "channel": channel, "message": message.to_string() })).await;
    loop {
        let envelope = next(socket, |message| message["type"] == "lsp" && message["channel"] == channel).await;
        let response: Value = serde_json::from_str(envelope["message"].as_str().unwrap()).unwrap();
        if response.get("id").is_some() && response.get("method").is_some() {
            let reply = json!({ "jsonrpc": "2.0", "id": response["id"], "result": null });
            control(socket, json!({ "type": "lsp", "channel": channel, "message": reply.to_string() })).await;
        } else if response["id"] == id {
            assert!(response.get("error").is_none(), "{method} failed: {response}");
            return response["result"].clone();
        }
    }
}

async fn next(socket: &mut Socket, wanted: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let message = timeout(deadline.saturating_duration_since(Instant::now()), socket.next()).await.expect("timed out").unwrap().unwrap();
        if let Message::Text(text) = message {
            let message: Value = serde_json::from_str(&text).unwrap();
            if wanted(&message) {
                return message;
            }
        }
    }
}

fn running(server: &Server) -> bool {
    let program = server.root.path().join("data/language-servers/typescript");
    Command::new("pgrep").arg("-f").arg(program).output().unwrap().status.success()
}

async fn stopped(server: &Server) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while running(server) {
        assert!(Instant::now() < deadline, "the server is still running");
        sleep(Duration::from_millis(50)).await;
    }
}
