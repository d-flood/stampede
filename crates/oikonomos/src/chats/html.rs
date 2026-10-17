use std::io::{self, BufRead, Write};

use serde::Deserialize;
use serde_json::{Value, json};

use super::Item;

pub const SERVER: &str = "oikonomos_chat";
pub const TOOL: &str = "render_html";
pub const CLAUDE_TOOL: &str = "mcp__oikonomos_chat__render_html";
const LIMIT: usize = 256 * 1024;

pub fn config() -> Value {
    json!({
        "command": std::env::current_exe().expect("cannot locate oikonomos"),
        "args": ["chat-tools"],
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snippet {
    title: String,
    html: String,
}

pub fn item(arguments: &Value) -> Option<Item> {
    let snippet: Snippet = serde_json::from_value(arguments.clone()).ok()?;
    if snippet.title.trim().is_empty() || snippet.title.chars().count() > 200
        || snippet.html.trim().is_empty() || snippet.html.len() > LIMIT
    {
        return None;
    }
    Some(Item::Html { title: snippet.title, html: snippet.html })
}

fn reply(request: &Value) -> Option<Value> {
    let id = request.get("id")?;
    let result = match request["method"].as_str() {
        Some("initialize") => json!({
            "protocolVersion": "2025-11-25",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": SERVER, "version": env!("CARGO_PKG_VERSION") },
        }),
        Some("ping") => json!({}),
        Some("tools/list") => json!({ "tools": [{
            "name": TOOL,
            "description": "Render an HTML snippet inline in Oikonomos chat. Use for diagrams, charts, tables, and visual explanations instead of ASCII art. Supply self-contained HTML with inline CSS, SVG, and optional inline JavaScript. It runs in an isolated sandbox: no network, external assets, parent-page access, forms, or popups. Make layouts responsive for narrow mobile screens. Do not repeat the HTML in a code block.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "minLength": 1, "maxLength": 200 },
                    "html": { "type": "string", "minLength": 1, "maxLength": LIMIT },
                },
                "required": ["title", "html"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true, "destructiveHint": false, "openWorldHint": false },
        }] }),
        Some("tools/call") if request["params"]["name"] == TOOL => {
            let valid = item(&request["params"]["arguments"]).is_some();
            json!({
                "content": [{ "type": "text", "text": if valid { "Rendered HTML in the chat." } else { "Provide a title (1–200 characters) and nonempty HTML (up to 256 KiB), with no other arguments." } }],
                "isError": !valid,
            })
        }
        _ => return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Method not found" } })),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

pub fn serve() -> io::Result<()> {
    let mut output = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let line = line?;
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => reply(&request),
            Err(_) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "Parse error" } })),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut output, &response)?;
            writeln!(output)?;
            output.flush()?;
        }
    }
    Ok(())
}
