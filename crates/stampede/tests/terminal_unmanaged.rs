use crate::common;

use std::fs;

use common::{Server, View, control, next_control};
use serde_json::{Value, json};
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn a_manually_started_tui_stays_an_ordinary_terminal_across_server_restart() {
    let mut server = Server::with_config("unmanaged", "[[rig]]\nname = 'Claude'\nagent = 'claude-code'\nenv = { RIG_ONLY = 'present' }\n").await;
    let mut view = View::open(server.port, "device", None).await;
    view.settle(|text| text.contains('$')).await;
    view.send("sh -c 'printf \"manual-tui:%s:%s\\n\" \"${RIG_ONLY-unset}\" \"${STAMPEDE_HOOK_URL-unset}\"; read -r _'\r").await;
    view.settle(|text| text.lines().any(|line| line == "manual-tui:unset:unset")).await;

    server.restart().await;
    let mut restored = View::open(server.port, "device", Some(&view.terminal_id)).await;
    restored.settle(|text| text.lines().any(|line| line == "manual-tui:unset:unset")).await;
    let record: Value = serde_json::from_slice(&fs::read(server.root.path().join(format!("data/terminals/{}.json", view.terminal_id))).unwrap()).unwrap();
    for field in ["rig", "agent", "status", "agent_session", "viewed_since_done"] {
        assert!(record.get(field).is_none(), "{field}: {record}");
    }

    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "observer" })).await;
    let hello = next_control(&mut socket, "hello").await;
    let terminal = hello["terminals"].as_array().unwrap().iter().find(|terminal| terminal["id"] == view.terminal_id).unwrap();
    assert!(terminal.get("agent").is_none());
    assert!(terminal.get("status").is_none());
    assert_eq!(next_control(&mut socket, "attention.changed").await["items"], json!([]));

    restored.send("\rprintf 'returned-to-shell\\n'\r").await;
    restored.settle(|text| text.lines().any(|line| line == "returned-to-shell")).await;
    restored.send("exit\r").await;
}
