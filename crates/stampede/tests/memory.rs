mod common;

use std::{env, fs, io::Write, iter, path::Path, time::Duration};

use common::{Server, View, apply, control, expand, next, next_control};
use serde_json::{Value, json};
use tempfile::NamedTempFile;
use tokio::{
    net::{TcpSocket, TcpStream},
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, client_async, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const CONFIG: &str = r#"
[[rig]]
name = "claude"
agent = "claude-code"
command = "claude"
env = { STAMPEDE_FAKE_FIXTURE = "PATH_TO_FIXTURE" }
"#;

fn recorded(fixture: &str) -> Vec<Value> {
    let path = format!("{}/tests/fixtures/chats/claude-code/{fixture}.recorded.jsonl", env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(path).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

fn written(entries: &[Value]) -> NamedTempFile {
    let fixture = NamedTempFile::new().unwrap();
    fs::write(&fixture, entries.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap();
    fixture
}

/// The simple turn, with `events` streamed inside its text block.
fn turn(events: impl Iterator<Item = Value>) -> NamedTempFile {
    let mut entries = recorded("simple-turn");
    let at = entries.iter().position(|entry| entry["stdout"]["event"]["type"] == "content_block_delta").unwrap();
    let streamed: Vec<Value> = events.map(|event| {
        let mut entry = entries[at].clone();
        entry["stdout"]["event"] = event;
        entry
    }).collect();
    entries.splice(at + 1..at + 1, streamed);
    written(&entries)
}

/// The simple turn, with a Bash call before its result whose output is `output`.
fn command_turn(output: &str) -> NamedTempFile {
    let approval = recorded("approval");
    let call = approval.iter().find(|entry| entry["stdout"]["type"] == "assistant" && entry["stdout"]["message"]["content"][0]["name"] == "Bash").unwrap();
    let mut result = approval.iter().find(|entry| entry["stdout"]["type"] == "user" && entry["stdout"]["message"]["content"][0]["type"] == "tool_result").unwrap().clone();
    result["stdout"]["message"]["content"][0]["content"] = json!(output);
    result["stdout"]["tool_use_result"]["stdout"] = json!(output);
    let mut entries = recorded("simple-turn");
    let at = entries.iter().position(|entry| entry["stdout"]["type"] == "result").unwrap();
    entries.splice(at..at, [call.clone(), result]);
    written(&entries)
}

async fn launch(fixture: &str) -> Server {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    Server::with_env("memory", &CONFIG.replace("PATH_TO_FIXTURE", fixture), vec![("PATH", path.into())]).await
}

async fn finish_turn(socket: &mut Socket, chat_id: &Value) {
    control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Greet the reader." })).await;
    timeout(Duration::from_secs(60), async {
        for status in ["working", "done"] {
            while next_control(socket, "chat.status").await["status"] != status {}
        }
    })
    .await
    .expect("the turn never ended");
}

async fn device(port: u16, id: &str) -> Socket {
    let (socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    hello(socket, id).await
}

/// A Device whose socket buffers little, so the server's own queue takes what it does not read; and its local port.
async fn unbuffered_device(port: u16, id: &str) -> (Socket, u16) {
    let socket = TcpSocket::new_v4().unwrap();
    socket.set_recv_buffer_size(4096).unwrap();
    let stream = socket.connect(([127, 0, 0, 1], port).into()).await.unwrap();
    let local = stream.local_addr().unwrap().port();
    let (socket, _) = client_async(format!("ws://127.0.0.1:{port}/ws"), MaybeTlsStream::Plain(stream)).await.unwrap();
    (hello(socket, id).await, local)
}

/// Whether the server's end of the connection from `client` is no longer established.
fn closed_by_server(port: u16, client: u16) -> bool {
    let (server, client) = (format!("0100007F:{port:04X}"), format!("0100007F:{client:04X}"));
    !fs::read_to_string("/proc/net/tcp").unwrap().lines().skip(1).any(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        fields[1] == server && fields[2] == client && fields[3] == "01"
    })
}

async fn hello(mut socket: Socket, id: &str) -> Socket {
    control(&mut socket, json!({ "type": "hello", "deviceId": id })).await;
    next_control(&mut socket, "hello").await;
    socket
}

async fn attached_chat(socket: &mut Socket) -> Value {
    control(socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(socket, "project.opened").await["projectId"].clone();
    control(socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "claude" })).await;
    let chat_id = next_control(socket, "chat.created").await["chat"]["id"].clone();
    control(socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    next_control(socket, "chat.items").await;
    chat_id
}

fn resident_bytes(pid: u32) -> u64 {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    let line = status.lines().find(|line| line.starts_with("VmRSS:")).unwrap();
    line.split_whitespace().nth(1).unwrap().parse::<u64>().unwrap() * 1024
}

fn holder(root: &Path, terminal_id: &str) -> u32 {
    let data_dir = root.join("data");
    fs::read_dir("/proc").unwrap().flatten().find_map(|entry| {
        let pid = entry.file_name().to_str()?.parse().ok()?;
        let cmdline = String::from_utf8_lossy(&fs::read(entry.path().join("cmdline")).ok()?).replace('\0', " ");
        (cmdline.contains(&format!(" hold --data-dir {}", data_dir.display())) && cmdline.contains(&format!("--id {terminal_id} "))).then_some(pid)
    }).expect("the terminal has no holder")
}

#[tokio::test]
async fn a_device_that_stops_reading_is_cut_off() {
    let block = "x".repeat(256 << 10);
    let fixture = turn((0..160).flat_map(|_| [
        json!({ "type": "content_block_start", "index": 1, "content_block": { "type": "text", "text": block } }),
        json!({ "type": "content_block_stop", "index": 1 }),
    ]));
    let server = launch(fixture.path().to_str().unwrap()).await;
    let mut watcher = device(server.port, "watcher").await;
    let (mut stalled, local) = unbuffered_device(server.port, "stalled").await;
    let chat_id = attached_chat(&mut stalled).await;
    control(&mut stalled, json!({ "type": "chat.send", "chatId": chat_id, "text": "Greet the reader." })).await;

    timeout(Duration::from_secs(60), async {
        for status in ["working", "done"] {
            while next_control(&mut watcher, "chat.status").await["status"] != status {}
        }
    })
    .await
    .expect("the flooding turn never ended");

    let deadline = Instant::now() + Duration::from_secs(30);
    while !closed_by_server(server.port, local) {
        assert!(Instant::now() < deadline, "the server kept the stalled Device connected");
        sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn streamed_text_reaches_a_device_as_appends() {
    let delta = json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "x".repeat(4096) } });
    let fixture = turn(iter::repeat_n(delta, 400));
    let server = launch(fixture.path().to_str().unwrap()).await;
    let mut socket = device(server.port, "reader").await;
    let chat_id = attached_chat(&mut socket).await;
    control(&mut socket, json!({ "type": "chat.send", "chatId": chat_id, "text": "Greet the reader." })).await;

    let (mut items, mut received, mut working) = (Vec::new(), 0, false);
    timeout(Duration::from_secs(60), async {
        loop {
            let message = next(&mut socket).await;
            match message["type"].as_str().unwrap() {
                "chat.item" | "chat.append" => {
                    received += message.to_string().len();
                    apply(&mut items, &message);
                }
                "chat.status" if message["status"] == "working" => working = true,
                "chat.status" if message["status"] == "done" && working => break,
                _ => {}
            }
        }
    })
    .await
    .expect("the streaming turn never ended");

    let text = items.iter().find(|item| item["kind"] == "text").unwrap()["text"].as_str().unwrap();
    assert_eq!(text.matches('x').count(), 400 * 4096, "the appended text lost or doubled deltas");
    assert!(received < 4 * text.len(), "the Device received {received} bytes for {} bytes of text", text.len());
}

#[tokio::test]
async fn a_terminal_flood_stays_bounded_while_a_device_stops_reading() {
    let server = Server::start("resync").await;
    let mut view = View::open(server.port, "slow", None).await;
    view.settle(|text| text.contains('$')).await;
    view.send(&format!("yes {}\r", "0123456789".repeat(8))).await;
    let holder = holder(server.root.path(), &view.terminal_id);

    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        for (name, pid) in [("server", server.pid()), ("holder", holder)] {
            let resident = resident_bytes(pid);
            assert!(resident < 128 << 20, "the {name} grew to {} MiB while the Device stopped reading", resident >> 20);
        }
        sleep(Duration::from_millis(100)).await;
    }

    view.send("\x03").await;
    view.send("echo caught-up-$((6*7))\r").await;
    view.settle(|text| text.contains("caught-up-42")).await;
}

#[tokio::test]
async fn replaying_a_long_session_leaves_the_server_near_its_idle_size() {
    let mut server = launch("tools").await;
    let mut socket = device(server.port, "device").await;
    let chat_id = attached_chat(&mut socket).await;
    finish_turn(&mut socket, &chat_id).await;
    let idle = resident_bytes(server.pid());
    let projects = server.root.path().join("home/.claude/projects");
    let session = fs::read_dir(&projects).unwrap().flatten().flat_map(|project| fs::read_dir(project.path()).unwrap().flatten())
        .map(|entry| entry.path()).find(|path| path.extension().is_some_and(|extension| extension == "jsonl")).unwrap();
    let filler = json!({ "type": "progress", "isSidechain": true, "data": "x".repeat(100 << 10) }).to_string() + "\n";
    let mut appended = fs::OpenOptions::new().append(true).open(&session).unwrap();
    for _ in 0..400 { appended.write_all(filler.as_bytes()).unwrap(); }

    server.restart().await;
    device(server.port, "device").await;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let resident = resident_bytes(server.pid());
        if resident < idle + (8 << 20) { break; }
        assert!(Instant::now() < deadline, "replaying 40 MiB left the server at {} MiB, idle {} MiB", resident >> 20, idle >> 20);
        sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn a_long_command_output_reaches_a_device_once_it_expands_the_command() {
    let output = "0123456789abcdef\n".repeat(4096);
    let fixture = command_turn(&output);
    let server = launch(fixture.path().to_str().unwrap()).await;
    let mut socket = device(server.port, "device").await;
    let chat_id = attached_chat(&mut socket).await;
    finish_turn(&mut socket, &chat_id).await;

    let mut other = device(server.port, "other").await;
    control(&mut other, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    let items = next_control(&mut other, "chat.items").await["items"].as_array().unwrap().clone();
    let index = items.iter().position(|item| item["kind"] == "command").unwrap();
    assert_eq!(items[index]["output"], "");
    assert_eq!(items[index]["elided"], true);
    let expanded = expand(&mut other, chat_id.as_str().unwrap(), index).await;
    assert_eq!(expanded["output"], output);
    assert_eq!(expanded.get("elided"), None);
}
