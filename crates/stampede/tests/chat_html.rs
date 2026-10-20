use std::{io::Write, process::{Command, Stdio}};

use serde_json::{Value, json};

#[test]
fn chat_html_tool_speaks_mcp_and_validates_snippets() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stampede"))
        .arg("chat-tools").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    let requests = [
        json!({ "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-11-25" } }),
        json!({ "method": "notifications/initialized" }),
        json!({ "id": 2, "method": "tools/list" }),
        json!({ "id": 3, "method": "tools/call", "params": { "name": "render_html", "arguments": { "title": "Flow", "html": "<svg></svg>" } } }),
        json!({ "id": 4, "method": "tools/call", "params": { "name": "render_html", "arguments": { "title": "Flow", "html": " " } } }),
        json!({ "id": 5, "method": "tools/call", "params": { "name": "render_html", "arguments": { "title": "Flow", "html": "x".repeat(256 * 1024 + 1) } } }),
        json!({ "id": 6, "method": "ping" }),
    ];
    for request in requests {
        writeln!(input, "{request}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let responses: Vec<Value> = String::from_utf8(output.stdout).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(responses.len(), 6);
    assert_eq!(responses[0]["result"]["capabilities"], json!({ "tools": {} }));
    let names: Vec<_> = responses[1]["result"]["tools"].as_array().unwrap().iter().map(|tool| tool["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["list_rigs", "delegate_to_rig", "ask_questions", "stage_command", "read_terminal", "render_html", "render_media"]);
    assert_eq!(responses[2]["result"]["isError"], false);
    assert_eq!(responses[3]["result"]["isError"], true);
    assert_eq!(responses[4]["result"]["isError"], true);
    assert_eq!(responses[5], json!({ "jsonrpc": "2.0", "id": 6, "result": {} }));
}
