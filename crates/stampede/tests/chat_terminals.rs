mod common;

use std::{env, fs, process::Stdio, time::Duration};

use common::{Server, View, control, next_control};
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, net::TcpStream, process::Command, time::{sleep, timeout}};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn launch() -> (Server, Socket, Value) {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("chat-terminals", "[[rig]]\nname = \"source\"\nagent = \"claude-code\"\n", vec![
        ("PATH", path.into()), ("STAMPEDE_FAKE_FIXTURE", "simple-turn".into()),
    ]).await;
    let mut socket = connect(server.port).await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project = next_control(&mut socket, "project.opened").await["projectId"].clone();
    let chat = create_chat(&mut socket, &project).await;
    (server, socket, chat)
}

async fn connect(port: u16) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    next_control(&mut socket, "hello").await;
    socket
}

async fn create_chat(socket: &mut Socket, project: &Value) -> Value {
    control(socket, json!({ "type": "chat.create", "projectId": project, "rig": "source" })).await;
    next_control(socket, "chat.created").await["chat"].clone()
}

async fn call(server: &Server, chat: &Value, name: &str, arguments: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stampede"))
        .arg("chat-tools").arg("--data-dir").arg(server.root.path().join("data"))
        .arg("--chat-id").arg(chat["id"].as_str().unwrap())
        .stdin(Stdio::piped()).stdout(Stdio::piped()).kill_on_drop(true).spawn().unwrap();
    child.stdin.take().unwrap().write_all(format!("{}\n", json!({
        "id": 1, "method": "tools/call", "params": { "name": name, "arguments": arguments },
    })).as_bytes()).await.unwrap();
    let output = timeout(Duration::from_secs(10), child.wait_with_output()).await.unwrap().unwrap();
    assert!(output.status.success());
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"].clone()
}

async fn read(server: &Server, chat: &Value, id: &str, lines: usize) -> Value {
    let result = call(server, chat, "read_terminal", json!({ "terminal_id": id, "lines": lines })).await;
    assert_eq!(result["isError"], false, "{result}");
    result["structuredContent"].clone()
}

#[tokio::test]
async fn staged_commands_wait_for_user_submission_and_survive_restart() {
    let (mut server, mut socket, chat) = launch().await;
    let command = "printf 'unreviewed\\n' > result\nprintf 'second line\\n' >> result\n";
    let staged = call(&server, &chat, "stage_command", json!({ "command": command })).await;
    assert_eq!(staged["isError"], false, "{staged}");
    assert_eq!(staged["structuredContent"]["cwd"], chat["cwd"]);
    let id = staged["structuredContent"]["terminal_id"].as_str().unwrap();
    let changed = next_control(&mut socket, "terminals.changed").await;
    assert_eq!(changed["terminals"][0]["pending"], command);
    assert_eq!(changed["projectId"], chat["projectId"]);
    let mut view = View::open(server.port, "observer", Some(id)).await;
    view.settle(|text| text.contains('$')).await;
    drop(view);
    assert!(!server.root.path().join("home/result").exists());
    let pending = read(&server, &chat, id, 200).await;
    assert_eq!(pending["pending"], true);
    assert!(!pending["output"].as_str().unwrap().contains("unreviewed"));

    server.restart().await;
    socket = connect(server.port).await;
    control(&mut socket, json!({ "type": "hello", "deviceId": "test" })).await;
    let hello = next_control(&mut socket, "hello").await;
    assert_eq!(hello["terminals"][0]["pending"], command);
    assert_eq!(read(&server, &chat, id, 200).await["pending"], true);
    assert!(!server.root.path().join("home/result").exists());

    let reviewed = "printf 'reviewed\\n' >> result\r";
    for _ in 0..2 {
        control(&mut socket, json!({ "type": "terminal.submit", "terminalId": id, "text": reviewed })).await;
    }
    timeout(Duration::from_secs(5), async {
        while fs::read_to_string(server.root.path().join("home/result")).unwrap_or_default() != "reviewed\n" {
            sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    assert_eq!(read(&server, &chat, id, 200).await["pending"], false);
    server.restart().await;
    assert_eq!(read(&server, &chat, id, 200).await["pending"], false);
    assert_eq!(fs::read_to_string(server.root.path().join("home/result")).unwrap(), "reviewed\n");
}

#[tokio::test]
async fn terminal_tools_read_hidden_scrollback_and_reject_unrelated_or_invalid_requests() {
    let (server, mut socket, chat) = launch().await;
    for arguments in [json!({ "command": "" }), json!({ "command": "echo hi", "run": true }),
        json!({ "command": "echo\u{001b}[31m" }), json!({ "command": "x".repeat(65537) })] {
        assert_eq!(call(&server, &chat, "stage_command", arguments).await["isError"], true);
    }
    let staged = call(&server, &chat, "stage_command", json!({ "command": "sudo example" })).await;
    let id = staged["structuredContent"]["terminal_id"].as_str().unwrap();
    let other = create_chat(&mut socket, &chat["projectId"]).await;
    assert_eq!(call(&server, &other, "read_terminal", json!({ "terminal_id": id })).await["isError"], true);
    assert_eq!(call(&server, &json!({ "id": "missing" }), "stage_command", json!({ "command": "echo hi" })).await["isError"], true);
    for arguments in [json!({ "terminal_id": id, "lines": 0 }), json!({ "terminal_id": id, "lines": 2001 }),
        json!({ "terminal_id": "../../config" }), json!({ "terminal_id": id, "extra": true })] {
        assert_eq!(call(&server, &chat, "read_terminal", arguments).await["isError"], true);
    }
    let mut view = View::open(server.port, "observer", Some(id)).await;
    view.settle(|text| text.contains('$')).await;
    view.send("printf '\\033[31m'; seq 1 100; printf '\\033[0mcomplete\\n'\r").await;
    view.settle(|text| text.contains("\ncomplete\n")).await;
    drop(view);
    let full = read(&server, &chat, id, 200).await;
    let output = full["output"].as_str().unwrap();
    assert!(output.contains("\n1\n2\n"), "{output}");
    assert!(output.contains("\n99\n100\ncomplete\n"), "{output}");
    assert!(!output.contains('\u{001b}'), "{output}");
    assert_eq!(full["truncated"], false);
    let latest = read(&server, &chat, id, 3).await;
    assert_eq!(latest["truncated"], true);
    assert!(latest["output"].as_str().unwrap().lines().count() <= 3);
    control(&mut socket, json!({ "type": "terminal.close", "terminalId": id })).await;
    timeout(Duration::from_secs(5), async {
        while server.root.path().join(format!("data/terminals/{id}.staged")).exists() {
            sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    assert_eq!(call(&server, &chat, "read_terminal", json!({ "terminal_id": id })).await["isError"], true);
}
