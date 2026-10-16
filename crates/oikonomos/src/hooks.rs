use std::{
    env,
    error::Error,
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{AppState, config::Agent, holder, protocol::{self, Message}};

const TIMEOUT: Duration = Duration::from_secs(1);

const CLAUDE_CODE_EVENTS: [&str; 10] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

const CODEX_EVENTS: [(&str, &str, u32); 8] = [
    ("SessionStart", "session_start", 10),
    ("UserPromptSubmit", "user_prompt_submit", 10),
    ("PreToolUse", "pre_tool_use", 10),
    ("PermissionRequest", "permission_request", 10),
    ("PostToolUse", "post_tool_use", 10),
    ("Stop", "stop", 10),
    ("Interrupt", "interrupt", 3),
    ("SessionEnd", "session_end", 3),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Working,
    Waiting,
    Done,
}

pub enum Transition {
    To(Status),
    Exit,
}

#[derive(clap::Args)]
pub struct HookArgs {
    pub agent: String,
    pub event: String,
}

/// Agents run this on every hook, so it never fails, blocks for long, or prints.
pub fn hook(args: HookArgs) {
    let _ = report(args);
}

fn report(HookArgs { agent, event }: HookArgs) -> Result<(), Box<dyn Error>> {
    let mut stdin = Vec::new();
    io::stdin().read_to_end(&mut stdin)?;
    let hook = Hook {
        terminal_id: env::var("OIKONOMOS_TERMINAL_ID")?,
        agent: serde_json::from_value(json!(agent))?,
        event,
        payload: serde_json::from_slice(&stdin).unwrap_or_default(),
    };
    if post(&hook).is_ok() {
        return Ok(());
    }
    let dir = PathBuf::from(env::var_os("OIKONOMOS_DATA_DIR").ok_or("no data directory")?);
    let mut stream = UnixStream::connect(holder::socket_path(&dir, &hook.terminal_id))?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    protocol::read(&mut stream)?;
    stream.write_all(&protocol::encode(&Message::Hook(hook)))?;
    while let Some(message) = protocol::read(&mut stream)? {
        if matches!(message, Message::Hooked) { break; }
    }
    Ok(())
}

fn post(hook: &Hook) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + TIMEOUT;
    let url = env::var("OIKONOMOS_HOOK_URL")?;
    let rest = url.strip_prefix("http://").ok_or("not an http URL")?;
    let (authority, path) = rest.split_once('/').ok_or("no path")?;
    let body = serde_json::to_string(hook)?;
    let address = authority.to_socket_addrs()?.next().ok_or("no address")?;
    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    stream.set_write_timeout(Some(remaining))?;
    stream.set_read_timeout(Some(remaining))?;
    write!(
        stream,
        "POST /{path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len(),
    )?;
    // The response means the server has applied this event, so the next hook can't overtake it.
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    if response.starts_with(b"HTTP/1.1 204") { Ok(()) } else { Err("hook endpoint did not accept the event".into()) }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hook {
    pub terminal_id: String,
    pub agent: Agent,
    pub event: String,
    #[serde(default)]
    pub payload: Value,
}

pub async fn receive(State(state): State<AppState>, Json(hook): Json<Hook>) -> StatusCode {
    if state.terminals.hook(&hook.terminal_id, hook.agent, &hook.event, &hook.payload) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

pub fn transition(agent: Agent, event: &str, payload: &Value) -> Option<Transition> {
    use Agent::*;
    use Status::*;
    use Transition::*;
    let field = |key: &str| payload[key].as_str().unwrap_or_default();
    Some(match (agent, event) {
        (ClaudeCode, "SessionStart") if field("source") == "compact" => return None,
        (ClaudeCode, "SessionStart") => To(Done),
        (Codex, "PreToolUse") if field("tool_name") == "request_user_input" => To(Waiting),
        (_, "UserPromptSubmit" | "PreToolUse" | "PostToolUse") | (ClaudeCode, "PostToolUseFailure") => {
            To(Working)
        }
        (_, "PermissionRequest") => To(Waiting),
        (ClaudeCode, "Notification") => match field("notification_type") {
            "permission_prompt" | "elicitation_dialog" | "elicitation_url_dialog" => To(Waiting),
            "elicitation_response" | "elicitation_complete" => To(Working),
            _ => return None,
        },
        (_, "Stop") | (ClaudeCode, "StopFailure") | (Codex, "Interrupt") => To(Done),
        (ClaudeCode, "SessionEnd") if matches!(field("reason"), "clear" | "resume") => return None,
        (_, "SessionEnd") => Exit,
        _ => return None,
    })
}

pub fn wiring(agent: Agent, exe: &Path) -> Vec<String> {
    let exe = exe.to_string_lossy();
    match agent {
        Agent::ClaudeCode => {
            let hooks: serde_json::Map<_, _> = CLAUDE_CODE_EVENTS
                .into_iter()
                .map(|event| {
                    let handler = json!({
                        "type": "command",
                        "command": exe,
                        "args": ["hook", "claude-code", event],
                        "timeout": 5,
                    });
                    (event.to_owned(), json!([{ "hooks": [handler] }]))
                })
                .collect();
            vec!["--settings".to_owned(), json!({ "hooks": hooks }).to_string()]
        }
        Agent::Codex => {
            let mut args = vec!["-c".to_owned(), "features.codex_hooks=true".to_owned()];
            let mut state = Vec::new();
            for (event, snake, timeout) in CODEX_EVENTS {
                let command = format!("{} hook codex {event}", crate::terminals::quote(&exe));
                let quoted = toml::Value::String(command.clone());
                args.push("-c".to_owned());
                args.push(format!("hooks.{event}=[{{hooks=[{{type=\"command\",command={quoted},timeout={timeout}}}]}}]"));
                state.push(format!(
                    "\"/<session-flags>/config.toml:{snake}:0:0\"={{trusted_hash=\"{}\"}}",
                    codex_hash(snake, &command, timeout)
                ));
            }
            args.push("-c".to_owned());
            args.push(format!("hooks.state={{{}}}", state.join(",")));
            args
        }
    }
}

/// Codex skips launch-defined hooks unless trusted by this hash of their definition.
fn codex_hash(snake: &str, command: &str, timeout: u32) -> String {
    let definition = json!({
        "event_name": snake,
        "hooks": [{ "async": false, "command": command, "timeout": timeout, "type": "command" }],
    });
    let digest = Sha256::digest(definition.to_string());
    format!("sha256:{}", digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>())
}

