mod common;

use std::{env, fs, time::Duration};

use common::{Server, chat_turn, close_chat, control, leftovers, next_control};
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const CONFIG: &str = r#"
[[rig]]
name = "claude"
agent = "claude-code"
command = "claude"
env = { OIKONOMOS_FAKE_FIXTURE = "rig-fields" }
model = "sonnet"
effort = "low"
mode = "plan"

[[rig]]
name = "codex"
agent = "codex"
command = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "rig-fields" }
model = "gpt-6-luna"
effort = "low"
mode = "read-only"

[[rig]]
name = "claude tui"
agent = "claude-code"
command = "sh"
args = ["-c", "printf '%s\\n' \"$0\" \"$@\" > claude-argv; exec sleep 600"]
model = "opus"
effort = "xhigh"
mode = "acceptEdits"

[[rig]]
name = "codex tui"
agent = "codex"
command = "sh"
args = ["-c", "printf '%s\\n' \"$0\" \"$@\" > codex-argv; exec sleep 600"]
model = "gpt-6-luna"
effort = "low"
mode = "full-access"
"#;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn a_claude_chat_starts_with_its_rigs_model_effort_and_mode() {
    let (server, mut socket, project_id) = open().await;
    let chat = create(&mut socket, &project_id, "claude").await;
    let chat_id = chat["id"].as_str().unwrap();
    let settings = json!({ "model": "sonnet", "effort": "low", "mode": "plan" });
    assert_eq!(chat["settings"], settings);

    let prompt = "Reply with the single word: ready";
    let (statuses, items) = chat_turn(&mut socket, chat_id, &chat["status"], prompt).await;
    assert_eq!(statuses, ["done", "working", "done"]);
    assert_eq!(
        items,
        [
            json!({ "kind": "user", "text": prompt, "images": 0 }),
            json!({ "kind": "text", "text": "ready", "streaming": false }),
        ]
    );
    let record: Value = serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{chat_id}.json"))).unwrap()).unwrap();
    assert_eq!(record["settings"], settings);

    close_chat(&mut socket, server.root.path(), chat_id).await;
}

#[tokio::test]
async fn a_codex_chat_starts_its_thread_with_its_rigs_model_effort_and_mode() {
    let (server, mut socket, project_id) = open().await;
    let chat = create(&mut socket, &project_id, "codex").await;
    assert_eq!(chat["settings"], json!({ "model": "gpt-6-luna", "effort": "low", "mode": "read-only" }));

    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
    let log = fs::read_to_string(server.root.path().join("home/.codex/fake-daemon/received.jsonl")).unwrap();
    let received: Vec<Value> = log.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let methods: Vec<&Value> = received.iter().map(|message| &message["method"]).collect();
    assert_eq!(methods, ["initialize", "initialized", "thread/start", "thread/unsubscribe"]);
    assert_eq!(
        received[2]["params"],
        json!({
            "cwd": server.root.path().join("home"),
            "model": "gpt-6-luna",
            "config": { "model_reasoning_effort": "low" },
            "approvalPolicy": "on-request",
            "sandbox": "read-only",
        })
    );
}

#[tokio::test]
async fn a_tui_gets_its_rigs_model_effort_and_mode_as_flags() {
    let (server, mut socket, project_id) = open().await;
    let expected = [
        ("claude tui", "claude-argv", "--model opus --effort xhigh --permission-mode acceptEdits --settings"),
        ("codex tui", "codex-argv", "-m gpt-6-luna -c model_reasoning_effort=low -a never -s danger-full-access -c"),
    ];
    for (rig, file, flags) in expected {
        control(&mut socket, json!({ "type": "terminal.create", "projectId": project_id, "rig": rig })).await;
        let terminal_id = next_control(&mut socket, "terminal.created").await["terminal"]["id"].clone();
        let path = server.root.path().join("home").join(file);
        let argv = timeout(Duration::from_secs(10), async {
            loop {
                if let Ok(argv) = fs::read_to_string(&path)
                    && argv.ends_with('\n')
                {
                    break argv;
                }
                sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("{rig} never ran"));
        let words: Vec<&str> = argv.lines().collect();
        assert_eq!(words[..flags.split(' ').count()].join(" "), flags, "{rig}: {words:?}");
        control(&mut socket, json!({ "type": "terminal.close", "terminalId": terminal_id })).await;
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while !leftovers(server.root.path()).is_empty() {
        assert!(Instant::now() < deadline, "left running: {:?}", leftovers(server.root.path()));
        sleep(Duration::from_millis(50)).await;
    }
}

async fn open() -> (Server, Socket, Value) {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("rig-fields", CONFIG, vec![("PATH", path.into())]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    (server, socket, project_id)
}

async fn create(socket: &mut Socket, project_id: &Value, rig: &str) -> Value {
    control(socket, json!({ "type": "chat.create", "projectId": project_id, "rig": rig })).await;
    let created = timeout(Duration::from_secs(10), next_control(socket, "chat.created"));
    let chat = created.await.unwrap_or_else(|_| panic!("{rig}: the chat was never created"))["chat"].clone();
    control(socket, json!({ "type": "chat.attach", "chatId": chat["id"] })).await;
    assert_eq!(next_control(socket, "chat.items").await["items"], json!([]));
    chat
}
