use crate::common;

use std::{fs, io::Write, path::Path, process::{Command, Stdio}, time::Duration};

use common::{Server, control, next_control};
use serde_json::{Value, json};
use tokio::time::timeout;
use tokio_tungstenite::connect_async;

fn description(data_dir: &Path, chat_id: Option<&str>) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_stampede"));
    command.arg("chat-tools").arg("--data-dir").arg(data_dir);
    if let Some(chat_id) = chat_id { command.args(["--chat-id", chat_id]); }
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    writeln!(child.stdin.take().unwrap(), "{}", json!({ "id": 1, "method": "tools/list" })).unwrap();
    let output = child.wait_with_output().unwrap();
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    let tools = response["result"]["tools"].as_array().unwrap();
    tools.iter().find(|tool| tool["name"] == "render_html").unwrap()["description"].as_str().unwrap().to_owned()
}

#[test]
fn chat_html_description_points_each_chat_at_its_canvas_directory() {
    let root = tempfile::tempdir().unwrap();
    let described = description(root.path(), Some("0123abcd"));
    assert!(described.contains(&format!(" in {}; ", root.path().join("canvases/0123abcd").display())), "{described}");
    assert!(!description(root.path(), None).contains("canvases"));
}

#[tokio::test]
async fn chat_canvases_stream_changes_and_keep_versions_across_restarts() {
    let mut server = Server::start("stampede-canvas-").await;
    let canvas = server.root.path().join("data/canvases/0123abcd");
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    fs::create_dir_all(&canvas).unwrap();
    let client = reqwest::Client::new();
    let port = server.port;
    let url = |query: &str| format!("http://127.0.0.1:{port}/api/canvases/0123abcd/page.html{query}");
    for (version, html) in [(1, "<h1>First</h1>"), (2, "<h1>Second</h1>")] {
        let changed = timeout(Duration::from_secs(10), async {
            loop {
                fs::write(canvas.join("notes.md"), html).unwrap();
                fs::write(canvas.join("page.html"), html).unwrap();
                if let Ok(changed) = timeout(Duration::from_millis(500), next_control(&mut socket, "canvas.changed")).await { break changed; }
            }
        }).await.expect("the canvas watcher never reported the change");
        assert_eq!(changed, json!({ "type": "canvas.changed", "canvas": "0123abcd/page.html", "versions": version }));
        let response = client.get(url("")).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["content-type"], "text/plain; charset=utf-8");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert_eq!(response.headers()["x-canvas-versions"], version.to_string());
        assert_eq!(response.text().await.unwrap(), html);
    }
    fs::write(canvas.join("page.html"), "<h1>While stopped</h1>").unwrap();
    server.restart().await;
    let response = client.get(url("")).send().await.unwrap();
    assert_eq!(response.text().await.unwrap(), "<h1>While stopped</h1>");
    timeout(Duration::from_secs(10), async {
        while client.get(url("")).send().await.unwrap().headers()["x-canvas-versions"] != "3" {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }).await.expect("the restarted server never kept the change made while it was stopped");
    for (version, html) in [(1, "<h1>First</h1>"), (2, "<h1>Second</h1>"), (3, "<h1>While stopped</h1>")] {
        assert_eq!(client.get(url(&format!("?version={version}"))).send().await.unwrap().text().await.unwrap(), html);
    }
    for path in ["0123abcd/page.html?version=4", "0123abcd/page.html?version=0", "0123abcd/notes.md", "0123abcd/missing.html", ".hidden/page.html", "%2e%2e%2f0123abcd/page.html"] {
        let response = client.get(format!("http://127.0.0.1:{}/api/canvases/{path}", server.port)).send().await.unwrap();
        assert_eq!(response.status(), 404, "{path}");
    }
}
