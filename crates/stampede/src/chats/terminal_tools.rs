use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use super::Chats;
use crate::terminals::Terminals;

pub const STAGE: &str = "stage_command";
pub const READ: &str = "read_terminal";
pub const CLAUDE_STAGE: &str = "mcp__stampede_chat__stage_command";
pub const CLAUDE_READ: &str = "mcp__stampede_chat__read_terminal";
const MAX_COMMAND: usize = 64 * 1024;
const MAX_OUTPUT: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stage { command: String }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Read {
    terminal_id: String,
    #[serde(default = "default_lines")]
    lines: usize,
}

fn default_lines() -> usize { 200 }

pub fn stage_tool() -> Value {
    json!({
        "name": STAGE,
        "description": "Open a Stampede terminal in this chat's working directory and stage a command for the user to inspect, edit, and run. Use when a command requires sudo or permissions you do not have. The command stays in an editable composer and is never sent to the shell until the user submits it. Returns terminal_id for read_terminal. Tell the user why their action is needed, then wait for them to run it. Never ask for their password or claim the staged command has run.",
        "inputSchema": {
            "type": "object", "properties": {
                "command": { "type": "string", "minLength": 1, "maxLength": MAX_COMMAND, "description": "Literal shell command, including any needed sudo. Multiline commands are supported." },
            }, "required": ["command"], "additionalProperties": false,
        },
        "annotations": { "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false },
    })
}

pub fn read_tool() -> Value {
    json!({
        "name": READ,
        "description": "Read plain-text scrollback from a terminal created by stage_command in this chat, even when its tab is hidden. Returns the latest lines, capped at 64 KiB, whether the command is still pending, and whether output was truncated. This is a snapshot; it does not indicate command completion or exit status. Password input that the terminal does not echo is absent from output. Read after the user runs the command; do not busy-poll while waiting for the user.",
        "inputSchema": {
            "type": "object", "properties": {
                "terminal_id": { "type": "string", "minLength": 1 },
                "lines": { "type": "integer", "minimum": 1, "maximum": 2000, "default": 200 },
            }, "required": ["terminal_id"], "additionalProperties": false,
        },
        "annotations": { "readOnlyHint": true, "destructiveHint": false, "openWorldHint": false },
    })
}

fn error(message: impl ToString) -> Value {
    json!({ "content": [{ "type": "text", "text": message.to_string() }], "isError": true })
}

fn result(value: Value) -> Value {
    json!({ "content": [{ "type": "text", "text": value.to_string() }], "structuredContent": value, "isError": false })
}

pub async fn call(chats: &Chats, terminals: &Arc<Terminals>, chat_id: Option<String>, name: &str, arguments: Value, scrollback: usize) -> Value {
    let Some(chat_id) = chat_id else { return error("Terminal tools require a calling Stampede chat.") };
    let Some(entry) = chats.get(&chat_id) else { return error("The calling chat is no longer open.") };
    match name {
        STAGE => {
            let input = match serde_json::from_value::<Stage>(arguments) {
                Ok(input) if !input.command.trim().is_empty() && input.command.len() <= MAX_COMMAND
                    && !input.command.chars().any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t')) => input,
                _ => return error("Provide only a nonempty command up to 64 KiB, without terminal control characters."),
            };
            let chat = entry.chat.lock().unwrap().clone();
            match terminals.stage(&chat.project_id, &chat.cwd, &chat_id, input.command, scrollback).await {
                Ok(terminal) => result(json!({ "terminal_id": terminal.id, "pending": true, "cwd": chat.cwd })),
                Err(cause) => error(format!("Could not stage the command: {cause}")),
            }
        }
        READ => {
            let input = match serde_json::from_value::<Read>(arguments) {
                Ok(input) if (1..=2000).contains(&input.lines) => input,
                _ => return error("Provide terminal_id and optionally lines between 1 and 2000."),
            };
            let Some(terminal) = terminals.for_chat(&input.terminal_id, &chat_id) else {
                return error("That terminal was not created by this chat or is no longer open.");
            };
            match terminals.read(&terminal.id).await {
                Ok(output) => {
                    let (output, truncated) = tail(&output, input.lines);
                    result(json!({ "terminal_id": terminal.id, "pending": terminal.pending.is_some(), "state": terminal.state,
                        "output": output, "truncated": truncated }))
                }
                Err(cause) => error(format!("Could not read the terminal: {cause}")),
            }
        }
        _ => error("Unknown terminal tool."),
    }
}

fn tail(output: &str, lines: usize) -> (&str, bool) {
    let mut start = output.rmatch_indices('\n').nth(lines - 1).map_or(0, |(index, _)| index + 1);
    start = start.max(output.len().saturating_sub(MAX_OUTPUT));
    while !output.is_char_boundary(start) { start += 1; }
    (&output[start..], start > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_keeps_latest_lines_and_bounds_unicode_bytes() {
        assert_eq!(tail("first\nsecond\nthird", 2), ("second\nthird", true));
        assert_eq!(tail("first\nsecond", 2), ("first\nsecond", false));
        let output = format!("x{}", "界".repeat(MAX_OUTPUT));
        let (text, truncated) = tail(&output, 200);
        assert!(truncated);
        assert!(text.len() <= MAX_OUTPUT);
        assert!(text.chars().all(|character| character == '界'));
    }
}
