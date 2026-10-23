use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    fs::{self, File},
    io::{self, BufRead, Read},
    path::{Path, PathBuf},
    process::Stdio,
    time::UNIX_EPOCH,
};

use futures_util::StreamExt;
use serde::{
    Deserialize, Deserializer,
    de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::unix::OwnedWriteHalf,
    process,
    sync::{mpsc::{self, UnboundedReceiver, UnboundedSender}, oneshot},
};
use uuid::Uuid;

use super::{
    AgentCommand, Answer, Builtin, Capabilities, Chat, Choice, CommandStatus, Context, Driver, Event, Item, Notice, Outcome, Phase, Setting,
    Settings, Session, Subagent, Todo, TodoStatus, Tokens, ToolCall, ToolKind,
    holder::{self, Frames},
    html,
    protocol::{self, Message},
    shell,
    Request, RequestKind, Image, Scope, AnsweredQuestions, Question, diff, seconds, unified, words,
};
use crate::{
    config::{Agent, AgentSettings},
    chats::Status,
    usage::{Reading, Window},
};

const ARGS: [&str; 7] = [
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-partial-messages",
    "--permission-prompt-tool=stdio",
];

const SDK_ENV: [(&str, &str); 2] = [("CLAUDE_CODE_ENTRYPOINT", "sdk-ts"), ("CLAUDE_CODE_SDK_READS_SESSION_STATE", "1")];

const WINDOWS: [(Window, &str); 2] = [(Window::FiveHour, "five_hour"), (Window::Weekly, "seven_day")];

const HEAD: u64 = 1 << 20;
const DENIED: &str = "The user denied this request.";

const MANAGEMENT: &[&str] = &[
    "mcp", "hooks", "memory", "permissions", "plugin", "plugins", "statusline", "theme", "export", "add-dir",
    "agents", "login", "logout", "config", "heapdump",
];

pub struct Claude {
    commands: UnboundedSender<Command>,
}

enum Command {
    Send(String, Vec<Image>),
    Retry,
    Interrupt,
    Answer(String, Answer),
    Run(Builtin),
    Close,
    Delegated(Item),
    Shell(String),
}

impl Driver for Claude {
    fn send(&self, text: &str, images: &[Image]) {
        let _ = self.commands.send(Command::Send(text.to_owned(), images.to_vec()));
    }

    fn retry(&self) {
        let _ = self.commands.send(Command::Retry);
    }

    fn interrupt(&self) {
        let _ = self.commands.send(Command::Interrupt);
    }

    fn answer(&self, request_id: &str, answer: Answer) {
        let _ = self.commands.send(Command::Answer(request_id.to_owned(), answer));
    }

    fn run(&self, builtin: Builtin) {
        let _ = self.commands.send(Command::Run(builtin));
    }

    fn delegated(&self, item: Item) {
        let _ = self.commands.send(Command::Delegated(item));
    }

    fn close(&self) {
        let _ = self.commands.send(Command::Close);
    }

    fn shell(&self, command: &str) {
        let _ = self.commands.send(Command::Shell(command.to_owned()));
    }
}

pub async fn options(cwd: &Path, env: &BTreeMap<String, String>, settings: &AgentSettings) -> io::Result<(Vec<Value>, AgentSettings)> {
    let mut child = process::Command::new("claude")
        .args(ARGS).args(settings.claude_flags()).envs(env).envs(SDK_ENV).current_dir(cwd)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .kill_on_drop(true).spawn()?;
    let result = async {
        let mut writer = child.stdin.take().unwrap();
        let initialize = json!({ "type": "control_request", "request_id": "models", "request": { "subtype": "initialize" } });
        writer.write_all(format!("{initialize}\n").as_bytes()).await?;
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut models = Vec::new();
        while let Some(line) = lines.next_line().await? {
            let message: Value = serde_json::from_str(&line)?;
            if message["type"] == "control_response" {
                let response = &message["response"];
                let id = response["request_id"].as_str().unwrap_or_default();
                if !["models", "settings"].contains(&id) { continue; }
                if response["subtype"] != "success" {
                    return Err(io::Error::other(response.to_string()));
                }
                if id == "models" {
                    models = response["response"]["models"].as_array().cloned().unwrap_or_default();
                    let request = json!({ "type": "control_request", "request_id": "settings", "request": { "subtype": "get_settings" } });
                    writer.write_all(format!("{request}\n").as_bytes()).await?;
                } else {
                    let applied = &response["response"]["applied"];
                    let settings = AgentSettings {
                        model: applied["model"].as_str().map(|model| model_alias(&models, model)),
                        effort: applied["effort"].as_str().map(str::to_owned),
                        mode: settings.mode.clone(),
                    };
                    return Ok((models, settings));
                }
            }
        }
        Err(io::Error::other("Claude Code closed before listing models"))
    }.await;
    let _ = child.kill().await;
    result
}

fn model_alias(models: &[Value], model: &str) -> String {
    let listed = models.iter().find(|listed| listed["value"] == model);
    let resolved = || models.iter().find(|listed| listed["value"] != "default" && listed["resolvedModel"] == model);
    listed.or_else(resolved).and_then(|listed| listed["value"].as_str()).unwrap_or(model).to_owned()
}

pub async fn start(
    data_dir: &Path,
    id: &str,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    settings: &AgentSettings,
    session: Option<&str>,
) -> io::Result<(Claude, UnboundedReceiver<Event>)> {
    let (frames, writer, transcript, received) = launch(data_dir, id, cwd, env, settings, session, None, None).await?;
    Ok((drive(frames, writer, transcript), received))
}

pub async fn resume(
    data_dir: &Path,
    chat: &Chat,
    env: &BTreeMap<String, String>,
) -> io::Result<(Claude, UnboundedReceiver<Event>)> {
    let _ = fs::remove_file(holder::socket_path(data_dir, &chat.id));
    let session = chat.agent_session.as_deref();
    let (frames, writer, mut transcript, received) =
        launch(data_dir, &chat.id, &chat.cwd, env, &chat.settings.chosen, session, chat.resume_at.as_deref(), None).await?;
    transcript.window = chat.context.map(|context| context.window);
    if chat.status != Status::Done {
        if chat.status == Status::Working { let _ = transcript.events.send(Event::Recoverable); }
        transcript.push(Item::interrupted());
    }
    transcript.restore_shell_context(&chat.shell_context);
    let _ = transcript.events.send(Event::Status(Status::Done));
    Ok((drive(frames, writer, transcript), received))
}

async fn launch(
    data_dir: &Path,
    id: &str,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    settings: &AgentSettings,
    session: Option<&str>,
    at: Option<&str>,
    sender: Option<UnboundedSender<Event>>,
) -> io::Result<(Frames, OwnedWriteHalf, Transcript, UnboundedReceiver<Event>)> {
    let variables = [("CLAUDE_CODE_ENABLE_TODO_TOOLS", "1"), ("CLAUDE_CODE_ARTIFACT", "1")].into_iter()
        .chain(env.iter().map(|(name, value)| (name.as_str(), value.as_str())))
        .chain(SDK_ENV)
        .chain([("CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING", "true")]);
    let mut config = html::config(data_dir, id);
    config["timeout"] = json!(86400000);
    let tools = ["--mcp-config".to_owned(), json!({ "mcpServers": { html::SERVER: config } }).to_string(),
        "--allowedTools".to_owned(), [html::CLAUDE_TOOL, crate::media::CLAUDE_TOOL, super::delegation::CLAUDE_LIST, super::delegation::CLAUDE_TOOL, super::questions::CLAUDE_TOOL,
            super::terminal_tools::CLAUDE_STAGE, super::terminal_tools::CLAUDE_READ].join(",")];
    let command = ["claude"].iter().chain(&ARGS).map(|arg| arg.to_string()).chain(settings.claude_flags()).chain(tools);
    let resume = session.into_iter().flat_map(|session| ["--resume".to_owned(), session.to_owned()]);
    let rewind = at.into_iter().flat_map(|at| ["--resume-session-at".to_owned(), at.to_owned()]);
    holder::spawn(data_dir, id, cwd, variables, command.chain(resume).chain(rewind)).await?;
    let (frames, mut writer, _) = holder::connect(data_dir, id).await?;
    let (events, received) = mpsc::unbounded_channel();
    let mut transcript = Transcript::new(sender.unwrap_or(events), cwd);
    transcript.location = Some(Location { data_dir: data_dir.to_owned(), id: id.to_owned(), env: env.clone() });
    transcript.session = session.map(str::to_owned);
    if let Some(session) = session {
        transcript = transcript.replay(env, session, at).await?;
        transcript.stop_commands();
    }
    transcript.settings.chosen = settings.clone();
    transcript.discover(&mut writer).await?;
    Ok((frames, writer, transcript, received))
}

/// Rebuilds the transcript from the session file up to the holder's latest turn, then from the turn the holder replays.
pub async fn reattach(
    data_dir: &Path,
    chat: &Chat,
    env: &BTreeMap<String, String>,
) -> io::Result<(Claude, UnboundedReceiver<Event>)> {
    let (mut frames, mut writer, buffered) = holder::connect(data_dir, &chat.id).await?;
    let mut replay = Vec::with_capacity(buffered);
    for _ in 0..buffered {
        let frame = frames.next().await.ok_or(io::ErrorKind::UnexpectedEof)??;
        replay.extend(protocol::decode(&frame)?);
    }
    let turn = replay.iter().find_map(|message| match message {
        Message::Input(line) => user(line)?["uuid"].as_str().map(str::to_owned),
        _ => None,
    });
    let (events, received) = mpsc::unbounded_channel();
    let mut transcript = Transcript::new(events, &chat.cwd);
    transcript.location = Some(Location { data_dir: data_dir.to_owned(), id: chat.id.clone(), env: env.clone() });
    transcript.session = chat.agent_session.clone();
    transcript.replaying = true;
    transcript.window = chat.context.map(|context| context.window);
    transcript.settings.chosen = chat.settings.chosen.clone();
    if let Some(session) = &chat.agent_session {
        transcript = transcript.replay(env, session, turn.as_deref().or(chat.resume_at.as_deref())).await?;
    }
    for message in replay {
        match message {
            Message::Line(line) => {
                if let Ok(message) = serde_json::from_slice(&line) {
                    transcript.read(&message);
                }
            }
            Message::Input(line) => {
                let Ok(message) = serde_json::from_slice::<Value>(&line) else { continue };
                match message["type"].as_str() {
                    Some("user") => transcript.sent(&message),
                    Some("control_response") => {
                        transcript.resolve(message["response"]["request_id"].as_str().unwrap_or_default())
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    transcript.replaying = false;
    transcript.restore_shell_context(&chat.shell_context);
    for request in std::mem::take(&mut transcript.pending) {
        let _ = transcript.events.send(Event::Request(request));
    }
    let _ = transcript.events.send(Event::Status(transcript.status));
    if transcript.todos_held {
        transcript.report_todos();
    }
    transcript.discover(&mut writer).await?;
    Ok((drive(frames, writer, transcript), received))
}

fn drive(mut frames: Frames, mut writer: OwnedWriteHalf, mut transcript: Transcript) -> Claude {
    let (commands, mut pending) = mpsc::unbounded_channel();
    let (progress, mut progressed) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                command = pending.recv() => match command {
                    Some(Command::Send(text, images)) => {
                        let text = match transcript.take_shell_context() {
                            Some(blocks) => format!("{blocks}\n\n{text}"),
                            None => text,
                        };
                        if send(&mut writer, &mut transcript, &text, &images).await.is_err() {
                            break;
                        }
                    }
                    Some(Command::Shell(command)) => transcript.run_shell(command, &progress),
                    Some(Command::Delegated(item)) => transcript.delegated(item),
                    Some(Command::Retry) => {
                        if let Some(content) = transcript.last_input.clone() {
                            let message = json!({ "type": "user", "message": { "role": "user", "content": content },
                                "parent_tool_use_id": null, "uuid": Uuid::new_v4().to_string() });
                            transcript.sent(&message);
                            if writer.write_all(&protocol::encode(&Message::Forget)).await.is_err()
                                || writer.write_all(&input(&message)).await.is_err() { break; }
                        }
                    }
                    Some(Command::Run(Builtin::Rewind(index))) => {
                        match rewind(&mut frames, &mut writer, &mut transcript, index).await {
                            Ok(Some(restarted)) => (frames, writer, transcript) = restarted,
                            Ok(None) => {},
                            Err(error) => {
                                transcript.push(Item::Notice { notice: Notice::Error, text: format!("Could not rewind: {error}") });
                            }
                        }
                    }
                    Some(Command::Run(builtin)) => {
                        if run(&mut writer, &mut transcript, builtin).await.is_err() {
                            break;
                        }
                    }
                    Some(Command::Interrupt) => {
                        transcript.stop_shells();
                        let interrupt = json!({
                            "type": "control_request",
                            "request_id": Uuid::new_v4().to_string(),
                            "request": { "subtype": "interrupt", "cancel_queued": true },
                        });
                        if transcript.status != Status::Done && writer.write_all(&input(&interrupt)).await.is_err() {
                            break;
                        }
                    }
                    Some(Command::Answer(id, answer)) => {
                        let original = transcript.inputs.remove(&id).unwrap_or_default();
                        transcript.unblock(&id);
                        let response = match answer {
                            Answer::Responses(_) => continue,
                            Answer::Allow => json!({ "behavior": "allow" }),
                            Answer::Rules(rules) => {
                                let parts = words(original["input"]["command"].as_str().unwrap_or_default());
                                let updates = rules.iter().zip(parts).map(|(rule, part)| {
                                    let command = shlex::try_join(rule.prefix.iter().map(String::as_str)).unwrap_or_default();
                                    let content = if rule.prefix == part { command } else { format!("{command}:*") };
                                    json!({ "type": "addRules", "rules": [{ "toolName": "Bash", "ruleContent": content }],
                                        "behavior": "allow", "destination": if rule.scope == Scope::Chat { "session" } else { "localSettings" } })
                                }).collect::<Vec<_>>();
                                json!({ "behavior": "allow", "updatedPermissions": updates })
                            }
                            Answer::Answers(answers) => {
                                let mut updated = original["input"].clone();
                                let mut mapped = serde_json::Map::new();
                                for (question, answers) in original["input"]["questions"].as_array().into_iter().flatten().zip(answers) {
                                    if let Some(question) = question["question"].as_str() {
                                        mapped.insert(question.to_owned(), json!(answers.join(", ")));
                                    }
                                }
                                updated["answers"] = json!(mapped);
                                json!({ "behavior": "allow", "updatedInput": updated })
                            }
                            Answer::Deny { instead: Some(instead) } if original["tool_name"] == "AskUserQuestion" => {
                                let mut updated = original["input"].clone();
                                let answers = original["input"]["questions"].as_array().into_iter().flatten()
                                    .filter_map(|q| q["question"].as_str()).map(|q| (q.to_owned(), json!(instead))).collect::<serde_json::Map<_, _>>();
                                updated["answers"] = json!(answers);
                                json!({ "behavior": "allow", "updatedInput": updated })
                            }
                            Answer::Deny { instead: Some(instead) } => json!({ "behavior": "deny", "message": instead }),
                            Answer::Deny { instead: None } => json!({ "behavior": "deny", "message": DENIED, "interrupt": true }),
                        };
                        let response = json!({
                            "type": "control_response",
                            "response": { "subtype": "success", "request_id": id, "response": response },
                        });
                        if writer.write_all(&input(&response)).await.is_err() {
                            break;
                        }
                    }
                    Some(Command::Close) => {
                        let _ = writer.write_all(&protocol::encode(&Message::Hangup)).await;
                    }
                    None => break,
                },
                Some(progress) = progressed.recv() => transcript.shell_progress(progress),
                frame = frames.next() => {
                    let Some(Ok(frame)) = frame else { break };
                    if let Ok(Some(Message::Line(line))) = protocol::decode(&frame)
                        && let Ok(message) = serde_json::from_slice(&line)
                    {
                        transcript.read(&message);
                    }
                }
            }
        }
        transcript.stop_commands();
    });
    Claude { commands }
}

async fn rewind(
    frames: &mut Frames,
    writer: &mut OwnedWriteHalf,
    transcript: &mut Transcript,
    index: usize,
) -> io::Result<Option<(Frames, OwnedWriteHalf, Transcript)>> {
    if transcript.status != Status::Done { return Ok(None); }
    let Some(checkpoint) = transcript.checkpoints.get(&index).cloned() else { return Ok(None) };
    let Some(location) = transcript.location.clone() else { return Ok(None) };
    let Some(session) = transcript.session.clone() else { return Ok(None) };
    let id = Uuid::new_v4().to_string();
    let request = json!({ "type": "control_request", "request_id": id,
        "request": { "subtype": "rewind_files", "user_message_id": checkpoint } });
    writer.write_all(&input(&request)).await?;
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let frame = frames.next().await.ok_or(io::ErrorKind::UnexpectedEof)??;
            if let Some(Message::Line(line)) = protocol::decode(&frame)? {
                let message: Value = serde_json::from_slice(&line)?;
                if message["type"] == "control_response" && message["response"]["request_id"] == id {
                    let response = &message["response"];
                    if response["subtype"] != "success" || response["response"]["canRewind"] == false {
                        return Err(io::Error::other(format!("{}", response.get("error").unwrap_or(&response["response"]))));
                    }
                    return Ok(());
                }
                transcript.read(&message);
            }
        }
    }).await.map_err(io::Error::other)??;
    writer.write_all(&protocol::encode(&Message::Hangup)).await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), async { while frames.next().await.is_some() {} }).await.map_err(io::Error::other)?;
    let _ = fs::remove_file(holder::socket_path(&location.data_dir, &location.id));
    let events = transcript.events.clone();
    let _ = events.send(Event::QuestionsTruncated(index));
    let _ = events.send(Event::Truncate(0));
    let _ = events.send(Event::Reset);
    let _ = events.send(Event::ShellContext(Vec::new()));
    let (frames, writer, mut fresh, _) = launch(&location.data_dir, &location.id, &transcript.cwd,
        &location.env, &transcript.settings.chosen, Some(&session), Some(&checkpoint), Some(events)).await?;
    let _ = fresh.events.send(Event::ResumeAt(Some(checkpoint)));
    fresh.push(Item::Notice { notice: Notice::Rewound, text: "Conversation rewound; tracked edits restored. Bash and subagent changes are not restored.".to_owned() });
    Ok(Some((frames, writer, fresh)))
}

/// Asks a fresh `claude` for the plan's windows with the `get_usage` control request, which makes no model call.
pub async fn usage(env: &BTreeMap<String, String>) -> io::Result<Vec<Reading>> {
    let mut claude = process::Command::new("claude")
        .args(ARGS)
        .envs(env)
        .envs(SDK_ENV)
        .current_dir(crate::home())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = claude.stdin.take().unwrap();
    let id = Uuid::new_v4().to_string();
    let request = json!({
        "type": "control_request",
        "request_id": id,
        "request": { "subtype": "get_usage", "skip_behaviors": true },
    });
    stdin.write_all(format!("{request}\n").as_bytes()).await?;
    let mut lines = BufReader::new(claude.stdout.take().unwrap()).lines();
    while let Some(line) = lines.next_line().await? {
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        let response = &message["response"];
        if message["type"] != "control_response" || response["request_id"] != id {
            continue;
        }
        drop(stdin);
        claude.wait().await?;
        if response["subtype"] != "success" {
            return Err(io::Error::other(format!("get_usage: {}", response["error"])));
        }
        let limits = &response["response"]["rate_limits"];
        let readings = WINDOWS.into_iter().filter_map(|(window, name)| {
            let used = limits[name]["utilization"].as_f64()? / 100.0;
            Some(Reading::new(Agent::ClaudeCode, window, used, limits[name]["resets_at"].as_str().and_then(super::epoch_ms)))
        });
        return Ok(readings.collect());
    }
    Err(io::Error::other("claude ended without answering get_usage"))
}

/// Asks one question over the Chat protocol with no tools, MCP servers, settings, hooks or CLAUDE.md files.
pub async fn ask(cwd: &Path, env: &BTreeMap<String, String>, model: Option<&str>, effort: Option<&str>, prompt: &str) -> io::Result<String> {
    let mut claude = process::Command::new("claude")
        .args(ARGS)
        .args(["--tools", "", "--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands", "--no-session-persistence", "--system-prompt", ""])
        .args(model.into_iter().flat_map(|model| ["--model", model]))
        .args(effort.into_iter().flat_map(|effort| ["--effort", effort]))
        .envs(env)
        .envs(SDK_ENV)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut stderr = claude.stderr.take().unwrap();
    let errors = tokio::spawn(async move {
        let mut text = String::new();
        let _ = tokio::io::AsyncReadExt::read_to_string(&mut stderr, &mut text).await;
        text
    });
    let mut stdin = claude.stdin.take().unwrap();
    let initialize = json!({ "type": "control_request", "request_id": Uuid::new_v4().to_string(), "request": { "subtype": "initialize" } });
    let question = json!({ "type": "user", "message": { "role": "user", "content": prompt }, "parent_tool_use_id": null, "uuid": Uuid::new_v4().to_string() });
    stdin.write_all(format!("{initialize}\n{question}\n").as_bytes()).await?;
    let mut lines = BufReader::new(claude.stdout.take().unwrap()).lines();
    while let Some(line) = lines.next_line().await? {
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        if message["type"] != "result" {
            continue;
        }
        drop(stdin);
        let text = message["result"].as_str().or(message["subtype"].as_str()).unwrap_or_default().trim().to_owned();
        return if message["is_error"] == true { Err(io::Error::other(text)) } else { Ok(text) };
    }
    let errors = errors.await.unwrap_or_default();
    Err(io::Error::other(Some(errors.trim()).filter(|errors| !errors.is_empty()).unwrap_or("claude ended without answering").to_owned()))
}

async fn send(writer: &mut OwnedWriteHalf, transcript: &mut Transcript, text: &str, images: &[Image]) -> io::Result<()> {
    let mut message = json!({
        "type": "user",
        "message": { "role": "user", "content": text },
        "parent_tool_use_id": null,
        "uuid": Uuid::new_v4().to_string(),
    });
    if !images.is_empty() {
        let mut content = vec![json!({ "type": "text", "text": text })];
        content.extend(images.iter().map(|image| json!({ "type": "image", "source": {
            "type": "base64", "media_type": image.media_type, "data": image.data
        } })));
        message["message"]["content"] = json!(content);
    }
    let idle = transcript.status == Status::Done;
    if !idle {
        message["priority"] = json!("next");
    }
    transcript.sent(&message);
    if idle {
        writer.write_all(&protocol::encode(&Message::Forget)).await?;
    }
    writer.write_all(&input(&message)).await
}

async fn run(writer: &mut OwnedWriteHalf, transcript: &mut Transcript, builtin: Builtin) -> io::Result<()> {
    let (setting, request, value) = match builtin {
        Builtin::Compact => return send(writer, transcript, "/compact", &[]).await,
        Builtin::Clear => {
            transcript.take_shell_context();
            return send(writer, transcript, "/clear", &[]).await;
        }
        Builtin::Rewind(_) => return Ok(()),
        Builtin::Model(model) => (Setting::Model, json!({ "subtype": "set_model", "model": model }), model),
        Builtin::Effort(effort) => {
            let request = json!({ "subtype": "apply_flag_settings", "settings": { "effortLevel": effort } });
            (Setting::Effort, request, effort)
        }
        Builtin::Mode(mode) => (Setting::Mode, json!({ "subtype": "set_permission_mode", "mode": mode }), mode),
    };
    *transcript.settings.get_mut(setting) = Some(value.clone());
    transcript.settings.mark_pending(setting);
    transcript.changed(setting);
    let request = transcript.request(request, Asked::Set(setting, value));
    writer.write_all(&request).await
}

fn user(line: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(line).ok().filter(|message| message["type"] == "user")
}

fn image_urls(content: &Value) -> Vec<String> {
    content.as_array().into_iter().flatten().filter(|block| block["type"] == "image").filter_map(|block| {
        let source = &block["source"];
        Some(format!("data:{};base64,{}", source["media_type"].as_str()?, source["data"].as_str()?))
    }).collect()
}

/// The sessions Claude keeps for `cwd`, each with the first prompt found in its file's head.
pub fn sessions(cwd: &Path, env: &BTreeMap<String, String>) -> io::Result<Vec<Session>> {
    let Some(config) = config_dir(env) else { return Ok(Vec::new()) };
    let name: String = cwd.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let entries = match fs::read_dir(config.join("projects").join(name)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        entries => entries?,
    };
    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension() != Some("jsonl".as_ref()) {
            continue;
        }
        let (Some(id), Some(prompt)) = (path.file_stem().and_then(|stem| stem.to_str()), first_prompt(&path)) else {
            continue;
        };
        let Some(updated) = entry.metadata().and_then(|metadata| metadata.modified()).ok() else { continue };
        let updated = updated.duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_millis() as u64;
        sessions.push(Session::new(Agent::ClaudeCode, id, &prompt, updated));
    }
    Ok(sessions)
}

fn first_prompt(path: &Path) -> Option<String> {
    let head = std::io::BufReader::new(File::open(path).ok()?.take(HEAD));
    let entries = head.lines().map_while(Result::ok).filter_map(|line| serde_json::from_str::<Value>(&line).ok());
    entries.filter(|entry| entry["type"] == "user" && entry["isMeta"] != true && entry["isSidechain"] != true).find_map(|entry| {
        let text = match &entry["message"]["content"] {
            Value::String(text) => text.clone(),
            Value::Array(blocks) => blocks.iter().filter_map(|block| block["text"].as_str()).collect::<Vec<_>>().join("\n"),
            _ => return None,
        };
        if text.starts_with("<command-message>") {
            let args = tagged(&text, "command-args").unwrap_or_default();
            return Some(format!("{} {args}", tagged(&text, "command-name")?).trim_end().to_owned());
        }
        // Local commands, their output, and notifications all arrive as user text wrapped in a tag.
        (!text.is_empty() && !text.starts_with('<')).then_some(text)
    })
}

fn tagged<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let start = text.find(&format!("<{tag}>"))? + tag.len() + 2;
    let end = start + text[start..].find(&format!("</{tag}>"))?;
    Some(&text[start..end])
}

fn lines(file: File) -> impl Iterator<Item = String> {
    std::io::BufReader::new(file).lines().map_while(Result::ok)
}

/// How a session entry chains to the others, which is all replaying needs before reading the entry in full.
#[derive(Deserialize)]
struct Link {
    #[serde(rename = "type")]
    kind: Option<String>,
    subtype: Option<String>,
    uuid: Option<String>,
    #[serde(rename = "parentUuid", default, deserialize_with = "present")]
    parent: Option<Option<String>>,
    #[serde(rename = "isSidechain", default)]
    sidechain: bool,
    message: Option<Linked>,
}

#[derive(Deserialize)]
struct Linked {
    #[serde(default, deserialize_with = "blocks")]
    content: Vec<Block>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: Option<String>,
    id: Option<String>,
    tool_use_id: Option<String>,
}

impl Link {
    fn turn(&self) -> bool {
        matches!(self.kind.as_deref(), Some("user" | "assistant"))
    }

    fn compact(&self) -> bool {
        self.kind.as_deref() == Some("system") && self.subtype.as_deref() == Some("compact_boundary")
    }

    fn tool_uses(&self) -> impl Iterator<Item = &str> {
        self.blocks("tool_use").filter_map(|block| block.id.as_deref())
    }

    fn tool_results(&self) -> impl Iterator<Item = &str> {
        self.blocks("tool_result").filter_map(|block| block.tool_use_id.as_deref())
    }

    fn blocks(&self, kind: &str) -> impl Iterator<Item = &Block> {
        self.message.iter().flat_map(|message| &message.content).filter(move |block| block.kind.as_deref() == Some(kind))
    }
}

/// A null `parentUuid` marks the first entry, unlike a missing one.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<String>>, D::Error> {
    Option::deserialize(deserializer).map(Some)
}

/// A message's content blocks, or none when its content is plain text.
fn blocks<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Block>, D::Error> {
    struct Blocks;

    impl<'de> Visitor<'de> for Blocks {
        type Value = Vec<Block>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("text or content blocks")
        }

        fn visit_str<E>(self, _: &str) -> Result<Vec<Block>, E> {
            Ok(Vec::new())
        }

        fn visit_unit<E>(self) -> Result<Vec<Block>, E> {
            Ok(Vec::new())
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<Block>, A::Error> {
            let mut blocks = Vec::new();
            while let Some(block) = seq.next_element()? {
                blocks.push(block);
            }
            Ok(blocks)
        }
    }

    deserializer.deserialize_any(Blocks)
}

/// The fields of a session entry, and of its tool result, that replaying reads. The rest of a long line, such as the
/// whole file a Read returned, is skipped rather than held, and anything but an object reads as null.
#[derive(Clone, Copy)]
struct Only(&'static [&'static str]);

const ENTRY: Only = Only(&[
    "type", "subtype", "uuid", "isSidechain", "isMeta", "isCompactSummary", "promptSource", "renderedRole", "message", "attachment",
    "toolUseResult", "thinkingDurationMs", "error", "timestamp", "usage", "task_id", "tool_use_id", "status", "patch", "summary",
]);

const RESULT: Only = Only(&[
    "type", "content", "structuredPatch", "original_file", "updated_file", "filePath", "notebook_path", "answers", "questions",
    "task_type", "task_id", "task", "agentId", "resolvedModel", "isAsync", "status", "totalDurationMs", "totalToolUseCount",
    "totalTokens", "backgroundTaskId",
]);

fn entry(line: &str) -> Option<Value> {
    let mut deserializer = serde_json::Deserializer::from_str(line);
    let entry = ENTRY.deserialize(&mut deserializer).ok()?;
    deserializer.end().ok()?;
    Some(entry)
}

impl<'de> DeserializeSeed<'de> for Only {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Only {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("JSON")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut kept = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if !self.0.contains(&key.as_str()) {
                map.next_value::<IgnoredAny>()?;
            } else if key == "toolUseResult" {
                kept.insert(key, map.next_value_seed(RESULT)?);
            } else {
                kept.insert(key, map.next_value()?);
            }
        }
        Ok(Value::Object(kept))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(Value::Null)
    }

    fn visit_str<E>(self, _: &str) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E>(self, _: bool) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
}

fn session_file(env: &BTreeMap<String, String>, session: &str) -> Option<PathBuf> {
    let name = format!("{session}.jsonl");
    let projects = fs::read_dir(config_dir(env)?.join("projects")).ok()?;
    projects.flatten().map(|project| project.path().join(&name)).find(|path| path.is_file())
}

fn config_dir(env: &BTreeMap<String, String>) -> Option<PathBuf> {
    match env.get("CLAUDE_CONFIG_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|| std::env::home_dir().map(|home| home.join(".claude"))),
    }
}

fn request(id: &str, request: &Value) -> RequestKind {
    let id = id.to_owned();
    let input = &request["input"];
    let text = |field: &str| input[field].as_str().unwrap_or_default().to_owned();
    match request["tool_name"].as_str() {
        Some("Bash") => {
            let parts = words(&text("command"));
            let rules = request["permission_suggestions"].as_array().into_iter().flatten()
                .flat_map(|suggestion| suggestion["rules"].as_array().into_iter().flatten())
                .filter(|rule| rule["toolName"] == "Bash")
                .filter_map(|rule| rule["ruleContent"].as_str())
                .flat_map(|rule| words(rule.trim_end_matches(":*"))).collect::<Vec<_>>();
            let suggestion = parts.iter().map(|part| rules.iter().find(|rule| part.starts_with(rule)).unwrap_or(part).clone()).collect();
            RequestKind::Command { id, words: parts, suggestion, session_prefixes: true, allow_rules: None }
        }
        Some("AskUserQuestion") => RequestKind::Question {
            id, questions: serde_json::from_value(input["questions"].clone()).unwrap_or_default(),
        },
        Some("ExitPlanMode") => RequestKind::Plan { id, plan: text("plan") },
        Some("Edit") => {
            let diff = diff(&text("old_string"), &text("new_string"));
            RequestKind::Edit { id, path: text("file_path"), diff }
        }
        Some("Write") => RequestKind::Edit { id, path: text("file_path"), diff: diff("", &text("content")) },
        tool => {
            let name = request["display_name"].as_str().or(tool).unwrap_or("a tool");
            let title = request["title"].as_str().map_or_else(|| format!("Claude wants to use {name}"), str::to_owned);
            RequestKind::Other { id, title, detail: serde_json::to_string_pretty(input).unwrap_or_default() }
        }
    }
}

fn recorded_answers(question: &Question, text: &str) -> Vec<String> {
    if text.is_empty() { return Vec::new(); }
    if !question.multi_select { return vec![text.to_owned()]; }
    let mut remaining = text;
    let mut answers = Vec::new();
    while !remaining.is_empty() {
        let option = question.options.iter().filter(|option| {
            !option.label.is_empty() && (remaining == option.label || remaining.strip_prefix(&option.label).is_some_and(|tail| tail.starts_with(", ")))
        }).max_by_key(|option| option.label.len());
        let Some(option) = option else {
            answers.push(remaining.to_owned());
            break;
        };
        answers.push(option.label.clone());
        remaining = remaining[option.label.len()..].strip_prefix(", ").unwrap_or_default();
    }
    answers
}

fn input(message: &Value) -> Vec<u8> {
    protocol::encode(&Message::Input(message.to_string().into_bytes()))
}

fn interrupted(content: &Value) -> bool {
    let mut blocks = content.as_array().into_iter().flatten();
    blocks.any(|block| block["text"].as_str().is_some_and(|text| text.starts_with("[Request interrupted by user")))
}

#[derive(Clone)]
struct Location {
    data_dir: PathBuf,
    id: String,
    env: BTreeMap<String, String>,
}

struct Running {
    item: Item,
    stdout: shell::Capture,
    stderr: shell::Capture,
    stop: Option<oneshot::Sender<()>>,
}

struct Transcript {
    running: HashMap<usize, Running>,
    shell_context: Vec<(usize, Item, String)>,
    shell: Option<(usize, Item)>,
    last_input: Option<Value>,
    location: Option<Location>,
    session: Option<String>,
    checkpoints: HashMap<usize, String>,
    events: UnboundedSender<Event>,
    cwd: PathBuf,
    len: usize,
    streaming: HashMap<u64, (usize, String)>,
    thinking: BTreeMap<u64, (usize, String)>,
    message: Option<String>,
    phase: Phase,
    said: Vec<(usize, String)>,
    tools: Option<(usize, Vec<ToolCall>)>,
    activity: super::Activity,
    commands: HashMap<String, (usize, String, String)>,
    snippets: HashMap<String, Item>,
    media: HashSet<String>,
    delegating: HashSet<String>,
    edits: HashMap<String, bool>,
    subagents: HashMap<String, (Subagent, Transcript)>,
    delegated: HashMap<String, usize>,
    issued: HashMap<String, String>,
    asking: HashMap<String, String>,
    todos: Vec<(String, Todo)>,
    creating: HashSet<String>,
    tasks: HashMap<String, String>,
    todos_held: bool,
    history: Option<PathBuf>,
    compacted: bool,
    error_reported: bool,
    status: Status,
    replaying: bool,
    pending: Vec<Request>,
    inputs: HashMap<String, Value>,
    /// The model of the main loop's latest API call, and how many tokens it was sent.
    call: Option<(String, u64)>,
    window: Option<u64>,
    steering: HashSet<String>,
    asked: HashMap<String, Asked>,
    models: Vec<Value>,
    slash_commands: Vec<AgentCommand>,
    settings: Settings,
    applied: AgentSettings,
}

enum Asked {
    Initialize,
    Settings,
    Set(Setting, String),
}

impl Transcript {
    fn new(events: UnboundedSender<Event>, cwd: &Path) -> Self {
        Self {
            running: HashMap::new(),
            shell_context: Vec::new(),
            shell: None,
            last_input: None,
            location: None,
            session: None,
            checkpoints: HashMap::new(),
            events,
            cwd: cwd.to_owned(),
            len: 0,
            streaming: HashMap::new(),
            thinking: BTreeMap::new(),
            message: None,
            phase: Phase::Final,
            said: Vec::new(),
            tools: None,
            activity: super::Activity::default(),
            commands: HashMap::new(),
            snippets: HashMap::new(),
            media: HashSet::new(),
            delegating: HashSet::new(),
            edits: HashMap::new(),
            subagents: HashMap::new(),
            delegated: HashMap::new(),
            issued: HashMap::new(),
            asking: HashMap::new(),
            todos: Vec::new(),
            creating: HashSet::new(),
            tasks: HashMap::new(),
            todos_held: false,
            history: None,
            compacted: false,
            error_reported: false,
            status: Status::Done,
            replaying: false,
            pending: Vec::new(),
            inputs: HashMap::new(),
            call: None,
            window: None,
            steering: HashSet::new(),
            asked: HashMap::new(),
            models: Vec::new(),
            slash_commands: Vec::new(),
            settings: Settings::default(),
            applied: AgentSettings::default(),
        }
    }

    /// Asks for the commands, models and settings, which a restarted server has to ask for again.
    async fn discover(&mut self, writer: &mut OwnedWriteHalf) -> io::Result<()> {
        writer.write_all(&self.request(json!({ "subtype": "initialize" }), Asked::Initialize)).await?;
        writer.write_all(&self.request(json!({ "subtype": "get_settings" }), Asked::Settings)).await
    }

    fn request(&mut self, request: Value, asked: Asked) -> Vec<u8> {
        let id = Uuid::new_v4().to_string();
        let message = json!({ "type": "control_request", "request_id": id, "request": request });
        self.asked.insert(id, asked);
        input(&message)
    }

    fn answered(&mut self, response: &Value) {
        let Some(asked) = response["request_id"].as_str().and_then(|id| self.asked.remove(id)) else { return };
        let success = response["subtype"] == "success";
        let body = &response["response"];
        match asked {
            Asked::Initialize if success => {
                self.models = body["models"].as_array().cloned().unwrap_or_default();
                let commands = body["commands"].as_array().into_iter().flatten().filter(|command| {
                    let name = command["name"].as_str().unwrap_or_default();
                    !name.starts_with("__") && !MANAGEMENT.contains(&name)
                });
                self.slash_commands = commands
                    .map(|command| AgentCommand {
                        name: command["name"].as_str().unwrap_or_default().to_owned(),
                        description: command["description"].as_str().unwrap_or_default().to_owned(),
                        hint: command["argumentHint"].as_str().unwrap_or_default().to_owned(),
                    })
                    .collect();
                let model = self.settings.chosen.model.take().map(|model| self.alias(&model));
                self.settings.chosen.model = model;
                if let Some(mode) = body["current_permission_mode"].as_str() {
                    self.settings.chosen.mode = Some(mode.to_owned());
                }
                self.declare();
                self.report_settings();
            }
            Asked::Settings if success => {
                let applied = &body["applied"];
                self.settings.chosen.model = applied["model"].as_str().map(|model| self.alias(model));
                self.settings.chosen.effort = applied["effort"].as_str().map(str::to_owned);
                self.applied = self.settings.chosen.clone();
                self.declare();
                self.report_settings();
            }
            Asked::Set(setting, value) => {
                if success {
                    let applied = match setting {
                        Setting::Model => &mut self.applied.model,
                        Setting::Effort => &mut self.applied.effort,
                        Setting::Mode => &mut self.applied.mode,
                    };
                    *applied = Some(value.clone());
                }
                let waiting = self.asked.values().any(|asked| matches!(asked, Asked::Set(other, _) if *other == setting));
                if !waiting && self.settings.get(setting).as_deref() == Some(value.as_str()) {
                    self.settings.pending.retain(|pending| *pending != setting);
                    if !success {
                        let applied = match setting {
                            Setting::Model => &self.applied.model,
                            Setting::Effort => &self.applied.effort,
                            Setting::Mode => &self.applied.mode,
                        };
                        *self.settings.get_mut(setting) = applied.clone();
                    }
                    self.changed(setting);
                }
            }
            _ => {}
        }
    }

    fn alias(&self, model: &str) -> String {
        model_alias(&self.models, model)
    }

    fn changed(&self, setting: Setting) {
        if setting == Setting::Model {
            self.declare();
        }
        self.report_settings();
    }

    fn declare(&self) {
        let model = self.settings.chosen.model.as_deref().unwrap_or("default");
        let listed = self.models.iter().find(|listed| listed["value"] == model);
        let efforts = listed.and_then(|listed| listed["supportedEffortLevels"].as_array()).into_iter().flatten();
        let capabilities = Capabilities {
            rewinds_files: true,
            models: self.models.iter().filter_map(|model| model["value"].as_str()).map(str::to_owned).collect(),
            efforts: efforts.filter_map(Value::as_str).map(str::to_owned).collect(),
            modes: Agent::ClaudeCode.modes().iter().map(|mode| Choice::mode(mode)).collect(),
            commands: self.slash_commands.clone(),
        };
        let _ = self.events.send(Event::Capabilities(capabilities));
    }

    fn report_settings(&self) {
        let _ = self.events.send(Event::Settings(self.settings.clone()));
    }

    fn observe(&mut self, mode: &str) {
        if !self.settings.pending.contains(&Setting::Mode) && self.settings.chosen.mode.as_deref() != Some(mode) {
            self.settings.chosen.mode = Some(mode.to_owned());
            self.applied.mode = Some(mode.to_owned());
            self.report_settings();
        }
    }

    fn delegated(&mut self, item: Item) {
        let Item::Subagent { id, .. } = &item else { return };
        if let Some(&index) = self.delegated.get(id) {
            let _ = self.events.send(Event::Item(index, item));
        } else {
            let id = id.clone();
            let index = self.push(item);
            self.delegated.insert(id, index);
        }
    }

    fn push(&mut self, item: Item) -> usize {
        let index = self.len;
        self.len += 1;
        self.tools = None;
        let _ = self.events.send(Event::Item(index, item));
        index
    }

    fn report(&mut self, status: Status) {
        if status == Status::Done { self.activity.clear(&self.events); }
        self.status = status;
        if !self.replaying {
            let _ = self.events.send(Event::Status(status));
        }
    }

    fn ask(&mut self, request: Request) {
        if self.replaying {
            self.pending.push(request);
        } else {
            let _ = self.events.send(Event::Request(request));
        }
    }

    fn resolve(&mut self, id: &str) {
        self.unblock(id);
        if self.replaying {
            self.pending.retain(|request| request.id() != id);
        } else {
            let _ = self.events.send(Event::Resolved(id.to_owned()));
        }
    }

    fn unblock(&mut self, request: &str) {
        if let Some(subagent) = self.asking.remove(request)
            && !self.asking.values().any(|other| *other == subagent)
        {
            self.subagent_status(&subagent, Status::Working);
        }
    }

    fn subagent_status(&mut self, id: &str, status: Status) -> bool {
        for (key, (subagent, transcript)) in &mut self.subagents {
            let changed = if key == id {
                let changed = subagent.status != Status::Done && subagent.status != status;
                if changed { subagent.status = status; }
                changed
            } else {
                transcript.subagent_status(id, status) && subagent.update()
            };
            if changed {
                let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
                return true;
            }
        }
        false
    }

    fn report_todos(&mut self) {
        self.todos_held = self.replaying;
        if !self.replaying {
            let todos = self.todos.iter().map(|(_, todo)| todo.clone()).collect();
            let _ = self.events.send(Event::Todos(todos));
        }
    }

    fn checkpoint(&mut self, message: &Value) {
        if let Some(id) = message["uuid"].as_str() { self.checkpoints.insert(self.len, id.to_owned()); }
    }

    fn sent(&mut self, message: &Value) {
        let content = &message["message"]["content"];
        if let Some(text) = content.as_str().map(str::to_owned).or_else(|| content.as_array().map(|blocks| {
            blocks.iter().filter_map(|block| block["text"].as_str()).collect::<Vec<_>>().join("\n")
        })) {
            self.last_input = Some(content.clone());
            let images = content.as_array().into_iter().flatten().filter(|block| block["type"] == "image").count();
            let _ = self.events.send(Event::ResumeAt(None));
            let (parts, typed) = shell::parse(&text);
            if self.replaying { self.shells(parts); }
            self.checkpoint(message);
            self.push(Item::User { text: typed.to_owned(), images, image_urls: image_urls(content), template: None });
            if message.get("priority").is_some()
                && let Some(uuid) = message["uuid"].as_str()
            {
                self.steering.insert(uuid.to_owned());
            }
            self.report(Status::Working);
        }
    }

    /// Runs `history` on a blocking thread, since a long session takes a while to read.
    async fn replay(mut self, env: &BTreeMap<String, String>, session: &str, until: Option<&str>) -> io::Result<Self> {
        let (env, session, until) = (env.clone(), session.to_owned(), until.map(str::to_owned));
        tokio::task::spawn_blocking(move || {
            self.history(&env, &session, until.as_deref());
            self
        })
        .await
        .map_err(io::Error::other)
    }

    /// Translates the session file's entries, up to the one whose uuid is `until`. A first pass keeps only how
    /// the entries chain, so a long session is never held in memory whole.
    fn history(&mut self, env: &BTreeMap<String, String>, session: &str, until: Option<&str>) {
        let Some(path) = session_file(env, session) else { return };
        let Ok(file) = File::open(&path) else { return };
        self.history = Some(path.clone());
        let links: Vec<Option<Link>> = lines(file).map(|line| serde_json::from_str(&line).ok()).collect();
        let entries = links.iter().enumerate().filter_map(|(index, link)| Some((index, link.as_ref()?)));
        let chained: HashMap<&str, &Link> = entries.clone().filter(|(_, link)| !link.sidechain).filter_map(|(_, link)| Some((link.uuid.as_deref()?, link))).collect();
        let leaf = entries.clone().rev().find(|(_, link)| link.turn() && !link.sidechain && link.parent.is_some());
        let mut branch = HashSet::new();
        let mut current = leaf.and_then(|(_, link)| link.uuid.as_deref());
        let mut complete = false;
        while let Some(id) = current {
            if !branch.insert(id) { break; }
            let Some(link) = chained.get(id) else { break };
            current = link.parent.as_ref().and_then(Option::as_deref);
            if link.parent == Some(None) { complete = true; }
        }
        let boundary = entries.clone().rev().find(|(_, link)| link.compact()).map(|(index, _)| index);
        let active_tools: HashSet<&str> = entries.filter(|(_, link)| link.uuid.as_deref().is_some_and(|id| branch.contains(id)))
            .flat_map(|(_, link)| link.tool_uses()).collect();
        let Ok(file) = File::open(&path) else { return };
        for (index, line) in lines(file).enumerate().take(links.len()) {
            let Some(link) = &links[index] else { continue };
            if complete && boundary.is_none_or(|boundary| index > boundary) && !branch.is_empty() && link.turn()
                && link.uuid.as_deref().is_some_and(|id| !branch.contains(id))
                && !link.tool_results().any(|id| active_tools.contains(id)) { continue; }
            if until.is_some() && link.uuid.as_deref() == until {
                // A compaction recorded just before the replayed turn's prompt belongs to that turn, which replays it.
                self.compacted = false;
                break;
            }
            let Some(entry) = entry(&line) else { continue };
            self.recorded(&entry);
        }
        self.history = None;
        self.compact();
    }

    /// A compaction is recorded before the prompt that asked for it, so its notice waits for that prompt.
    fn recorded(&mut self, entry: &Value) {
        self.task_notification(entry);
        if entry["isSidechain"] == true || entry["isMeta"] == true || entry["isCompactSummary"] == true {
            return;
        }
        match entry["type"].as_str() {
            Some("system") if entry["subtype"] == "compact_boundary" => self.compacted = true,
            Some("user") if entry["promptSource"] != "system" => match entry["message"]["content"].as_str() {
                Some(content) => {
                    let (parts, typed) = shell::parse(content);
                    let shelled = !parts.is_empty();
                    self.shells(parts);
                    if let Some(text) = prompt(typed).filter(|text| !shelled || !text.is_empty()) {
                        self.last_input = Some(entry["message"]["content"].clone());
                        self.checkpoint(entry);
                        self.push(Item::User { text, images: 0, image_urls: Vec::new(), template: None });
                        self.compact();
                    }
                }
                None => {
                    let blocks = entry["message"]["content"].as_array().into_iter().flatten();
                    let text = blocks.clone().filter_map(|block| block["text"].as_str()).collect::<Vec<_>>().join("\n");
                    let (parts, typed) = shell::parse(&text);
                    self.shells(parts);
                    let text = typed.to_owned();
                    let images = blocks.filter(|block| block["type"] == "image").count();
                    if !text.is_empty() || images > 0 { self.last_input = Some(entry["message"]["content"].clone()); self.checkpoint(entry); self.push(Item::User { text, images, image_urls: image_urls(&entry["message"]["content"]), template: None }); }
                    self.compact();
                    self.results(&entry["message"]["content"], &entry["toolUseResult"]);
                }
            },
            Some("attachment") if entry["attachment"]["type"] == "queued_command" && entry["renderedRole"] != "system" => {
                if let Some(text) = entry["attachment"]["prompt"].as_str() {
                    self.push(Item::User { text: text.to_owned(), images: 0, image_urls: Vec::new(), template: None });
                }
            }
            Some("assistant") => {
                self.compact();
                self.assistant(entry, &entry["thinkingDurationMs"], false);
            }
            _ => {}
        }
    }

    fn run_shell(&mut self, command: String, progress: &UnboundedSender<shell::Progress>) {
        let item = Item::Shell { command: command.clone(), status: CommandStatus::Running, output: String::new(), pending: false };
        let index = self.push(item.clone());
        let env = self.location.as_ref().map(|location| location.env.clone()).unwrap_or_default();
        match shell::spawn(index, &command, &self.cwd, &env, progress.clone()) {
            Ok(stop) => {
                self.running.insert(index, Running { item, stdout: shell::Capture::default(), stderr: shell::Capture::default(), stop: Some(stop) });
            }
            Err(error) => {
                let _ = self.events.send(Event::Item(index, Item::Shell { command, status: CommandStatus::Failed, output: error.to_string(), pending: false }));
            }
        }
    }

    fn shell_progress(&mut self, progress: shell::Progress) {
        match progress {
            shell::Progress::Output { index, text, error } => {
                let Some(running) = self.running.get_mut(&index) else { return };
                if error { running.stderr.push(&text) } else { running.stdout.push(&text) }
                if let Item::Shell { output, .. } = &mut running.item && output.len() < shell::SHOWN {
                    output.push_str(&text);
                    if output.len() >= shell::SHOWN { output.push_str("\n… output truncated …"); }
                    let _ = self.events.send(Event::Item(index, running.item.clone()));
                }
            }
            shell::Progress::Exit { index, code } => {
                let Some(Running { mut item, stdout, stderr, .. }) = self.running.remove(&index) else { return };
                let Item::Shell { command, status, pending, .. } = &mut item else { return };
                *status = if code == Some(0) { CommandStatus::Ok } else { CommandStatus::Failed };
                *pending = true;
                let block = shell::block(command, &stdout.text(), &stderr.text(), code);
                let _ = self.events.send(Event::Item(index, item.clone()));
                self.shell_context.push((index, item, block));
                self.report_shell_context();
            }
        }
    }

    fn stop_shells(&mut self) {
        for running in self.running.values_mut() {
            if let Some(stop) = running.stop.take() { let _ = stop.send(()); }
        }
    }

    fn stop_commands(&mut self) {
        for (id, (index, command, output)) in self.commands.drain() {
            self.activity.finish(&id, &self.events);
            let _ = self.events.send(Event::Item(index, Item::Command { command, status: CommandStatus::Failed, output }));
        }
        for (subagent, transcript) in self.subagents.values_mut() {
            transcript.stop_commands();
            if subagent.update() { let _ = self.events.send(Event::Item(subagent.index, subagent.item())); }
        }
    }

    fn report_shell_context(&self) {
        let _ = self.events.send(Event::ShellContext(self.shell_context.iter().map(|(_, _, block)| block.clone()).collect()));
    }

    fn take_shell_context(&mut self) -> Option<String> {
        if self.shell_context.is_empty() { return None; }
        let blocks = std::mem::take(&mut self.shell_context).into_iter().map(|(index, mut item, block)| {
            if let Item::Shell { pending, .. } = &mut item { *pending = false; }
            let _ = self.events.send(Event::Item(index, item));
            block
        }).collect::<Vec<_>>();
        self.report_shell_context();
        Some(blocks.join("\n"))
    }

    /// Claude Code records a `!` command and its output as separate prompts, so output fills the latest command.
    fn shells(&mut self, parts: Vec<shell::Part>) {
        for part in parts {
            if let shell::Part::Input(command) = part {
                let item = Item::Shell { command, status: CommandStatus::Ok, output: String::new(), pending: false };
                self.shell = Some((self.push(item.clone()), item));
                continue;
            }
            let Some((index, Item::Shell { output, status, .. })) = &mut self.shell else { continue };
            match part {
                shell::Part::Output(text) => output.push_str(&text),
                _ => *status = CommandStatus::Failed,
            }
            let (index, item) = (*index, self.shell.as_ref().unwrap().1.clone());
            let _ = self.events.send(Event::Item(index, item));
        }
    }

    fn restore_shell_context(&mut self, blocks: &[String]) {
        for block in blocks {
            self.shells(shell::parse(block).0);
            let Some((index, mut item)) = self.shell.take() else { continue };
            if let Item::Shell { pending, .. } = &mut item { *pending = true; }
            let _ = self.events.send(Event::Item(index, item.clone()));
            self.shell_context.push((index, item, block.clone()));
        }
    }

    fn compact(&mut self) {
        if std::mem::take(&mut self.compacted) {
            self.push(Item::compacted());
        }
    }

    fn read(&mut self, message: &Value) {
        self.task_notification(message);
        let top = message["parent_tool_use_id"].is_null();
        match message["type"].as_str() {
            Some("system") if message["subtype"] == "task_started" => {
                if let (Some(task), Some(tool)) = (message["task_id"].as_str(), message["tool_use_id"].as_str()) {
                    self.tasks.insert(task.to_owned(), tool.to_owned());
                }
            }
            Some("system") if message["subtype"] == "compact_boundary" => {
                self.push(Item::compacted());
            }
            Some("system") => {
                if message["subtype"] == "init"
                    && let Some(session) = message["session_id"].as_str()
                {
                    self.session = Some(session.to_owned());
                    let _ = self.events.send(Event::Session(session.to_owned()));
                }
                if let Some(mode) = message["permissionMode"].as_str() {
                    self.observe(mode);
                }
            }
            Some("stream_event") if top => self.stream(&message["event"]),
            Some("assistant") if top => self.assistant(message, &message["thinking_duration_ms"], true),
            Some("user") if top => self.results(&message["message"]["content"], &message["tool_use_result"]),
            Some("assistant" | "user" | "stream_event") => {
                if let Some(parent) = message["parent_tool_use_id"].as_str() {
                    for block in message["message"]["content"].as_array().into_iter().flatten().filter(|block| block["type"] == "tool_use") {
                        if let Some(tool) = block["id"].as_str() { self.issued.insert(tool.to_owned(), parent.to_owned()); }
                    }
                    self.route(parent, message);
                }
            }
            Some("command_lifecycle") if message["state"] != "queued" => {
                if let Some(uuid) = message["command_uuid"].as_str() {
                    self.steering.remove(uuid);
                }
            }
            Some("control_request") if message["request"]["subtype"] == "can_use_tool" => {
                if let Some(id) = message["request_id"].as_str() {
                    let asked = &message["request"];
                    let agent = asked["agent_id"].as_str().and_then(|agent| self.tasks.get(agent));
                    let subagent = agent.or_else(|| asked["tool_use_id"].as_str().and_then(|tool| self.issued.get(tool))).cloned();
                    if let Some(subagent) = &subagent {
                        self.asking.insert(id.to_owned(), subagent.clone());
                        self.subagent_status(subagent, Status::Waiting);
                    }
                    self.inputs.insert(id.to_owned(), asked.clone());
                    self.ask(Request { kind: request(id, asked), subagent });
                }
            }
            Some("control_cancel_request") => self.resolve(message["request_id"].as_str().unwrap_or_default()),
            Some("result") => {
                let error_reported = std::mem::take(&mut self.error_reported);
                if !error_reported && message["terminal_reason"] != "aborted_streaming"
                    && (message["is_error"] == true || message["subtype"].as_str().is_some_and(|subtype| subtype.starts_with("error")))
                {
                    let errors = message["errors"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join("\n");
                    let text = if errors.is_empty() { format!("Claude Code did not complete the turn: {}", message["subtype"].as_str().unwrap_or("error")) } else { errors };
                    self.push(Item::Notice { notice: Notice::Error, text });
                }
                self.total(&message["modelUsage"]);
                // A steer Claude has not started yet runs as a turn of its own after this result.
                if self.steering.is_empty() {
                    self.report(Status::Done);
                }
            }
            Some("rate_limit_event") if !self.replaying => {
                let windows = &message["rate_limit_info"]["unifiedWindows"];
                let readings = WINDOWS.into_iter().filter_map(|(window, name)| {
                    let used = windows[name]["utilization"].as_f64()?;
                    let resets_at = windows[name]["resetsAt"].as_u64().map(|seconds| seconds * 1000);
                    Some(Reading::new(Agent::ClaudeCode, window, used, resets_at))
                });
                let _ = self.events.send(Event::Usage(readings.collect()));
            }
            Some("control_response") => self.answered(&message["response"]),
            Some("conversation_reset") => {
                let _ = self.events.send(Event::QuestionsTruncated(0));
                self.activity.clear(&self.events);
                self.call = None;
                self.window = None;
                self.len = 0;
                self.streaming.clear();
                self.thinking.clear();
                self.tools = None;
                self.commands.clear();
                self.snippets.clear();
                self.media.clear();
                self.delegating.clear();
                self.edits.clear();
                self.subagents.clear();
                self.delegated.clear();
                self.issued.clear();
                self.asking.clear();
                self.todos.clear();
                self.creating.clear();
                self.tasks.clear();
                self.todos_held = false;
                self.pending.clear();
                self.checkpoints.clear();
                self.steering.clear();
                self.error_reported = false;
                let _ = self.events.send(Event::Reset);
                let _ = self.events.send(Event::Truncate(0));
            }
            _ => {}
        }
    }

    fn route(&mut self, parent: &str, message: &Value) -> bool {
        for (id, (subagent, transcript)) in &mut self.subagents {
            if id == parent {
                transcript.nested(message, &message["tool_use_result"]);
            } else if !transcript.route(parent, message) {
                continue;
            }
            if subagent.update() {
                let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
            }
            return true;
        }
        false
    }

    fn task_notification(&mut self, message: &Value) {
        match message["type"].as_str() {
            Some("system") if message["subtype"] == "task_updated" || message["subtype"] == "task_notification" => {
                let usage = &message["usage"];
                self.task_status(
                    message["task_id"].as_str(),
                    message["tool_use_id"].as_str(),
                    message["status"].as_str().or_else(|| message["patch"]["status"].as_str()),
                    message["summary"].as_str(),
                    [&usage["duration_ms"], &usage["tool_uses"], &usage["total_tokens"]].map(Value::as_u64),
                );
            }
            Some("user") => {
                let content = &message["message"]["content"];
                if let Some(text) = content.as_str() {
                    self.task_text(text);
                } else {
                    for block in content.as_array().into_iter().flatten() {
                        if let Some(text) = block["text"].as_str() { self.task_text(text); }
                    }
                }
            }
            Some("attachment") if message["attachment"]["type"] == "queued_command" => {
                if let Some(text) = message["attachment"]["prompt"].as_str() { self.task_text(text); }
            }
            _ => {}
        }
    }

    fn task_text(&mut self, text: &str) {
        for notification in text.split("<task-notification>").skip(1) {
            if let Some((notification, _)) = notification.split_once("</task-notification>") {
                let usage = tagged(notification, "usage").unwrap_or_default();
                let total = |tag: &str| tagged(usage, tag).and_then(|total| total.trim().parse().ok());
                self.task_status(
                    tagged(notification, "task-id"),
                    tagged(notification, "tool-use-id"),
                    tagged(notification, "status"),
                    tagged(notification, "result"),
                    [total("duration_ms"), total("tool_uses"), total("subagent_tokens").or_else(|| total("total_tokens"))],
                );
            }
        }
    }

    fn task_status(&mut self, task: Option<&str>, tool: Option<&str>, status: Option<&str>, report: Option<&str>, totals: [Option<u64>; 3]) {
        for (subagent, transcript) in self.subagents.values_mut() {
            transcript.task_status(task, tool, status, report, totals);
            if subagent.update() {
                let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
            }
        }
        let tool = tool.filter(|tool| self.subagents.contains_key(*tool) || self.commands.contains_key(*tool))
            .or_else(|| task.and_then(|task| self.tasks.get(task).map(String::as_str))).map(str::to_owned);
        if let Some(outcome) = outcome(status) {
            if let Some((subagent, _)) = tool.as_ref().and_then(|tool| self.subagents.get_mut(tool)) {
                subagent.finish(Some(outcome));
                reported(subagent, report, totals);
                let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
            }
            if let Some((index, command, output)) = tool.as_ref().and_then(|tool| self.commands.remove(tool)) {
                let status = if status == Some("completed") { CommandStatus::Ok } else { CommandStatus::Failed };
                let _ = self.events.send(Event::Item(index, Item::Command { command, status, output }));
            }
        }
    }

    fn called(&mut self, message: &Value) {
        let usage = &message["usage"];
        let sent = ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"];
        let used = sent.iter().filter_map(|field| usage[field].as_u64()).sum();
        self.call = message["model"].as_str().map(|model| (model.to_owned(), used));
        self.meter(None);
    }

    fn total(&mut self, models: &Value) {
        let Some(models) = models.as_object().filter(|models| !models.is_empty()) else { return };
        let count = |fields: &[&str]| {
            models.values().flat_map(|usage| fields.iter().filter_map(|field| usage[field].as_u64())).sum()
        };
        let input = count(&["inputTokens", "cacheReadInputTokens", "cacheCreationInputTokens"]);
        let output = count(&["outputTokens"]);
        if let Some((model, _)) = &self.call
            && let Some(window) = models.get(model).and_then(|usage| usage["contextWindow"].as_u64())
        {
            self.window = Some(window);
        }
        self.meter(Some(Tokens { input, output }));
    }

    fn meter(&self, tokens: Option<Tokens>) {
        let context = self.call.as_ref().zip(self.window).map(|((_, used), window)| Context { used: *used, window });
        if context.is_some() || tokens.is_some() {
            let _ = self.events.send(Event::Meters(context, tokens));
        }
    }

    /// Text blocks of a `streamed` message already arrived as stream events.
    fn assistant(&mut self, message: &Value, thinking: &Value, streamed: bool) {
        self.begin(&message["message"]);
        let blocks = message["message"]["content"].as_array().into_iter().flatten();
        if let Some(error) = message["error"].as_str() {
            self.error_reported |= streamed;
            let text: Vec<&str> = blocks.filter_map(|block| block["text"].as_str()).collect();
            let notice = if error == "rate_limit" { Notice::Limit } else { Notice::Error };
            self.push(Item::Notice { notice, text: text.join("\n") });
            return;
        }
        for block in blocks {
            match block["type"].as_str() {
                Some("text") if !streamed => {
                    let text = block["text"].as_str().unwrap_or_default().to_owned();
                    let index = self.push(Item::Text { text: text.clone(), streaming: false, phase: self.phase });
                    self.said.push((index, text));
                }
                Some("thinking") => {
                    let streamed = self.thinking.pop_first().map(|(_, item)| item);
                    let text = block["thinking"].as_str().filter(|text| !text.is_empty()).map(str::to_owned)
                        .or_else(|| streamed.as_ref().map(|(_, text)| text.clone()).filter(|text| !text.is_empty()));
                    let item = Item::Thinking { seconds: seconds(thinking.as_u64().unwrap_or_default()), text };
                    if let Some((index, _)) = streamed {
                        let _ = self.events.send(Event::Item(index, item));
                    } else {
                        self.push(item);
                    }
                }
                Some("tool_use") => {
                    self.phase = Phase::Commentary;
                    for (index, text) in self.said.drain(..) {
                        let _ = self.events.send(Event::Item(index, Item::Text { text, streaming: false, phase: Phase::Commentary }));
                    }
                    self.tool(block, &message["timestamp"]);
                }
                _ => {}
            }
        }
    }

    /// Only a tool call later in the same message shows that its text was said while working.
    fn begin(&mut self, message: &Value) {
        let id = message["id"].as_str();
        if id.is_none() || self.message.as_deref() != id {
            self.message = id.map(str::to_owned);
            self.phase = Phase::Final;
            self.said.clear();
            self.thinking.clear();
        }
    }

    fn tool(&mut self, block: &Value, timestamp: &Value) {
        let (Some(id), Some(name)) = (block["id"].as_str(), block["name"].as_str()) else { return };
        let input = &block["input"];
        let text = |key: &str| input[key].as_str().unwrap_or_default().to_owned();
        if !self.replaying {
            let label = match name {
                "Bash" => format!("Running {}", text("command")),
                "Read" => format!("Reading {}", self.relative(&text("file_path"))),
                "Grep" | "Glob" => format!("Searching {}", text("pattern")),
                "WebFetch" => format!("Fetching {}", text("url")),
                "WebSearch" => format!("Searching the web for {}", text("query")),
                "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => format!("Editing {}", self.relative(&text("file_path"))),
                "Agent" | "Task" => format!("Starting agent: {}", text("description")),
                _ => format!("Using {name}"),
            };
            self.activity.start(id, label, &self.events);
        }
        match name {
            super::delegation::CLAUDE_TOOL => { self.delegating.insert(id.to_owned()); }
            crate::media::CLAUDE_TOOL => { self.media.insert(id.to_owned()); }
            html::CLAUDE_TOOL => {
                if let Some(item) = html::item(input) {
                    self.snippets.insert(id.to_owned(), item);
                } else {
                    self.call(ToolKind::Mcp, name, input.to_string());
                }
            }
            "Bash" => {
                let command = text("command");
                let running = Item::Command { command: command.clone(), status: CommandStatus::Running, output: String::new() };
                let index = self.push(running);
                self.commands.insert(id.to_owned(), (index, command, String::new()));
            }
            "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => {
                self.edits.insert(id.to_owned(), name == "Write");
            }
            "Read" => self.call(ToolKind::Read, name, self.relative(&text("file_path"))),
            "Grep" | "Glob" => self.call(ToolKind::Search, name, text("pattern")),
            "WebFetch" => self.call(ToolKind::Fetch, name, text("url")),
            "WebSearch" => self.call(ToolKind::Web, name, text("query")),
            "Agent" | "Task" => {
                let replaying = self.replaying || self.history.is_some();
                let started_at = timestamp.as_str().filter(|_| replaying).and_then(super::epoch_ms).unwrap_or_else(super::now);
                let (mut subagent, events) = Subagent::new(id, text("description"), started_at);
                subagent.name = input["subagent_type"].as_str().map(str::to_owned);
                subagent.prompt = input["prompt"].as_str().map(str::to_owned);
                subagent.model = input["model"].as_str().map(str::to_owned);
                subagent.index = self.push(subagent.item());
                self.subagents.insert(id.to_owned(), (subagent, Transcript::new(events, &self.cwd)));
            }
            "TodoWrite" => {
                let todos = input["todos"].as_array().into_iter().flatten().map(|todo| {
                    let text = todo["content"].as_str().unwrap_or_default().to_owned();
                    (String::new(), Todo { text, status: todo_status(todo["status"].as_str().unwrap_or_default()) })
                });
                self.todos = todos.collect();
                self.report_todos();
            }
            "TaskCreate" => {
                self.creating.insert(id.to_owned());
            }
            "TaskUpdate" => {
                let task = input["taskId"].as_str().unwrap_or_default();
                if input["status"] == "deleted" {
                    self.todos.retain(|(id, _)| id != task);
                } else if let Some((_, todo)) = self.todos.iter_mut().find(|(id, _)| id == task) {
                    if let Some(status) = input["status"].as_str() {
                        todo.status = todo_status(status);
                    }
                    if let Some(subject) = input["subject"].as_str() {
                        todo.text = subject.to_owned();
                    }
                }
                self.report_todos();
            }
            _ if name.starts_with("mcp__") => self.call(ToolKind::Mcp, name, input.to_string()),
            _ => self.call(ToolKind::Other, name, input.to_string()),
        }
    }

    fn nested(&mut self, message: &Value, result: &Value) {
        let content = &message["message"]["content"];
        match message["type"].as_str() {
            Some("assistant") => {
                let streamed = self.message.as_deref().is_some_and(|id| message["message"]["id"] == id)
                    && (!self.said.is_empty() || !self.streaming.is_empty());
                self.assistant(message, &message["thinking_duration_ms"], streamed);
            }
            Some("stream_event") => self.stream(&message["event"]),
            Some("user") => self.results(content, result),
            _ => {}
        }
    }

    fn results(&mut self, content: &Value, result: &Value) {
        for block in content.as_array().into_iter().flatten().filter(|block| block["type"] == "tool_result") {
            let Some(id) = block["tool_use_id"].as_str() else { continue };
            if block["is_error"] != true && result["answers"].is_object()
                && let Ok(questions) = serde_json::from_value::<Vec<Question>>(result["questions"].clone())
                && !questions.is_empty()
            {
                let answers = questions.iter().map(|question| {
                    recorded_answers(question, result["answers"][&question.question].as_str().unwrap_or_default())
                }).collect();
                let record = AnsweredQuestions { source: Some(id.to_owned()), after: self.len, questions, answers, responses: Default::default() };
                let _ = self.events.send(Event::QuestionsRecovered(record));
            }
            self.activity.finish(id, &self.events);
            if block["is_error"] != true && result["task_type"] == "local_bash" {
                self.task_status(result["task_id"].as_str(), None, Some("stopped"), None, [None; 3]);
            }
            if let Some(item) = self.snippets.remove(id) {
                if block["is_error"] != true {
                    self.push(item);
                }
            }
            if self.delegating.remove(id)
                && let Some(item) = super::delegation::item(block)
            {
                if let Item::Subagent { id, .. } = &item
                    && !self.delegated.contains_key(id)
                {
                    self.delegated(item);
                }
            }
            if self.media.remove(id) && block["is_error"] != true
                && let Some(item) = crate::media::item(block)
            {
                self.push(item);
            }
            if let Some(write) = self.edits.remove(id) && block["is_error"] != true {
                self.edit(result, write);
            }
            if self.creating.remove(id)
                && let (Some(task), Some(text)) = (result["task"]["id"].as_str(), result["task"]["subject"].as_str())
            {
                self.todos.push((task.to_owned(), Todo { text: text.to_owned(), status: TodoStatus::Pending }));
                self.report_todos();
            }
            if let Some((subagent, transcript)) = self.subagents.get_mut(id) {
                if let Some(task) = result["agentId"].as_str() { self.tasks.insert(task.to_owned(), id.to_owned()); }
                if let Some(history) = &self.history
                    && let Some(agent) = result["agentId"].as_str()
                {
                    let path = history.with_extension("").join("subagents").join(format!("agent-{agent}.jsonl"));
                    transcript.history = Some(history.clone());
                    for entry in File::open(path).into_iter().flat_map(lines).filter_map(|line| entry(&line)) {
                        transcript.nested(&entry, &entry["toolUseResult"]);
                    }
                    transcript.history = None;
                }
                subagent.update();
                if let Some(model) = result["resolvedModel"].as_str() {
                    subagent.model = Some(model.to_owned());
                }
                if result["isAsync"] != true {
                    let texts: Vec<&str> = result["content"].as_array().into_iter().flatten().filter_map(|block| block["text"].as_str()).collect();
                    let report = Some(texts.join("\n")).filter(|report| !report.is_empty());
                    let fallback = if block["is_error"] == true { Outcome::Failed } else { Outcome::Completed };
                    subagent.finish(Some(outcome(result["status"].as_str()).unwrap_or(fallback)));
                    let totals = [&result["totalDurationMs"], &result["totalToolUseCount"], &result["totalTokens"]].map(Value::as_u64);
                    reported(subagent, report.as_deref(), totals);
                }
                let _ = self.events.send(Event::Item(subagent.index, subagent.item()));
            }
            let Some((index, command, _)) = self.commands.remove(id) else {
                continue;
            };
            let output = match &block["content"] {
                Value::String(text) => text.clone(),
                blocks => {
                    let texts: Vec<&str> = blocks.as_array().into_iter().flatten().filter_map(|block| block["text"].as_str()).collect();
                    texts.join("\n")
                }
            };
            let task = result["backgroundTaskId"].as_str();
            let status = if block["is_error"] == true {
                CommandStatus::Failed
            } else if let Some(task) = task {
                self.tasks.insert(task.to_owned(), id.to_owned());
                self.commands.insert(id.to_owned(), (index, command.clone(), output.clone()));
                CommandStatus::Running
            } else {
                CommandStatus::Ok
            };
            let _ = self.events.send(Event::Item(index, Item::Command { command, status, output }));
        }
        if interrupted(content) {
            self.push(Item::interrupted());
        }
    }

    /// A created file's patch is empty, and a notebook edit reports the whole notebook before and after instead.
    fn edit(&mut self, result: &Value, write: bool) {
        let text = |key: &str| result[key].as_str().unwrap_or_default();
        let diff = match result["structuredPatch"].as_array() {
            Some(_) if result["type"] == "create" => unified("", text("content")),
            Some(hunks) => hunks.iter().map(hunk).collect(),
            None => unified(text("original_file"), text("updated_file")),
        };
        let path = result["filePath"].as_str().unwrap_or(text("notebook_path"));
        if !path.is_empty() {
            self.push(Item::edit(self.relative(path), diff, write));
        }
    }

    fn call(&mut self, kind: ToolKind, name: &str, detail: String) {
        let call = ToolCall { kind, name: name.to_owned(), detail };
        match &mut self.tools {
            Some((index, calls)) => {
                calls.push(call);
                let _ = self.events.send(Event::Item(*index, Item::Tools { calls: calls.clone() }));
            }
            None => {
                let index = self.push(Item::Tools { calls: vec![call.clone()] });
                self.tools = Some((index, vec![call]));
            }
        }
    }

    fn relative(&self, path: &str) -> String {
        super::relative(&self.cwd, path)
    }

    fn stream(&mut self, event: &Value) {
        if event["type"] == "message_start" {
            self.begin(&event["message"]);
            return self.called(&event["message"]);
        }
        let Some(block) = event["index"].as_u64() else { return };
        let activity_id = format!("block:{block}");
        if event["type"] == "content_block_start" {
            let content = &event["content_block"];
            match content["type"].as_str() {
                Some("thinking") => self.activity.start(&activity_id, "Thinking…".to_owned(), &self.events),
                Some("text") => self.activity.start(&activity_id, "Writing response…".to_owned(), &self.events),
                Some("tool_use") => {
                    if let (Some(id), Some(name)) = (content["id"].as_str(), content["name"].as_str()) {
                        self.activity.start(id, format!("Using {name}"), &self.events);
                    }
                }
                _ => {}
            }
        }
        if event["type"] == "content_block_stop" { self.activity.finish(&activity_id, &self.events); }
        match event["type"].as_str() {
            Some("content_block_start") if event["content_block"]["type"] == "thinking" => {
                let text = event["content_block"]["thinking"].as_str().unwrap_or_default().to_owned();
                let index = self.push(Item::Thinking { seconds: 0, text: Some(text.clone()) });
                self.thinking.insert(block, (index, text));
            }
            Some("content_block_delta") if event["delta"]["type"] == "thinking_delta" => {
                if let Some((index, text)) = self.thinking.get_mut(&block) {
                    text.push_str(event["delta"]["thinking"].as_str().unwrap_or_default());
                    let _ = self.events.send(Event::Item(*index, Item::Thinking { seconds: 0, text: Some(text.clone()) }));
                }
            }
            Some("content_block_start") if event["content_block"]["type"] == "text" => {
                let text = event["content_block"]["text"].as_str().unwrap_or_default().to_owned();
                let index = self.push(Item::Text { text: text.clone(), streaming: true, phase: self.phase });
                self.streaming.insert(block, (index, text));
            }
            Some("content_block_delta") if event["delta"]["type"] == "text_delta" => {
                if let Some((index, text)) = self.streaming.get_mut(&block) {
                    text.push_str(event["delta"]["text"].as_str().unwrap_or_default());
                    let item = Item::Text { text: text.clone(), streaming: true, phase: self.phase };
                    let _ = self.events.send(Event::Item(*index, item));
                }
            }
            Some("content_block_stop") => {
                if let Some((index, text)) = self.streaming.remove(&block) {
                    let _ = self.events.send(Event::Item(index, Item::Text { text: text.clone(), streaming: false, phase: self.phase }));
                    self.said.push((index, text));
                }
            }
            _ => {}
        }
    }
}

fn hunk(hunk: &Value) -> String {
    let count = |key: &str| hunk[key].as_u64().unwrap_or_default();
    let mut text = format!("@@ -{},{} +{},{} @@\n", count("oldStart"), count("oldLines"), count("newStart"), count("newLines"));
    for line in hunk["lines"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        text.push_str(line);
        text.push('\n');
    }
    text
}

fn outcome(status: Option<&str>) -> Option<Outcome> {
    match status? {
        "completed" => Some(Outcome::Completed),
        "failed" => Some(Outcome::Failed),
        "stopped" | "killed" => Some(Outcome::Stopped),
        _ => None,
    }
}

fn reported(subagent: &mut Subagent, report: Option<&str>, [duration, tools, tokens]: [Option<u64>; 3]) {
    if let Some(report) = report {
        subagent.report = Some(report.to_owned());
    }
    subagent.duration = duration.or(subagent.duration);
    subagent.tools = tools.map(|tools| tools as usize).or(subagent.tools);
    subagent.tokens = tokens.or(subagent.tokens);
}

fn todo_status(status: &str) -> TodoStatus {
    match status {
        "in_progress" => TodoStatus::Active,
        "completed" => TodoStatus::Done,
        _ => TodoStatus::Pending,
    }
}

/// The prompt a session-file user entry records: slash commands are stored as tags, and their output is not a prompt.
fn prompt(content: &str) -> Option<String> {
    if content.starts_with("<local-command-") {
        return None;
    }
    if !content.starts_with("<command-") {
        return Some(content.to_owned());
    }
    let tag = |name: &str| {
        let start = content.find(&format!("<{name}>"))? + name.len() + 2;
        let end = content[start..].find(&format!("</{name}>"))?;
        Some(content[start..start + end].trim())
    };
    let name = tag("command-name")?;
    Some(match tag("command-args") {
        Some(args) if !args.is_empty() => format!("{name} {args}"),
        _ => name.to_owned(),
    })
}

#[cfg(test)]
mod result_tests {
    use super::*;

    #[test]
    fn api_errors_are_reported_once_without_hiding_later_failures() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new(events, Path::new("/project"));
        transcript.read(&json!({
            "type": "assistant", "error": "model_not_found",
            "message": { "content": [{ "type": "text", "text": "Unknown model" }] },
        }));
        transcript.read(&json!({
            "type": "result", "subtype": "success", "is_error": true,
            "terminal_reason": "api_error", "result": "Unknown model",
        }));
        transcript.read(&json!({
            "type": "result", "subtype": "error_during_execution", "is_error": true,
            "errors": ["Next turn failed"],
        }));
        let mut errors = Vec::new();
        while let Ok(event) = received.try_recv() {
            if let Event::Item(_, Item::Notice { notice: Notice::Error, text }) = event {
                errors.push(text);
            }
        }
        assert_eq!(errors, ["Unknown model", "Next turn failed"]);
    }

    #[test]
    fn aborted_streams_finish_without_error_notices() {
        for reason in [Value::Null, json!("aborted_streaming"), json!("error")] {
            let (events, mut received) = mpsc::unbounded_channel();
            let mut transcript = Transcript::new(events, Path::new("/project"));
            transcript.read(&json!({
                "type": "result", "subtype": "error_during_execution", "is_error": true,
                "terminal_reason": reason, "errors": ["diagnostic"],
            }));
            let mut errors = Vec::new();
            let mut done = false;
            while let Ok(event) = received.try_recv() {
                match event {
                    Event::Item(_, Item::Notice { notice: Notice::Error, text }) => errors.push(text),
                    Event::Status(Status::Done) => done = true,
                    _ => {}
                }
            }
            assert!(done);
            let expected = if reason == "aborted_streaming" { vec![] } else { vec!["diagnostic"] };
            assert_eq!(errors, expected, "{reason}");
        }
    }
}

#[cfg(test)]
mod subagent_tests {
    use super::*;

    #[test]
    fn subagents_stream_partial_text_and_thinking_without_duplicates() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new(events, Path::new("/project"));
        transcript.read(&json!({ "type": "assistant", "message": { "content": [{
            "type": "tool_use", "id": "child", "name": "Agent", "input": { "description": "Review" },
        }] } }));
        for event in [
            json!({ "type": "message_start", "message": { "id": "reply" } }),
            json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "thinking", "thinking": "" } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "thinking_delta", "thinking": "Checking the implementation." } }),
            json!({ "type": "content_block_stop", "index": 0 }),
            json!({ "type": "content_block_start", "index": 1, "content_block": { "type": "text", "text": "" } }),
            json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "text_delta", "text": "The first finding" } }),
        ] {
            transcript.read(&json!({ "type": "stream_event", "parent_tool_use_id": "child", "event": event }));
        }
        let items = std::iter::from_fn(|| received.try_recv().ok()).filter_map(|event| match event {
            Event::Item(_, Item::Subagent { status: Status::Working, items, .. }) => Some(items),
            _ => None,
        }).last().unwrap();
        assert!(items.iter().any(|item| matches!(item, Item::Thinking { text, .. } if text.as_deref() == Some("Checking the implementation."))), "{items:?}");
        assert!(items.iter().any(|item| matches!(item, Item::Text { text, streaming: true, .. } if text == "The first finding")), "{items:?}");
        transcript.read(&json!({ "type": "stream_event", "parent_tool_use_id": "child", "event": { "type": "content_block_stop", "index": 1 } }));
        transcript.read(&json!({ "type": "assistant", "parent_tool_use_id": "child", "message": { "id": "reply", "content": [
            { "type": "thinking", "thinking": "Checking the implementation." },
            { "type": "text", "text": "The first finding" },
        ] } }));
        let items = &transcript.subagents["child"].0.items;
        assert_eq!(items.iter().filter(|item| matches!(item, Item::Thinking { .. })).count(), 1);
        assert_eq!(items.iter().filter(|item| matches!(item, Item::Text { .. })).count(), 1);
    }

    #[test]
    fn subagents_publish_text_and_thinking_while_working() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new(events, Path::new("/project"));
        transcript.read(&json!({ "type": "assistant", "message": { "content": [{
            "type": "tool_use", "id": "child", "name": "Agent", "input": { "description": "Review" },
        }] } }));
        transcript.read(&json!({ "type": "assistant", "parent_tool_use_id": "child", "message": { "content": [
            { "type": "thinking", "thinking": "Checking the implementation." },
            { "type": "text", "text": "Reading the relevant files." },
            { "type": "tool_use", "id": "read", "name": "Read", "input": { "file_path": "/project/main.rs" } },
        ] } }));
        let items = std::iter::from_fn(|| received.try_recv().ok()).filter_map(|event| match event {
            Event::Item(_, Item::Subagent { status: Status::Working, items, .. }) => Some(items),
            _ => None,
        }).last().unwrap();
        assert!(items.iter().any(|item| matches!(item, Item::Thinking { text, .. } if text.as_deref() == Some("Checking the implementation."))), "{items:?}");
        assert!(items.iter().any(|item| matches!(item, Item::Text { text, .. } if text == "Reading the relevant files.")), "{items:?}");
        assert!(items.iter().any(|item| matches!(item, Item::Tools { calls } if calls.iter().any(|call| call.name == "Read"))), "{items:?}");
    }

    #[tokio::test]
    async fn shell_output_waits_for_the_next_prompt_and_returns_from_history() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new(events, Path::new("/"));
        let (progress, mut progressed) = mpsc::unbounded_channel();
        transcript.run_shell("printf out; printf '<err>' >&2; exit 3".to_owned(), &progress);
        while !transcript.running.is_empty() {
            transcript.shell_progress(progressed.recv().await.unwrap());
        }
        let blocks = transcript.take_shell_context().unwrap();
        assert!(blocks.contains("<bash-stderr>&lt;err&gt;</bash-stderr><bash-exit-code>3</bash-exit-code>"), "{blocks}");
        let (mut items, mut contexts) = (BTreeMap::new(), Vec::new());
        while let Ok(event) = received.try_recv() {
            match event {
                Event::Item(index, item) => { items.insert(index, item); }
                Event::ShellContext(blocks) => contexts.push(blocks.len()),
                _ => {}
            }
        }
        assert_eq!(contexts, [1, 0]);
        assert!(matches!(&items[&0], Item::Shell { status: CommandStatus::Failed, output, pending: false, .. } if output.contains("out") && output.contains("<err>")));

        let (events, mut received) = mpsc::unbounded_channel();
        let mut history = Transcript::new(events, Path::new("/"));
        for content in [format!("{blocks}\n\nWhy?"), "<bash-input> ls</bash-input>".to_owned(), "<bash-stdout>a &gt; b</bash-stdout><bash-stderr></bash-stderr>".to_owned()] {
            history.recorded(&json!({ "type": "user", "message": { "content": content } }));
        }
        let mut items = BTreeMap::new();
        while let Ok(event) = received.try_recv() {
            if let Event::Item(index, item) = event { items.insert(index, item); }
        }
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[&0], Item::Shell { command, status: CommandStatus::Failed, output, .. } if command.starts_with("printf out") && output.ends_with("<err>")));
        assert!(matches!(&items[&1], Item::User { text, .. } if text == "Why?"));
        assert!(matches!(&items[&2], Item::Shell { command, status: CommandStatus::Ok, output, .. } if command == "ls" && output == "a > b"));
    }

    #[test]
    fn background_shells_remain_running_until_notified_live_and_from_history() {
        for recorded in [false, true] {
            for status in ["completed", "failed", "stopped", "killed"] {
                let (events, mut received) = mpsc::unbounded_channel();
                let mut transcript = Transcript::new(events, Path::new("/project"));
                let assistant = json!({ "type": "assistant", "message": { "content": [{
                    "type": "tool_use", "id": "shell", "name": "Bash", "input": { "command": "pnpm dev", "run_in_background": true },
                }] } });
                let mut result = json!({ "type": "user", "message": { "content": [{
                    "type": "tool_result", "tool_use_id": "shell", "content": "Server starting",
                }] } });
                result[if recorded { "toolUseResult" } else { "tool_use_result" }] = json!({ "backgroundTaskId": "task" });
                for entry in [assistant, result] {
                    if recorded { transcript.recorded(&entry); } else { transcript.read(&entry); }
                }
                let mut items = BTreeMap::new();
                while let Ok(event) = received.try_recv() {
                    if let Event::Item(index, item) = event { items.insert(index, item); }
                }
                assert!(matches!(items.get(&0), Some(Item::Command { status: CommandStatus::Running, output, .. }) if output == "Server starting"));
                let notification = json!({ "type": "user", "promptSource": "system", "message": { "content": format!(
                    "<task-notification><task-id>task</task-id><status>{status}</status></task-notification>"
                ) } });
                if recorded { transcript.recorded(&notification); } else { transcript.read(&notification); }
                while let Ok(event) = received.try_recv() {
                    if let Event::Item(index, item) = event { items.insert(index, item); }
                }
                let expected = if status == "completed" { CommandStatus::Ok } else { CommandStatus::Failed };
                assert!(matches!(items.get(&0), Some(Item::Command { status, output, .. }) if *status == expected && output == "Server starting"));
                assert_eq!(items.len(), 1);
                assert!(transcript.commands.is_empty());
            }
        }
    }

    #[test]
    fn task_stop_finishes_only_the_stopped_shell_after_success() {
        for recorded in [false, true] {
            for error in [true, false] {
                let (events, mut received) = mpsc::unbounded_channel();
                let mut transcript = Transcript::new(events, Path::new("/project"));
                for id in ["finished", "active"] {
                    let assistant = json!({ "type": "assistant", "message": { "content": [{
                        "type": "tool_use", "id": id, "name": "Bash", "input": { "command": id, "run_in_background": true },
                    }] } });
                    let mut result = json!({ "type": "user", "message": { "content": [{
                        "type": "tool_result", "tool_use_id": id, "content": "Ready",
                    }] } });
                    result[if recorded { "toolUseResult" } else { "tool_use_result" }] = json!({ "backgroundTaskId": format!("task-{id}") });
                    for entry in [assistant, result] {
                        if recorded { transcript.recorded(&entry); } else { transcript.read(&entry); }
                    }
                }
                let mut result = json!({ "type": "user", "message": { "content": [{
                    "type": "tool_result", "tool_use_id": "stop", "is_error": error, "content": "TaskStop result",
                }] } });
                result[if recorded { "toolUseResult" } else { "tool_use_result" }] = json!({
                    "message": "Successfully stopped task: task-finished (finished)",
                    "task_id": "task-finished", "task_type": "local_bash", "command": "finished",
                });
                if recorded { transcript.recorded(&result); } else { transcript.read(&result); }
                let mut items = BTreeMap::new();
                while let Ok(event) = received.try_recv() {
                    if let Event::Item(index, item) = event { items.insert(index, item); }
                }
                let expected = if error { CommandStatus::Running } else { CommandStatus::Failed };
                assert!(matches!(&items[&0], Item::Command { status, output, .. } if *status == expected && output == "Ready"));
                assert!(matches!(&items[&1], Item::Command { status: CommandStatus::Running, .. }));
            }
        }
    }

    #[test]
    fn background_completion_is_restored_without_hiding_active_agents() {
        for status in ["completed", "failed", "stopped", "killed"] {
            for recorded in [false, true] {
                for shape in ["system", "updated", "text", "blocks", "attachment"] {
                    let (events, mut received) = mpsc::unbounded_channel();
                    let mut transcript = Transcript::new(events, Path::new("/project"));
                    let mut entries = Vec::new();
                    for id in ["finished", "active"] {
                        entries.push(json!({ "type": "assistant", "message": { "content": [{
                            "type": "tool_use", "id": id, "name": "Agent", "input": { "description": id },
                        }] } }));
                        let mut result = json!({ "type": "user", "message": { "content": [{
                            "type": "tool_result", "tool_use_id": id, "content": "Async agent launched successfully.",
                        }] } });
                        result[if recorded { "toolUseResult" } else { "tool_use_result" }] = json!({ "isAsync": true, "agentId": format!("agent-{id}") });
                        entries.push(result);
                    }
                    let notification = format!("<task-notification>\n<task-id>agent-finished</task-id>\n<status>{status}</status>\n</task-notification>");
                    entries.push(match shape {
                        "system" => json!({ "type": "system", "subtype": "task_notification", "task_id": "agent-finished", "tool_use_id": "resumed-tool", "status": status }),
                        "updated" => json!({ "type": "system", "subtype": "task_updated", "tool_use_id": "finished", "patch": { "status": status } }),
                        "text" => json!({ "type": "user", "promptSource": "system", "message": { "content": notification } }),
                        "blocks" => json!({ "type": "user", "promptSource": "system", "message": { "content": [{ "type": "text", "text": notification }] } }),
                        _ => json!({ "type": "attachment", "isMeta": true, "renderedRole": "system", "attachment": { "type": "queued_command", "prompt": notification } }),
                    });
                    if recorded {
                        let directory = tempfile::tempdir().unwrap();
                        let project = directory.path().join("projects/test");
                        fs::create_dir_all(&project).unwrap();
                        fs::write(project.join("session.jsonl"), entries.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap();
                        let env = BTreeMap::from([("CLAUDE_CONFIG_DIR".to_owned(), directory.path().to_str().unwrap().to_owned())]);
                        transcript.history(&env, "session", None);
                    } else {
                        for entry in entries { transcript.read(&entry); }
                    }
                    let mut items = BTreeMap::new();
                    while let Ok(event) = received.try_recv() {
                        if let Event::Item(index, item) = event { items.insert(index, item); }
                    }
                    let statuses: Vec<_> = items.values().filter_map(|item| match item {
                        Item::Subagent { task, status, outcome, .. } => Some((task.as_str(), *status, *outcome)),
                        _ => None,
                    }).collect();
                    let outcome = match status {
                        "completed" => Outcome::Completed,
                        "failed" => Outcome::Failed,
                        _ => Outcome::Stopped,
                    };
                    assert_eq!(statuses, [("finished", Status::Done, Some(outcome)), ("active", Status::Working, None)], "{status}, recorded: {recorded}, {shape}");
                    assert!(!items.values().any(|item| matches!(item, Item::User { .. })));
                }
            }
        }
    }

    #[test]
    fn lost_commands_stop_in_main_and_nested_transcripts() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new(events, Path::new("/project"));
        for message in [
            json!({ "type": "assistant", "message": { "content": [
                { "type": "tool_use", "id": "main", "name": "Bash", "input": { "command": "pnpm dev" } },
                { "type": "tool_use", "id": "child", "name": "Agent", "input": { "description": "Build" } },
            ] } }),
            json!({ "type": "user", "tool_use_result": { "backgroundTaskId": "task" }, "message": { "content": [
                { "type": "tool_result", "tool_use_id": "main", "content": "Ready" },
            ] } }),
            json!({ "type": "assistant", "parent_tool_use_id": "child", "message": { "content": [
                { "type": "tool_use", "id": "nested", "name": "Bash", "input": { "command": "pnpm watch" } },
            ] } }),
        ] { transcript.read(&message); }
        transcript.stop_commands();
        let mut items = BTreeMap::new();
        while let Ok(event) = received.try_recv() {
            if let Event::Item(index, item) = event { items.insert(index, item); }
        }
        assert!(matches!(&items[&0], Item::Command { status: CommandStatus::Failed, output, .. } if output == "Ready"));
        assert!(matches!(&items[&1], Item::Subagent { items, .. } if matches!(items[0], Item::Command { status: CommandStatus::Failed, .. })));
        assert!(transcript.commands.is_empty());
        assert!(transcript.subagents["child"].1.commands.is_empty());
    }

    #[test]
    fn nested_subagents_fill_and_finish_at_any_depth() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new(events, Path::new("/project"));
        let messages = [
            json!({ "type": "assistant", "parent_tool_use_id": null, "message": { "content": [{
                "type": "tool_use", "id": "A", "name": "Agent", "input": { "description": "outer" },
            }] } }),
            json!({ "type": "assistant", "parent_tool_use_id": "A", "message": { "content": [{
                "type": "tool_use", "id": "B", "name": "Agent", "input": { "description": "inner" },
            }] } }),
            json!({ "type": "assistant", "parent_tool_use_id": "B", "message": { "content": [{
                "type": "tool_use", "id": "read", "name": "Read", "input": { "file_path": "/project/notes.txt" },
            }] } }),
            json!({ "type": "user", "parent_tool_use_id": "B", "message": { "content": [{
                "type": "tool_result", "tool_use_id": "read", "content": "notes",
            }] } }),
            json!({ "type": "user", "parent_tool_use_id": "A", "message": { "content": [{
                "type": "tool_result", "tool_use_id": "B", "content": [{ "type": "text", "text": "Found." }],
            }] }, "tool_use_result": {
                "status": "completed", "content": [{ "type": "text", "text": "Found." }],
                "totalDurationMs": 1000, "totalToolUseCount": 1, "totalTokens": 500,
            } }),
        ];
        for message in messages { transcript.read(&message); }
        let outer = std::iter::from_fn(|| received.try_recv().ok()).filter_map(|event| match event {
            Event::Item(_, Item::Subagent { id, items, .. }) if id == "A" => Some(items),
            _ => None,
        }).last().unwrap();
        let [Item::Subagent { id, status, outcome, items, .. }] = outer.as_slice() else { panic!("{outer:?}") };
        assert_eq!((id.as_str(), *status, *outcome), ("B", Status::Done, Some(Outcome::Completed)));
        let read = ToolCall { kind: ToolKind::Read, name: "Read".to_owned(), detail: "notes.txt".to_owned() };
        assert!(matches!(items.as_slice(), [Item::Tools { calls }] if calls == &[read]), "{items:?}");
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    #[test]
    fn html_renders_only_after_success_live_and_from_history() {
        let arguments = json!({ "title": "Flow", "html": "<b>Start → Finish</b>" });
        let assistant = json!({ "type": "assistant", "message": { "content": [{
            "type": "tool_use", "id": "snippet", "name": html::CLAUDE_TOOL, "input": arguments,
        }] } });
        for recorded in [false, true] {
            for denied in [false, true] {
                let (events, mut received) = mpsc::unbounded_channel();
                let mut transcript = Transcript::new(events, Path::new("/project"));
                let result = json!({ "type": "user", "message": { "content": [{
                    "type": "tool_result", "tool_use_id": "snippet", "is_error": denied, "content": "done",
                }] } });
                if recorded { transcript.recorded(&assistant); } else { transcript.read(&assistant); }
                assert!(!std::iter::from_fn(|| received.try_recv().ok()).any(|event| matches!(event, Event::Item(..))));
                if recorded { transcript.recorded(&result); } else { transcript.read(&result); }
                let items: Vec<_> = std::iter::from_fn(|| received.try_recv().ok()).filter_map(|event| match event {
                    Event::Item(index, item) => Some((index, item)),
                    _ => None,
                }).collect();
                if denied {
                    assert!(items.is_empty());
                } else {
                    assert!(matches!(items.as_slice(), [(0, item)] if Some(item) == html::item(&arguments).as_ref()));
                }
                assert!(transcript.snippets.is_empty());
            }
        }
    }

    #[test]
    fn media_renders_only_after_success_live_and_from_history_without_the_source_file() {
        let asset = json!({ "title": "Screenshot", "asset": "f382e7e5-ff76-42cc-b150-cb8b59c8aefa.png" });
        let assistant = json!({ "type": "assistant", "message": { "content": [{
            "type": "tool_use", "id": "media", "name": crate::media::CLAUDE_TOOL,
            "input": { "path": "/removed/screenshot.png", "title": "Screenshot" },
        }] } });
        for content in [json!(asset.to_string()), json!([{ "type": "text", "text": asset.to_string() }])] {
            for recorded in [false, true] {
                for denied in [false, true] {
                    let (events, mut received) = mpsc::unbounded_channel();
                    let mut transcript = Transcript::new(events, Path::new("/project"));
                    let block = json!({ "type": "tool_result", "tool_use_id": "media", "is_error": denied, "content": content });
                    let result = json!({ "type": "user", "message": { "content": [block] } });
                    if recorded { transcript.recorded(&assistant); } else { transcript.read(&assistant); }
                    assert!(!std::iter::from_fn(|| received.try_recv().ok()).any(|event| matches!(event, Event::Item(..))));
                    if recorded { transcript.recorded(&result); } else { transcript.read(&result); }
                    let items: Vec<_> = std::iter::from_fn(|| received.try_recv().ok()).filter_map(|event| match event {
                        Event::Item(_, item) => Some(item),
                        _ => None,
                    }).collect();
                    if denied { assert!(items.is_empty()); }
                    else { assert_eq!(items, [crate::media::item(&block).unwrap()]); }
                    assert!(transcript.media.is_empty());
                }
            }
        }
    }
}

#[cfg(test)]
mod phase_tests {
    use super::*;

    #[test]
    fn text_turns_to_commentary_once_its_message_calls_a_tool_live_and_from_history() {
        let entry = |id: &str, block: Value| json!({ "type": "assistant", "message": { "id": id, "content": [block] } });
        let text = |text: &str| json!({ "type": "text", "text": text });
        let tool = json!({ "type": "tool_use", "id": "read", "name": "Read", "input": { "file_path": "/project/notes.txt" } });
        let streamed = |id: &str, text: &str| {
            [
                json!({ "type": "message_start", "message": { "id": id } }),
                json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } }),
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": text } }),
                json!({ "type": "content_block_stop", "index": 0 }),
            ]
            .map(|event| json!({ "type": "stream_event", "event": event, "parent_tool_use_id": null }))
        };
        for recorded in [false, true] {
            let (events, mut received) = mpsc::unbounded_channel();
            let mut transcript = Transcript::new(events, Path::new("/project"));
            if recorded {
                for message in [entry("working", text("Reading.")), entry("working", tool.clone()), entry("answer", text("Done."))] {
                    transcript.recorded(&message);
                }
            } else {
                let messages = streamed("working", "Reading.").into_iter().chain([entry("working", tool.clone())]).chain(streamed("answer", "Done."));
                for message in messages {
                    transcript.read(&message);
                }
            }
            let mut items = BTreeMap::new();
            while let Ok(event) = received.try_recv() {
                if let Event::Item(index, item) = event {
                    items.insert(index, item);
                }
            }
            let texts: Vec<_> = items.into_values().filter_map(|item| match item {
                Item::Text { text, phase, .. } => Some((text, phase)),
                _ => None,
            }).collect();
            assert_eq!(texts, [("Reading.".to_owned(), Phase::Commentary), ("Done.".to_owned(), Phase::Final)], "recorded: {recorded}");
        }
    }
}
