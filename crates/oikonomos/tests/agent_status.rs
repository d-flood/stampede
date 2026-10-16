mod common;

use std::{env, fs, process::Stdio, time::Duration};

use common::{Server, control, next_control};
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, net::TcpStream, process::Command, time::{Instant, sleep}};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const CONFIG: &str = r#"
[[rig]]
name = "claude"
agent = "claude-code"
command = "sh"
args = ["-c", "exec sleep 600"]

[[rig]]
name = "codex"
agent = "codex"
command = "sh"
args = ["-c", "exec sleep 600"]
"#;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// The status after each hook event of a fixture; `-` is no status, the Agent having exited,
/// and `!` marks a Tab that needs attention.
const SEQUENCES: [(&str, &str, &str, &str); 7] = [
    ("claude", "claude-code", "simple-turn", "done working working working done!"),
    ("claude", "claude-code", "permission", "done working working waiting! waiting! working done!"),
    ("claude", "claude-code", "question", "done working working waiting! waiting! working done!"),
    ("claude", "claude-code", "exit", "done working done! done! done! done! -"),
    ("codex", "codex", "simple-turn", "done working done!"),
    ("codex", "codex", "approval", "working working waiting! working done!"),
    ("codex", "codex", "exit", "working done! -"),
];

#[tokio::test]
async fn recorded_hook_sequences_drive_agent_status() {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let mut server = Server::with_env("status", CONFIG, vec![("PATH", path.into())]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();

    for (rig, agent, fixture, sequence) in SEQUENCES {
        let create = json!({ "type": "terminal.create", "projectId": project_id, "rig": rig });
        control(&mut socket, create).await;
        let created = next_control(&mut socket, "terminal.created").await;
        let terminal_id = created["terminal"]["id"].as_str().unwrap().to_owned();
        assert_eq!(created["terminal"]["status"], "done", "{agent} {fixture}: launch");
        assert_eq!(created["terminal"]["needsAttention"], false, "{agent} {fixture}: launch");

        let path = format!("{}/tests/fixtures/hooks/{agent}/{fixture}.recorded.jsonl", env!("CARGO_MANIFEST_DIR"));
        let events: Vec<(String, Value)> = fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter_map(|line| event(agent, serde_json::from_str(line).unwrap()))
            .collect();
        let expected: Vec<&str> = sequence.split(' ').collect();
        assert_eq!(events.len(), expected.len(), "{agent} {fixture}: expectations");

        for ((event, payload), expected) in events.iter().zip(expected) {
            hook(&server, &terminal_id, agent, event, payload).await;
            let terminal = listed(&mut socket, &terminal_id).await;
            let context = format!("{agent} {fixture} after {event}: {terminal}");
            if expected == "-" {
                assert!(terminal.get("status").is_none(), "{context}");
                assert!(terminal.get("rig").is_none(), "{context}");
                assert!(terminal.get("agentSession").is_none(), "{context}");
                assert_eq!(terminal["needsAttention"], false, "{context}");
            } else {
                let status = expected.trim_end_matches('!');
                assert_eq!(terminal["status"], status, "{context}");
                assert_eq!(terminal["agentSession"], payload["session_id"], "{context}");
                assert_eq!(terminal["needsAttention"], expected.ends_with('!'), "{context}");
            }
        }
        if sequence.ends_with('-') {
            continue;
        }

        let turn: Vec<&(String, Value)> = ["UserPromptSubmit", "Stop"]
            .iter()
            .map(|name| events.iter().find(|(event, _)| event == name).unwrap())
            .collect();
        let view = |terminal_ids: &[&str]| json!({ "type": "device.view", "terminalIds": terminal_ids });
        control(&mut socket, view(&[&terminal_id])).await;
        let terminal = listed(&mut socket, &terminal_id).await;
        assert_eq!(terminal["needsAttention"], false, "{agent} {fixture}: viewed: {terminal}");

        for shown in [true, false] {
            if !shown {
                control(&mut socket, view(&[])).await;
            }
            for (event, payload) in &turn {
                hook(&server, &terminal_id, agent, event, payload).await;
                let terminal = listed(&mut socket, &terminal_id).await;
                let context = format!("{agent} {fixture} after {event}, shown {shown}: {terminal}");
                let done = event == "Stop";
                assert_eq!(terminal["status"], if done { "done" } else { "working" }, "{context}");
                assert_eq!(terminal["needsAttention"], done && !shown, "{context}");
            }
        }
    }

    control(&mut socket, json!({ "type": "terminal.create", "projectId": project_id, "rig": "claude" })).await;
    let created = next_control(&mut socket, "terminal.created").await;
    let terminal_id = created["terminal"]["id"].as_str().unwrap();
    let payload = json!({ "session_id": "outage-session" });
    hook(&server, terminal_id, "claude-code", "UserPromptSubmit", &payload).await;
    let record = server.root.path().join(format!("data/terminals/{terminal_id}.json"));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let saved: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
        if saved["status"] == "working" { break; }
        assert!(Instant::now() < deadline);
        sleep(Duration::from_millis(20)).await;
    }
    server.stop().await;
    hook(&server, terminal_id, "claude-code", "Stop", &payload).await;
    server.restart().await;
    let (mut reconnected, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    let terminal = listed(&mut reconnected, terminal_id).await;
    assert_eq!(terminal["status"], "done");
    assert_eq!(terminal["agentSession"], "outage-session");
    assert_eq!(terminal["needsAttention"], true);
}

async fn hook(server: &Server, terminal_id: &str, agent: &str, event: &str, payload: &Value) {
    let url = format!("http://127.0.0.1:{}/api/hook", server.port);
    let mut hook = Command::new(env!("CARGO_BIN_EXE_oikonomos"))
        .args(["hook", agent, event])
        .env("OIKONOMOS_TERMINAL_ID", terminal_id)
        .env("OIKONOMOS_HOOK_URL", url)
        .env("OIKONOMOS_DATA_DIR", server.root.path().join("data"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = hook.stdin.take().unwrap();
    stdin.write_all(payload.to_string().as_bytes()).await.unwrap();
    drop(stdin);
    let output = hook.wait_with_output().await.unwrap();
    assert!(output.status.success() && output.stdout.is_empty() && output.stderr.is_empty());
}

/// A hello reply comes after every earlier control message and hook has been applied.
async fn listed(socket: &mut Socket, terminal_id: &str) -> Value {
    control(socket, json!({ "type": "hello", "deviceId": "device" })).await;
    let hello = next_control(socket, "hello").await;
    let terminals = hello["terminals"].as_array().unwrap();
    terminals.iter().find(|terminal| terminal["id"] == terminal_id).unwrap().clone()
}

/// Codex fixtures also record notify and OSC signals, which Rigs don't wire.
fn event(agent: &str, mut line: Value) -> Option<(String, Value)> {
    let event = line["event"].as_str()?.to_owned();
    match agent {
        "codex" if line["source"] != "hook" => None,
        "codex" => Some((event, line["payload"].take())),
        _ => {
            line.as_object_mut()?.remove("event");
            Some((event, line))
        }
    }
}
