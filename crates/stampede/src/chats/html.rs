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

fn record(args: &ChatToolsArgs) -> Option<super::Chat> {
    args.chat_id.as_deref().and_then(|id| super::read_record(&super::record_path(&args.data_dir, id)).ok())
}

fn can_delegate(args: &ChatToolsArgs) -> bool {
    record(args).is_none_or(|chat| chat.parent.is_none())
}

fn reply(request: &Value, args: &ChatToolsArgs) -> Option<Value> {
    let data_dir = &args.data_dir;
    let id = request.get("id")?;
    let canvas = args.chat_id.as_deref().map(|chat_id| format!(
        " For editable pages or substantial HTML, write .html files in {}; Stampede renders them live.",
        crate::canvas::dir(data_dir, chat_id).display(),
    )).unwrap_or_default();
    let result = match request["method"].as_str() {
        Some("initialize") => json!({
            "protocolVersion": "2025-11-25",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": SERVER, "version": env!("CARGO_PKG_VERSION") },
        }),
        Some("ping") => json!({}),
        Some("tools/list") => {
            let chat = record(args);
            let mut catalog = json!({ "tools": [super::questions::tool(), super::terminal_tools::stage_tool(), super::terminal_tools::read_tool(), {
                "name": TOOL,
                "description": format!("Render a small self-contained HTML snippet inline for visual explanations. Inline CSS/SVG/JS supported; no network, external assets, parent access, forms or popups. Make it mobile-responsive; do not repeat HTML in chat.{canvas}"),
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
                "description": "Display a local image or recording inline. Pass an absolute path and accessible title. Supports PNG/JPEG/GIF/WebP/MP4/WebM up to 100 MiB; use browser-compatible codecs. Stampede copies the file to durable storage. Do not embed base64 or repeat media in code blocks.",
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
            }] });
            let tools = catalog["tools"].as_array_mut().unwrap();
            if chat.as_ref().is_none_or(|chat| chat.parent.is_none()) {
                tools.splice(0..0, [super::delegation::list_tool(), super::delegation::tool()]);
            }
            tools.extend(chat.and_then(|chat| chat.duty).map(crate::cli::tool));
            catalog
        }
        Some("tools/call") if matches!(request["params"]["name"].as_str(), Some(super::delegation::LIST | super::delegation::TOOL)) && !can_delegate(args) => {
            json!({ "content": [{ "type": "text", "text": "Delegation tools are unavailable in delegated chats. Return your result to the calling agent." }], "isError": true })
        }
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
        Some("tools/call") if request["params"]["name"] == crate::cli::TOOL => {
            crate::cli::call(data_dir, args.chat_id.as_deref(), request["params"]["arguments"].clone())
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
