mod common;

use std::{fs, process::Stdio};

use common::{Server, control, next_control};
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, process::Command};
use tokio_tungstenite::connect_async;

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

/// The status after each hook event of a fixture; `-` is no status, the Agent having exited.
const SEQUENCES: [(&str, &str, &str, &str); 7] = [
    ("claude", "claude-code", "simple-turn", "done working working working done"),
    ("claude", "claude-code", "permission", "done working working waiting waiting working done"),
    ("claude", "claude-code", "question", "done working working waiting waiting working done"),
    ("claude", "claude-code", "exit", "done working done done done done -"),
    ("codex", "codex", "simple-turn", "done working done"),
    ("codex", "codex", "approval", "working working waiting working done"),
    ("codex", "codex", "exit", "working done -"),
];

#[tokio::test]
async fn recorded_hook_sequences_drive_agent_status() {
    let server = Server::with_config("status", CONFIG).await;
    let url = format!("http://127.0.0.1:{}/api/hook", server.port);
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();

    for (rig, agent, fixture, expected) in SEQUENCES {
        let create = json!({ "type": "terminal.create", "projectId": project_id, "rig": rig });
        control(&mut socket, create).await;
        let created = next_control(&mut socket, "terminal.created").await;
        let terminal_id = created["terminal"]["id"].as_str().unwrap().to_owned();
        assert_eq!(created["terminal"]["status"], "done", "{agent} {fixture}: launch");

        let path = format!("{}/tests/fixtures/hooks/{agent}/{fixture}.recorded.jsonl", env!("CARGO_MANIFEST_DIR"));
        let events: Vec<(String, Value)> = fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter_map(|line| event(agent, serde_json::from_str(line).unwrap()))
            .collect();
        let expected: Vec<&str> = expected.split(' ').collect();
        assert_eq!(events.len(), expected.len(), "{agent} {fixture}: expectations");

        for ((event, payload), status) in events.iter().zip(expected) {
            let mut hook = Command::new(env!("CARGO_BIN_EXE_oikonomos"))
                .args(["hook", agent, event])
                .env("OIKONOMOS_TERMINAL_ID", &terminal_id)
                .env("OIKONOMOS_HOOK_URL", &url)
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

            control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
            let hello = next_control(&mut socket, "hello").await;
            let terminal = hello["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .find(|terminal| terminal["id"] == terminal_id)
                .unwrap();
            let context = format!("{agent} {fixture} after {event}: {terminal}");
            if status == "-" {
                assert!(terminal.get("status").is_none(), "{context}");
                assert!(terminal.get("rig").is_none(), "{context}");
                assert!(terminal.get("agentSession").is_none(), "{context}");
            } else {
                assert_eq!(terminal["status"], status, "{context}");
                assert_eq!(terminal["agentSession"], payload["session_id"], "{context}");
            }
        }
    }
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
