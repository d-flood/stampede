use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use super::Item;

pub const SERVER: &str = "stampede_chat";
pub const TOOL: &str = "render_html";
pub const CLAUDE_TOOL: &str = "mcp__stampede_chat__render_html";
const LIMIT: usize = 256 * 1024;

#[derive(clap::Args)]
pub struct ChatToolsArgs {
    #[arg(long, value_name = "DIR", default_value_os_t = crate::data_dir())]
    pub data_dir: PathBuf,
    #[arg(long)]
    pub chat_id: Option<String>,
}

pub fn config(data_dir: &Path, chat_id: &str) -> Value {
    json!({
        "command": std::env::current_exe().expect("cannot locate stampede"),
        "args": ["chat-tools", "--data-dir", std::path::absolute(data_dir).expect("cannot locate data directory"), "--chat-id", chat_id],
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

fn reply(request: &Value, args: &ChatToolsArgs) -> Option<Value> {
    let data_dir = &args.data_dir;
    let id = request.get("id")?;
    let canvas = args.chat_id.as_deref().map(|chat_id| format!(
        " For a page or document the user may iterate on, write it instead as an .html file in {}; Stampede renders it live beside the chat, so revise it with small edits rather than rewriting it.",
        crate::canvas::dir(data_dir, chat_id).display(),
    )).unwrap_or_default();
    let result = match request["method"].as_str() {
        Some("initialize") => json!({
            "protocolVersion": "2025-11-25",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": SERVER, "version": env!("CARGO_PKG_VERSION") },
        }),
        Some("ping") => json!({}),
        Some("tools/list") => json!({ "tools": [super::delegation::list_tool(), super::delegation::tool(), super::questions::tool(), super::terminal_tools::stage_tool(), super::terminal_tools::read_tool(), {
            "name": TOOL,
            "description": format!("Render an HTML snippet inline in Stampede chat. Use for diagrams, charts, tables, and visual explanations instead of ASCII art. Supply self-contained HTML with inline CSS, SVG, and optional inline JavaScript. It runs in an isolated sandbox: no network, external assets, parent-page access, forms, or popups. Make layouts responsive for narrow mobile screens. Do not repeat the HTML in a code block.{canvas}"),
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
        }, {
            "name": crate::media::TOOL,
            "description": "Display an existing image or screen recording inline in Stampede chat. Supply an absolute local file path and a descriptive title (also used as image alt text). Supports PNG, JPEG, GIF, WebP, MP4, and WebM up to 100 MiB; use browser-compatible video codecs. The file is copied into durable storage, so temporary source files may be removed afterward. Create screenshots and recordings with your existing tools before calling this tool. Do not encode media as base64 or repeat it in a code block.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "minLength": 1, "maxLength": 200 },
                    "path": { "type": "string", "minLength": 1 },
                },
                "required": ["title", "path"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false },
        }] }),
        Some("tools/call") if matches!(request["params"]["name"].as_str(), Some(super::delegation::LIST | super::delegation::TOOL | super::questions::TOOL | super::terminal_tools::STAGE | super::terminal_tools::READ)) => {
            super::delegation::call(args, request["params"].clone())
        }
        Some("tools/call") if request["params"]["name"] == TOOL => {
            let valid = item(&request["params"]["arguments"]).is_some();
            json!({
                "content": [{ "type": "text", "text": if valid { "Rendered HTML in the chat." } else { "Provide a title (1–200 characters) and nonempty HTML (up to 256 KiB), with no other arguments." } }],
                "isError": !valid,
            })
        }
        Some("tools/call") if request["params"]["name"] == crate::media::TOOL => {
            crate::media::call(data_dir, &request["params"]["arguments"])
        }
        _ => return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Method not found" } })),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

pub fn serve(args: ChatToolsArgs) -> io::Result<()> {
    let mut output = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let line = line?;
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => reply(&request, &args),
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
