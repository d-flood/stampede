use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{mpsc::{self, UnboundedReceiver, UnboundedSender}, oneshot},
    time::timeout,
};
use uuid::Uuid;

use super::{
    AgentCommand, Answer, Builtin, CLOSING, Capabilities, Chat, Choice, CommandStatus, Context, Driver, Event, Image, Item,
    Notice, Phase, Request, RequestKind, Session, Settings, Status, Tokens, ToolCall, ToolKind, delegation, html, questions,
    relative, shell, terminal_tools, unified, words,
};
use crate::{
    config::{Agent, AgentSettings},
    usage::{Group, Reading, Window},
};

const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const SIGN_IN: &str = "Google Antigravity isn't signed in. Run `agy` in a Terminal and sign in with Google.";
const AUTH_REQUIRED: i64 = -32000;

pub struct Antigravity {
    requests: UnboundedSender<Action>,
}

enum Action {
    Send(String, Vec<Image>),
    Retry,
    Interrupt,
    Answer(String, Answer),
    Run(Builtin),
    Delegated(Item),
    Close,
    Shell(String),
}

impl Driver for Antigravity {
    fn send(&self, text: &str, images: &[Image]) { let _ = self.requests.send(Action::Send(text.to_owned(), images.to_vec())); }
    fn retry(&self) { let _ = self.requests.send(Action::Retry); }
    fn interrupt(&self) { let _ = self.requests.send(Action::Interrupt); }
    fn answer(&self, id: &str, answer: Answer) { let _ = self.requests.send(Action::Answer(id.to_owned(), answer)); }
    fn run(&self, builtin: Builtin) { let _ = self.requests.send(Action::Run(builtin)); }
    fn delegated(&self, item: Item) { let _ = self.requests.send(Action::Delegated(item)); }
    fn close(&self) { let _ = self.requests.send(Action::Close); }
    fn shell(&self, command: &str) { let _ = self.requests.send(Action::Shell(command.to_owned())); }
    fn checkpoint(&self, _index: usize, reply: oneshot::Sender<Option<String>>) { let _ = reply.send(None); }
}

struct Connection {
    child: Child,
    stdin: ChildStdin,
    incoming: UnboundedReceiver<Value>,
    next: u64,
}

impl Connection {
    async fn open(cwd: &Path, env: &BTreeMap<String, String>, full_access: bool) -> io::Result<Self> {
        let mut command = Command::new(executable(env));
        command.arg("acp");
        if full_access { command.arg("--dangerously-skip-permissions"); }
        let mut child = command.current_dir(cwd).envs(env)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
            .kill_on_drop(true).spawn()
            .map_err(|error| match error.kind() {
                io::ErrorKind::NotFound => io::Error::new(error.kind(), "Google Antigravity's CLI, `agy`, isn't installed. Install it from antigravity.google, then run `agy` in a Terminal and sign in with Google."),
                _ => io::Error::new(error.kind(), format!("Cannot start Google Antigravity: {error}")),
            })?;
        let (stdin, stdout) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        let (sender, incoming) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(message) = serde_json::from_str::<Value>(&line) && sender.send(message).is_err() { break; }
            }
        });
        let mut connection = Self { child, stdin, incoming, next: 0 };
        let initialize = json!({
            "protocolVersion": 1,
            "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false },
            "clientInfo": { "name": "Stampede", "version": env!("CARGO_PKG_VERSION") },
        });
        connection.call("initialize", initialize, &mut Vec::new()).await?;
        Ok(connection)
    }

    async fn write(&mut self, mut message: Value) -> io::Result<()> {
        message["jsonrpc"] = json!("2.0");
        let mut line = serde_json::to_vec(&message)?;
        line.push(b'\n');
        self.stdin.write_all(&line).await?;
        self.stdin.flush().await
    }

    async fn request(&mut self, method: &str, params: Value) -> io::Result<u64> {
        let id = self.next;
        self.next += 1;
        self.write(json!({ "id": id, "method": method, "params": params })).await?;
        Ok(id)
    }

    async fn notify(&mut self, method: &str, params: Value) -> io::Result<()> {
        self.write(json!({ "method": method, "params": params })).await
    }

    async fn respond(&mut self, id: Value, result: Value) -> io::Result<()> {
        self.write(json!({ "id": id, "result": result })).await
    }

    /// Waits for the answer to `method`, keeping whatever arrives meanwhile in `held`.
    async fn call(&mut self, method: &str, params: Value, held: &mut Vec<Value>) -> io::Result<Value> {
        let id = self.request(method, params).await?;
        timeout(RPC_TIMEOUT, async {
            loop {
                let mut message = self.incoming.recv().await.ok_or_else(|| io::Error::other(format!("Google Antigravity stopped during {method}")))?;
                if response(&message) == Some(id) {
                    return match message.get("error") {
                        Some(error) => Err(failure(error)),
                        None => Ok(message["result"].take()),
                    };
                }
                held.push(message);
            }
        }).await.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Google Antigravity took too long to respond"))?
    }

    async fn close(mut self) {
        let _ = self.child.kill().await;
    }
}

fn executable(env: &BTreeMap<String, String>) -> String {
    env.get("AGY_CLI_PATH").cloned().or_else(|| std::env::var("AGY_CLI_PATH").ok()).unwrap_or_else(|| "agy".to_owned())
}

fn response(message: &Value) -> Option<u64> {
    if message.get("method").is_some() { None } else { message["id"].as_u64() }
}

fn failure(error: &Value) -> io::Error {
    if error["code"].as_i64() == Some(AUTH_REQUIRED) {
        return io::Error::new(io::ErrorKind::PermissionDenied, SIGN_IN);
    }
    io::Error::other(format!("Google Antigravity: {}", error["message"].as_str().unwrap_or("the request failed")))
}

fn values(options: &[Value], id: &str) -> Vec<Value> {
    let Some(option) = options.iter().find(|option| option["id"] == id) else { return Vec::new() };
    option["options"].as_array().into_iter().flatten()
        .flat_map(|entry| entry["options"].as_array().cloned().unwrap_or_else(|| vec![entry.clone()]))
        .collect()
}

fn current(options: &[Value], id: &str) -> Option<String> {
    options.iter().find(|option| option["id"] == id)?["currentValue"].as_str().map(str::to_owned)
}

/// Reads each quota group's weekly limit with agy's `/quota`, which makes no model call.
pub async fn usage(env: &BTreeMap<String, String>) -> io::Result<Vec<Reading>> {
    let output = Command::new(executable(env)).args(["-p", "/quota", "--output-format", "json"]).envs(env).current_dir(crate::home())
        .stdin(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true).output().await?;
    if !output.status.success() {
        return Err(io::Error::other(format!("agy /quota exited with {}", output.status)));
    }
    let quota: Value = serde_json::from_slice(&output.stdout)?;
    Ok(quota["command"]["data"]["groups"].as_array().into_iter().flatten().flat_map(|group| {
        group["buckets"].as_array().into_iter().flatten().filter_map(|bucket| {
            let mut reading = Reading::new(Agent::Antigravity, Window::Weekly, 1.0 - bucket["remaining_fraction"].as_f64()?, bucket["reset_time"].as_str().and_then(super::epoch_ms));
            let short = if bucket["id"] == "gemini-weekly" { "gemini" } else { "other" };
            reading.group = Some(Group { name: group["name"].as_str()?.to_owned(), short: short.to_owned() });
            Some(reading)
        })
    }).collect())
}

pub async fn options(cwd: &Path, env: &BTreeMap<String, String>, settings: &AgentSettings) -> io::Result<(Vec<Value>, AgentSettings)> {
    let connection = Connection::open(cwd, env, false).await?;
    let mut connection = connection;
    let result = connection.call("session/new", json!({ "cwd": cwd, "mcpServers": [] }), &mut Vec::new()).await.map(|session| {
        let options = session["configOptions"].as_array().cloned().unwrap_or_default();
        let efforts: Vec<Value> = values(&options, "thought_level").iter().map(|effort| effort["value"].clone()).collect();
        let models = values(&options, "model").into_iter().map(|model| json!({ "value": model["value"], "name": model["name"], "efforts": efforts })).collect();
        let chosen = AgentSettings {
            model: settings.model.clone().or_else(|| current(&options, "model")),
            effort: settings.effort.clone().or_else(|| current(&options, "thought_level")),
            mode: settings.mode.clone(),
            ..AgentSettings::default()
        };
        (models, chosen)
    });
    connection.close().await;
    result
}

/// Lists the conversations agy kept for `cwd`; agy lists every directory's, whatever `cwd` asks for.
pub async fn sessions(cwd: &Path, env: &BTreeMap<String, String>) -> io::Result<Vec<Session>> {
    let mut connection = Connection::open(cwd, env, false).await?;
    let result = async {
        let mut sessions = Vec::new();
        let mut params = json!({ "cwd": cwd });
        loop {
            let mut page = connection.call("session/list", params.clone(), &mut Vec::new()).await?;
            for session in page["sessions"].as_array().into_iter().flatten() {
                if session["cwd"].as_str().map(Path::new) != Some(cwd) { continue; }
                let (Some(id), Some(title), Some(updated)) = (session["sessionId"].as_str(), session["title"].as_str().filter(|title| !title.is_empty()), session["updatedAt"].as_str().and_then(super::epoch_ms)) else { continue };
                sessions.push(Session::new(Agent::Antigravity, id, title, updated));
            }
            if page["nextCursor"].is_null() { return Ok(sessions); }
            params["cursor"] = page["nextCursor"].take();
        }
    }.await;
    connection.close().await;
    result
}

/// Opens the Chat's conversation, showing in the Chat why agy could not, so that the user can fix it and send again.
pub async fn start(data_dir: &Path, chat: &Chat, env: &BTreeMap<String, String>) -> (Antigravity, UnboundedReceiver<Event>) {
    let (events, received) = mpsc::unbounded_channel();
    let (requests, actions) = mpsc::unbounded_channel();
    let (progress, shells) = mpsc::unbounded_channel();
    let mut transcript = Transcript::new(data_dir, &chat.cwd, events, chat.settings.clone());
    if let Some(tokens) = chat.tokens { transcript.tokens = tokens; }
    transcript.capabilities.modes = Agent::Antigravity.modes().iter().map(|mode| Choice::mode(mode)).collect();
    let mut live = Live {
        data_dir: data_dir.to_owned(), chat_id: chat.id.clone(), env: env.clone(), connection: None, session: chat.agent_session.clone(),
        loaded: false, full_access: false, prompts: HashSet::new(), interrupting: false, permissions: HashMap::new(), progress, transcript,
    };
    live.open(chat.agent_session.is_some() && chat.status != Status::Done, &chat.shell_context).await;
    tokio::spawn(live.drive(actions, shells));
    (Antigravity { requests }, received)
}

struct Live {
    data_dir: PathBuf,
    chat_id: String,
    env: BTreeMap<String, String>,
    connection: Option<Connection>,
    session: Option<String>,
    /// Whether this driver has shown the session's history, so a reconnect skips its replay.
    loaded: bool,
    full_access: bool,
    prompts: HashSet<u64>,
    interrupting: bool,
    permissions: HashMap<String, (Value, Vec<Value>)>,
    progress: UnboundedSender<shell::Progress>,
    transcript: Transcript,
}

impl Live {
    async fn open(&mut self, unfinished: bool, shell_context: &[String]) {
        if let Err(error) = self.connect().await { self.transcript.error(error.to_string()); }
        self.transcript.stop_commands();
        self.transcript.finish();
        if unfinished {
            self.transcript.push(Item::interrupted());
            let _ = self.transcript.events.send(Event::Recoverable);
        }
        self.transcript.restore_shell_context(shell_context);
        self.transcript.report_settings();
    }

    async fn drive(mut self, mut actions: UnboundedReceiver<Action>, mut shells: UnboundedReceiver<shell::Progress>) {
        loop {
            tokio::select! {
                action = actions.recv() => match action {
                    Some(Action::Close) | None => break,
                    Some(action) => if let Err(error) = self.act(action).await { self.fail(error.to_string()).await; },
                },
                message = next(&mut self.connection) => match message {
                    Some(message) => self.handle(message, false).await,
                    None => {
                        if let Some(connection) = self.connection.take() { connection.close().await; }
                        self.prompts.clear();
                        self.fail("Google Antigravity stopped unexpectedly.".to_owned()).await;
                    }
                },
                Some(progress) = shells.recv() => self.transcript.shell_progress(progress),
            }
        }
        self.transcript.stop_shells();
        self.cancel_permissions().await;
        if let Some(mut connection) = self.connection.take() {
            if let (false, Some(session)) = (self.prompts.is_empty(), &self.session) {
                let _ = timeout(CLOSING, connection.notify("session/cancel", json!({ "sessionId": session }))).await;
            }
            connection.close().await;
        }
        self.transcript.stop_commands();
        self.transcript.finish();
    }

    async fn fail(&mut self, text: String) {
        self.transcript.error(text);
        if self.prompts.is_empty() { self.end_turn().await; }
    }

    async fn connect(&mut self) -> io::Result<()> {
        if self.connection.is_some() { return Ok(()); }
        let full_access = self.transcript.settings.chosen.mode.as_deref() == Some("full-access");
        let mut connection = Connection::open(&self.transcript.cwd, &self.env, full_access).await?;
        let config = html::config(&self.data_dir, &self.chat_id);
        let servers = json!([{ "name": html::SERVER, "command": config["command"], "args": config["args"], "env": [] }]);
        let mut held = Vec::new();
        let opened = match &self.session {
            Some(session) => connection.call("session/load", json!({ "sessionId": session, "cwd": self.transcript.cwd, "mcpServers": servers }), &mut held).await,
            None => connection.call("session/new", json!({ "cwd": self.transcript.cwd, "mcpServers": servers }), &mut held).await,
        };
        let opened = match opened {
            Ok(opened) => opened,
            Err(error) => { connection.close().await; return Err(error); }
        };
        if let Some(session) = opened["sessionId"].as_str() {
            self.session = Some(session.to_owned());
            let _ = self.transcript.events.send(Event::Session(session.to_owned()));
        }
        self.connection = Some(connection);
        self.full_access = full_access;
        if !self.loaded {
            for message in held { self.handle(message, true).await; }
        }
        self.loaded = true;
        let chosen = self.transcript.settings.chosen.clone();
        self.transcript.configure(&opened);
        self.apply(chosen).await;
        Ok(())
    }

    /// Brings agy's model, effort and mode in line with the Chat's.
    async fn apply(&mut self, chosen: AgentSettings) {
        self.transcript.settings.chosen.mode = chosen.mode.clone();
        for (id, value) in [("model", chosen.model), ("thought_level", chosen.effort)] {
            if let Some(value) = value && current(&self.transcript.options, id).as_ref() != Some(&value)
                && let Err(error) = self.set_option(id, &value).await
            {
                self.transcript.error(error.to_string());
            }
        }
        let mode = agy_mode(chosen.mode.as_deref());
        if self.transcript.mode.as_deref() != Some(mode) && let Err(error) = self.set_mode(mode).await {
            self.transcript.error(error.to_string());
        }
        self.transcript.report_settings();
    }

    async fn call(&mut self, method: &str, mut params: Value) -> io::Result<Value> {
        let session = self.session.clone().ok_or_else(|| io::Error::other("Google Antigravity has no conversation yet"))?;
        let connection = self.connection.as_mut().ok_or_else(|| io::Error::other("Google Antigravity isn't running"))?;
        params["sessionId"] = json!(session);
        let mut held = Vec::new();
        let result = connection.call(method, params, &mut held).await;
        for message in held { self.handle(message, false).await; }
        result
    }

    async fn set_option(&mut self, id: &str, value: &str) -> io::Result<()> {
        let result = self.call("session/set_config_option", json!({ "configId": id, "value": value })).await?;
        match result["configOptions"].as_array() {
            Some(options) => self.transcript.options = options.clone(),
            None => if let Some(option) = self.transcript.options.iter_mut().find(|option| option["id"] == id) { option["currentValue"] = json!(value); },
        }
        self.transcript.adopt_options();
        Ok(())
    }

    async fn set_mode(&mut self, mode: &str) -> io::Result<()> {
        self.call("session/set_mode", json!({ "modeId": mode })).await?;
        self.transcript.mode = Some(mode.to_owned());
        Ok(())
    }

    async fn act(&mut self, action: Action) -> io::Result<()> {
        match action {
            Action::Send(text, images) => self.send(text, images).await,
            Action::Retry => {
                self.connect().await?;
                match self.transcript.last_input.clone() {
                    Some((text, images)) => self.send(text, images).await,
                    None => { self.transcript.finish(); Ok(()) }
                }
            }
            Action::Interrupt => {
                self.transcript.stop_shells();
                self.cancel_permissions().await;
                if let (false, Some(session), Some(connection)) = (self.prompts.is_empty(), &self.session, &mut self.connection) {
                    self.interrupting = true;
                    connection.notify("session/cancel", json!({ "sessionId": session })).await?;
                }
                Ok(())
            }
            Action::Answer(id, answer) => self.answer(&id, answer).await,
            Action::Run(builtin) => self.run(builtin).await,
            Action::Delegated(item) => { self.transcript.delegated(item); Ok(()) }
            Action::Shell(command) => { self.transcript.run_shell(command, &self.progress, &self.env); Ok(()) }
            Action::Close => Ok(()),
        }
    }

    async fn send(&mut self, text: String, images: Vec<Image>) -> io::Result<()> {
        self.connect().await?;
        let mut prompt = Vec::new();
        if let Some(context) = self.transcript.take_shell_context() { prompt.push(json!({ "type": "text", "text": context })); }
        if !text.is_empty() { prompt.push(json!({ "type": "text", "text": text })); }
        prompt.extend(images.iter().map(|image| json!({ "type": "image", "mimeType": image.media_type, "data": image.data })));
        self.transcript.close();
        if std::mem::take(&mut self.interrupting) {
            self.transcript.stop_commands();
            self.transcript.push(Item::interrupted());
        }
        self.transcript.user(&text, &images, false);
        self.transcript.last_input = Some((text, images));
        self.transcript.status(Status::Working);
        let session = self.session.clone().unwrap_or_default();
        let connection = self.connection.as_mut().ok_or_else(|| io::Error::other("Google Antigravity isn't running"))?;
        let id = connection.request("session/prompt", json!({ "sessionId": session, "prompt": prompt })).await?;
        self.prompts.insert(id);
        Ok(())
    }

    async fn answer(&mut self, id: &str, answer: Answer) -> io::Result<()> {
        let Some((rpc, options)) = self.permissions.remove(id) else { return Ok(()) };
        let allow = matches!(answer, Answer::Allow | Answer::Rules(_));
        let kinds: &[&str] = if allow { &["allow_once", "allow_always"] } else { &["reject_once", "reject_always"] };
        let chosen = kinds.iter().find_map(|kind| options.iter().find(|option| option["kind"] == *kind)).map(|option| option["optionId"].clone());
        let outcome = match chosen {
            Some(option) => json!({ "outcome": "selected", "optionId": option }),
            None => json!({ "outcome": "cancelled" }),
        };
        let _ = self.transcript.events.send(Event::Resolved(id.to_owned()));
        self.transcript.status(if self.permissions.is_empty() { Status::Working } else { Status::Waiting });
        if let Some(connection) = &mut self.connection { connection.respond(rpc, json!({ "outcome": outcome })).await?; }
        if let Answer::Deny { instead: Some(text) } = answer { self.send(text, Vec::new()).await?; }
        Ok(())
    }

    async fn cancel_permissions(&mut self) {
        for (id, (rpc, _)) in std::mem::take(&mut self.permissions) {
            let _ = self.transcript.events.send(Event::Resolved(id));
            if let Some(connection) = &mut self.connection {
                let _ = connection.respond(rpc, json!({ "outcome": { "outcome": "cancelled" } })).await;
            }
        }
    }

    async fn run(&mut self, builtin: Builtin) -> io::Result<()> {
        match builtin {
            Builtin::Model(model) => {
                self.transcript.settings.chosen.model = Some(model.clone());
                if self.connection.is_some() { self.set_option("model", &model).await?; }
            }
            Builtin::Effort(effort) => {
                self.transcript.settings.chosen.effort = Some(effort.clone());
                if self.connection.is_some() { self.set_option("thought_level", &effort).await?; }
            }
            Builtin::Mode(mode) if Agent::Antigravity.modes().contains(&mode.as_str()) => {
                let full_access = mode == "full-access";
                if self.connection.is_some() && full_access != self.full_access {
                    if self.transcript.status != Status::Done {
                        return Err(io::Error::other("Wait for the turn to finish before switching to or from full access."));
                    }
                    if let Some(connection) = self.connection.take() { connection.close().await; }
                }
                self.transcript.settings.chosen.mode = Some(mode.clone());
                if self.connection.is_none() {
                    if self.session.is_some() { self.connect().await?; }
                } else {
                    self.set_mode(agy_mode(Some(&mode))).await?;
                }
            }
            _ => return Err(io::Error::other("Google Antigravity does not support this command")),
        }
        self.transcript.report_settings();
        Ok(())
    }

    async fn handle(&mut self, message: Value, history: bool) {
        match (message["method"].as_str(), message.get("id")) {
            (Some("session/update"), None) => {
                let update = &message["params"]["update"];
                if update["sessionUpdate"] == "current_mode_update" {
                    if let Some(mode) = update["currentModeId"].as_str() {
                        self.transcript.mode = Some(mode.to_owned());
                        self.transcript.settings.chosen.mode = Some(if self.full_access { "full-access" } else { mode }.to_owned());
                        self.transcript.report_settings();
                    }
                } else {
                    self.transcript.update(update, history);
                }
            }
            (Some("session/request_permission"), Some(id)) => self.permission(id.clone(), &message["params"]).await,
            (Some(_), Some(id)) => {
                if let Some(connection) = &mut self.connection {
                    let _ = connection.write(json!({ "id": id, "error": { "code": -32601, "message": "Method not found" } })).await;
                }
            }
            (Some(_), None) => {}
            (None, _) => if let Some(id) = response(&message) && self.prompts.remove(&id) { self.prompted(&message).await },
        }
    }

    async fn permission(&mut self, rpc: Value, params: &Value) {
        let id = Uuid::new_v4().to_string();
        let call = self.transcript.merged(&params["toolCall"]);
        let input = &call["rawInput"];
        let kind = match call["kind"].as_str() {
            Some("execute") => RequestKind::Command {
                id: id.clone(), words: words(&command(&call)), suggestion: Vec::new(), session_prefixes: false, allow_rules: Some(false),
            },
            _ if let Some(change) = diff(&call) => RequestKind::Edit {
                id: id.clone(), path: relative(&self.transcript.cwd, change["path"].as_str().unwrap_or_default()),
                diff: unified(change["oldText"].as_str().unwrap_or_default(), change["newText"].as_str().unwrap_or_default()),
            },
            _ => RequestKind::Other {
                id: id.clone(), title: "Google Antigravity requests permission".to_owned(),
                detail: call["title"].as_str().map(str::to_owned).unwrap_or_else(|| input.to_string()),
            },
        };
        let options = params["options"].as_array().cloned().unwrap_or_default();
        self.permissions.insert(id, (rpc, options));
        let _ = self.transcript.events.send(Event::Request(Request { kind, subagent: None }));
        self.transcript.status(Status::Waiting);
    }

    async fn prompted(&mut self, message: &Value) {
        if let Some(error) = message.get("error") { self.transcript.error(failure(error).to_string()); }
        let result = &message["result"];
        if let Some(usage) = result.get("usage") {
            self.transcript.tokens.input += usage["inputTokens"].as_u64().unwrap_or(0);
            self.transcript.tokens.output += usage["outputTokens"].as_u64().unwrap_or(0);
            let _ = self.transcript.events.send(Event::Meters(None, Some(self.transcript.tokens)));
        }
        if !self.prompts.is_empty() { return; }
        let ending = match result["stopReason"].as_str() {
            Some("refusal") => Some("Google Antigravity refused to continue."),
            Some("max_tokens") => Some("Google Antigravity reached its output limit."),
            Some("max_turn_requests") => Some("Google Antigravity reached its limit of requests in one turn."),
            _ => None,
        };
        if let Some(text) = ending { self.transcript.error(text.to_owned()); }
        let interrupted = std::mem::take(&mut self.interrupting) || result["stopReason"] == "cancelled";
        self.end_turn().await;
        if interrupted { self.transcript.push(Item::interrupted()); }
    }

    async fn end_turn(&mut self) {
        self.interrupting = false;
        self.cancel_permissions().await;
        self.transcript.tools.clear();
        self.transcript.stop_commands();
        self.transcript.finish();
    }
}

async fn next(connection: &mut Option<Connection>) -> Option<Value> {
    match connection {
        Some(connection) => connection.incoming.recv().await,
        None => std::future::pending().await,
    }
}

/// Agy has no full-access mode of its own: Stampede starts it skipping permissions instead.
fn agy_mode(mode: Option<&str>) -> &str {
    match mode {
        None | Some("full-access") => "default",
        Some(mode) => mode,
    }
}

fn command(call: &Value) -> String {
    let input = &call["rawInput"];
    input["command"].as_str().or(input["CommandLine"].as_str()).or(call["title"].as_str()).unwrap_or("Shell command").to_owned()
}

fn diff(call: &Value) -> Option<&Value> {
    call["content"].as_array()?.iter().find(|content| content["type"] == "diff")
}

fn texts(call: &Value) -> Vec<&str> {
    call["content"].as_array().into_iter().flatten()
        .filter(|content| content["type"] == "content" && content["content"]["type"] == "text")
        .filter_map(|content| content["content"]["text"].as_str()).collect()
}

fn output(call: &Value) -> String {
    let raw = &call["rawOutput"];
    raw["output"].as_str().or(raw.as_str()).map(str::to_owned).unwrap_or_else(|| texts(call).join("\n"))
}

/// The tool's result as MCP shaped it, for the Items that render Stampede's own tools.
fn result(call: &Value) -> Value {
    let raw = &call["rawOutput"];
    if raw.get("content").is_some() || raw.get("structuredContent").is_some() { return raw.clone(); }
    json!({ "content": texts(call).iter().map(|text| json!({ "type": "text", "text": text })).collect::<Vec<_>>() })
}

/// Which of Stampede's tools a call is, read from its title since ACP names no MCP server.
fn stampede(call: &Value) -> Option<&'static str> {
    if matches!(call["kind"].as_str(), Some("read" | "edit" | "delete" | "move" | "search" | "execute")) { return None; }
    let title = call["title"].as_str()?;
    [questions::TOOL, html::TOOL, crate::media::TOOL, delegation::TOOL, terminal_tools::STAGE, terminal_tools::READ].into_iter().find(|tool| title.contains(tool))
}

#[derive(Clone, Copy, PartialEq)]
enum Chunk {
    User,
    Text,
    Thought,
}

struct Running {
    item: Item,
    stdout: shell::Capture,
    stderr: shell::Capture,
    stop: Option<oneshot::Sender<()>>,
}

struct Transcript {
    data_dir: PathBuf,
    cwd: PathBuf,
    events: UnboundedSender<Event>,
    items: Vec<Item>,
    open: Option<(Chunk, usize)>,
    thinking: Option<Instant>,
    tools: HashMap<String, (usize, Value, Option<u64>)>,
    settings: Settings,
    capabilities: Capabilities,
    options: Vec<Value>,
    mode: Option<String>,
    status: Status,
    tokens: Tokens,
    last_input: Option<(String, Vec<Image>)>,
    running: HashMap<usize, Running>,
    shell: Option<(usize, Item)>,
    shell_context: Vec<(usize, Item, String)>,
}

impl Transcript {
    fn new(data_dir: &Path, cwd: &Path, events: UnboundedSender<Event>, settings: Settings) -> Self {
        Self {
            data_dir: data_dir.to_owned(), cwd: cwd.to_owned(), events, items: Vec::new(), open: None, thinking: None, tools: HashMap::new(),
            settings, capabilities: Capabilities::default(), options: Vec::new(), mode: None, status: Status::Done, tokens: Tokens { input: 0, output: 0 },
            last_input: None, running: HashMap::new(), shell: None, shell_context: Vec::new(),
        }
    }

    fn push(&mut self, item: Item) -> usize {
        let index = self.items.len();
        self.items.push(item.clone());
        let _ = self.events.send(Event::Item(index, item));
        index
    }

    fn changed(&self, index: usize) { let _ = self.events.send(Event::Item(index, self.items[index].clone())); }
    fn status(&mut self, status: Status) { self.status = status; let _ = self.events.send(Event::Status(status)); }
    fn error(&mut self, text: String) { self.close(); self.push(Item::Notice { notice: Notice::Error, text }); }

    fn configure(&mut self, session: &Value) {
        if let Some(options) = session["configOptions"].as_array() { self.options = options.clone(); }
        if let Some(mode) = session["modes"]["currentModeId"].as_str() { self.mode = Some(mode.to_owned()); }
        self.adopt_options();
    }

    fn adopt_options(&mut self) {
        self.capabilities.models = values(&self.options, "model").iter().filter_map(|model| model["value"].as_str().map(str::to_owned)).collect();
        self.capabilities.efforts = values(&self.options, "thought_level").iter().filter_map(|effort| effort["value"].as_str().map(str::to_owned)).collect();
        self.settings.chosen.model = current(&self.options, "model").or(self.settings.chosen.model.take());
        self.settings.chosen.effort = current(&self.options, "thought_level").or(self.settings.chosen.effort.take());
    }

    fn report_settings(&self) {
        let _ = self.events.send(Event::Settings(self.settings.clone()));
        let _ = self.events.send(Event::Capabilities(self.capabilities.clone()));
    }

    /// Ends the streaming chunk, so that the next one starts its own Item.
    fn close(&mut self) {
        if let Some((Chunk::Thought, index)) = self.open.take()
            && let Item::Thinking { seconds, .. } = &mut self.items[index]
        {
            *seconds = self.thinking.take().map_or(0, |started| started.elapsed().as_secs());
            self.changed(index);
        }
    }

    /// Marks the turn's last reply as addressed to the user.
    fn conclude(&mut self) {
        if let Some(index) = self.items.iter().rposition(|item| matches!(item, Item::Text { .. }))
            && let Item::Text { phase, .. } = &mut self.items[index] && *phase != Phase::Final
        {
            *phase = Phase::Final;
            self.changed(index);
        }
    }

    fn presented(&mut self) {
        for index in (0..self.items.len()).rev() {
            match &mut self.items[index] {
                Item::Text { phase, .. } => *phase = Phase::Final,
                Item::Thinking { .. } => continue,
                _ => break,
            }
            self.changed(index);
        }
    }

    fn finish(&mut self) {
        self.close();
        for index in 0..self.items.len() {
            if let Item::Text { streaming, .. } = &mut self.items[index] && *streaming { *streaming = false; self.changed(index); }
        }
        self.conclude();
        let _ = self.events.send(Event::Activity(None));
        self.status(Status::Done);
    }

    fn stop_commands(&mut self) {
        for index in 0..self.items.len() {
            if let Item::Command { status, started_at, .. } = &mut self.items[index] && *status == CommandStatus::Running {
                *status = CommandStatus::Failed;
                *started_at = None;
                self.changed(index);
            }
        }
    }

    fn user(&mut self, text: &str, images: &[Image], history: bool) -> usize {
        if history { self.conclude(); }
        let image_urls = images.iter().map(|image| format!("data:{};base64,{}", image.media_type, image.data)).collect();
        self.push(Item::User { text: text.to_owned(), images: images.len(), image_urls, template: None, rewind_from: None })
    }

    fn update(&mut self, update: &Value, history: bool) {
        match update["sessionUpdate"].as_str() {
            Some("user_message_chunk") => self.chunk(Chunk::User, &update["content"], history),
            Some("agent_message_chunk") => self.chunk(Chunk::Text, &update["content"], history),
            Some("agent_thought_chunk") => self.chunk(Chunk::Thought, &update["content"], history),
            Some("tool_call" | "tool_call_update") => self.tool(update, history),
            Some("usage_update") => {
                if let (Some(used), Some(window)) = (update["used"].as_u64(), update["size"].as_u64()) {
                    let _ = self.events.send(Event::Meters(Some(Context { used, window, cache: None }), None));
                }
            }
            Some("session_info_update") => {
                if let Some(title) = update["title"].as_str().filter(|title| !title.is_empty()) { let _ = self.events.send(Event::Title(title.to_owned())); }
            }
            Some("available_commands_update") => {
                self.capabilities.commands = update["availableCommands"].as_array().into_iter().flatten().filter_map(|command| Some(AgentCommand {
                    name: command["name"].as_str()?.to_owned(),
                    description: command["description"].as_str().unwrap_or_default().to_owned(),
                    hint: command["input"]["hint"].as_str().unwrap_or_default().to_owned(),
                })).collect();
                self.report_settings();
            }
            Some("config_option_update") => {
                if let Some(options) = update["configOptions"].as_array() { self.options = options.clone(); }
                self.adopt_options();
                self.report_settings();
            }
            _ => {}
        }
    }

    fn chunk(&mut self, kind: Chunk, content: &Value, history: bool) {
        let open = self.open.filter(|(open, _)| *open == kind).map(|(_, index)| index);
        if open.is_none() { self.close(); }
        let text = content["text"].as_str().unwrap_or_default();
        let index = match (kind, open) {
            (Chunk::User, _) => return self.user_chunk(content, open, history),
            (_, _) if content["type"] != "text" || text.is_empty() => return,
            (_, Some(index)) => index,
            (Chunk::Text, None) => self.push(Item::Text { text: String::new(), streaming: !history, phase: Phase::Commentary }),
            (Chunk::Thought, None) => {
                self.thinking = (!history).then(Instant::now);
                self.push(Item::Thinking { seconds: 0, text: Some(String::new()) })
            }
        };
        match &mut self.items[index] {
            Item::Text { text: existing, .. } | Item::Thinking { text: Some(existing), .. } => existing.push_str(text),
            _ => return,
        }
        self.changed(index);
        self.open = Some((kind, index));
    }

    fn user_chunk(&mut self, content: &Value, open: Option<usize>, history: bool) {
        let image = (content["type"] == "image").then(|| Image {
            media_type: content["mimeType"].as_str().unwrap_or_default().to_owned(), data: content["data"].as_str().unwrap_or_default().to_owned(),
        });
        let (parts, text) = shell::parse(content["text"].as_str().unwrap_or_default());
        if open.is_none() { self.shells(parts); }
        if text.is_empty() && image.is_none() { return; }
        let images: Vec<Image> = image.into_iter().collect();
        let index = match open {
            Some(index) => {
                if let Item::User { text: existing, images: count, image_urls, .. } = &mut self.items[index] {
                    existing.push_str(text);
                    *count += images.len();
                    image_urls.extend(images.iter().map(|image| format!("data:{};base64,{}", image.media_type, image.data)));
                }
                self.changed(index);
                if let Some((existing, kept)) = &mut self.last_input { existing.push_str(text); kept.extend(images); }
                index
            }
            None => {
                let index = self.user(text, &images, history);
                self.last_input = Some((text.to_owned(), images));
                index
            }
        };
        self.open = Some((Chunk::User, index));
    }

    /// The call as last reported, with `update`'s fields on top.
    fn merged(&self, update: &Value) -> Value {
        let known = update["sessionUpdate"] != "tool_call";
        let mut call = update["toolCallId"].as_str().and_then(|id| self.tools.get(id)).filter(|_| known).map(|(_, call, _)| call.clone()).unwrap_or_else(|| json!({}));
        for (key, value) in update.as_object().into_iter().flatten().filter(|(key, value)| *key != "sessionUpdate" && !value.is_null()) {
            call[key] = value.clone();
        }
        call
    }

    fn tool(&mut self, update: &Value, history: bool) {
        self.close();
        let Some(id) = update["toolCallId"].as_str() else { return };
        let call = self.merged(update);
        // Agy numbers tool calls afresh each turn, so a new call replaces any earlier one of its id.
        let (index, started_at) = if update["sessionUpdate"] == "tool_call" {
            self.tools.remove(id);
            let tool = stampede(&call);
            if tool.is_some_and(|tool| super::presents(html::SERVER, tool)) { self.presented(); }
            if tool == Some(questions::TOOL) { return; }
            (self.push(Item::Tools { calls: Vec::new() }), (!history).then(super::now))
        } else {
            let Some((index, _, started_at)) = self.tools.get(id) else { return };
            (*index, *started_at)
        };
        if !matches!(self.items[index], Item::Subagent { .. }) {
            self.items[index] = self.item(&call, started_at);
            self.changed(index);
        }
        self.tools.insert(id.to_owned(), (index, call, started_at));
    }

    fn item(&self, call: &Value, started_at: Option<u64>) -> Item {
        let status = call["status"].as_str();
        if call["kind"] == "execute" {
            let status = match status { Some("completed") => CommandStatus::Ok, Some("failed") => CommandStatus::Failed, _ => CommandStatus::Running };
            let started_at = started_at.filter(|_| status == CommandStatus::Running);
            return Item::Command { command: command(call), status, output: output(call), started_at };
        }
        if status == Some("completed") && let Some(change) = diff(call) && let Some(path) = change["path"].as_str() {
            let new = change["newText"].as_str().unwrap_or_default();
            return Item::edit(relative(&self.cwd, path), unified(change["oldText"].as_str().unwrap_or_default(), new), change["oldText"].is_null());
        }
        let tool = stampede(call);
        let rendered = match tool {
            Some(html::TOOL) => html::item(&call["rawInput"]),
            Some(crate::media::TOOL) if status == Some("completed") => crate::media::item(&result(call)),
            Some(delegation::TOOL) if matches!(status, Some("completed" | "failed")) => delegation::item(&result(call), Some(&self.data_dir)),
            _ => None,
        };
        if let Some(item) = rendered { return item; }
        let title = call["title"].as_str().unwrap_or("Tool").to_owned();
        let location = call["locations"][0]["path"].as_str().map(|path| relative(&self.cwd, path));
        let (kind, detail) = match call["kind"].as_str() {
            Some("read") => (ToolKind::Read, location.unwrap_or_else(|| title.clone())),
            Some("search") => (ToolKind::Search, title.clone()),
            Some("fetch") => (ToolKind::Fetch, call["rawInput"]["url"].as_str().map(str::to_owned).unwrap_or_else(|| title.clone())),
            _ => (if tool.is_some() { ToolKind::Mcp } else { ToolKind::Other }, if call["rawInput"].is_null() { title.clone() } else { call["rawInput"].to_string() }),
        };
        Item::Tools { calls: vec![ToolCall { kind, name: title, detail }] }
    }

    fn delegated(&mut self, item: Item) {
        let Item::Subagent { id, name, prompt, .. } = &item else { return };
        let index = self.items.iter().position(|item| matches!(item, Item::Subagent { id: existing, .. } if existing == id))
            .or_else(|| self.tools.values().find_map(|(index, call, _)| {
                (stampede(call) == Some(delegation::TOOL)
                    && name.as_deref().is_some_and(|name| call["rawInput"]["rig"] == name)
                    && prompt.as_deref().is_some_and(|prompt| call["rawInput"]["task"] == prompt)
                    && !matches!(self.items[*index], Item::Subagent { .. })).then_some(*index)
            }));
        match index {
            Some(index) => { self.items[index] = item; self.changed(index); }
            None => { self.push(item); }
        }
    }

    fn run_shell(&mut self, command: String, progress: &UnboundedSender<shell::Progress>, env: &BTreeMap<String, String>) {
        let item = Item::Shell { command: command.clone(), status: CommandStatus::Running, output: String::new(), pending: false, started_at: Some(super::now()) };
        let index = self.push(item.clone());
        match shell::spawn(index, &command, &self.cwd, env, progress.clone()) {
            Ok(stop) => { self.running.insert(index, Running { item, stdout: shell::Capture::default(), stderr: shell::Capture::default(), stop: Some(stop) }); }
            Err(error) => {
                self.items[index] = Item::Shell { command, status: CommandStatus::Failed, output: error.to_string(), pending: false, started_at: None };
                self.changed(index);
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
                    self.items[index] = running.item.clone();
                    self.changed(index);
                }
            }
            shell::Progress::Exit { index, code } => {
                let Some(Running { mut item, stdout, stderr, .. }) = self.running.remove(&index) else { return };
                let Item::Shell { command, status, pending, started_at, .. } = &mut item else { return };
                *status = if code == Some(0) { CommandStatus::Ok } else { CommandStatus::Failed };
                *pending = true;
                *started_at = None;
                let block = shell::block(command, &stdout.text(), &stderr.text(), code);
                self.items[index] = item.clone();
                self.changed(index);
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

    fn report_shell_context(&self) {
        let _ = self.events.send(Event::ShellContext(self.shell_context.iter().map(|(_, _, block)| block.clone()).collect()));
    }

    fn take_shell_context(&mut self) -> Option<String> {
        if self.shell_context.is_empty() { return None; }
        let blocks = std::mem::take(&mut self.shell_context).into_iter().map(|(index, mut item, block)| {
            if let Item::Shell { pending, .. } = &mut item { *pending = false; }
            self.items[index] = item;
            self.changed(index);
            block
        }).collect::<Vec<_>>();
        self.report_shell_context();
        Some(blocks.join("\n"))
    }

    /// Shows `!` commands sent with an earlier prompt, whose input and output arrive as separate parts.
    fn shells(&mut self, parts: Vec<shell::Part>) {
        for part in parts {
            if let shell::Part::Input(command) = part {
                let item = Item::Shell { command, status: CommandStatus::Ok, output: String::new(), pending: false, started_at: None };
                self.shell = Some((self.push(item.clone()), item));
                continue;
            }
            let Some((index, Item::Shell { output, status, .. })) = &mut self.shell else { continue };
            match part {
                shell::Part::Output(text) => output.push_str(&text),
                _ => *status = CommandStatus::Failed,
            }
            let index = *index;
            self.items[index] = self.shell.as_ref().unwrap().1.clone();
            self.changed(index);
        }
    }

    fn restore_shell_context(&mut self, blocks: &[String]) {
        for block in blocks {
            self.shells(shell::parse(block).0);
            let Some((index, mut item)) = self.shell.take() else { continue };
            if let Item::Shell { pending, .. } = &mut item { *pending = true; }
            self.items[index] = item.clone();
            self.changed(index);
            self.shell_context.push((index, item, block.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn full_access_survives_agent_mode_updates() {
        let root = tempfile::tempdir().unwrap();
        let (events, _received) = mpsc::unbounded_channel();
        let (progress, _) = mpsc::unbounded_channel();
        let mut live = Live {
            data_dir: root.path().to_owned(), chat_id: "test".to_owned(), env: BTreeMap::new(), connection: None, session: None,
            loaded: false, full_access: true, prompts: HashSet::new(), interrupting: false, permissions: HashMap::new(), progress,
            transcript: Transcript::new(root.path(), root.path(), events, Settings::default()),
        };
        live.transcript.settings.chosen.mode = Some("full-access".to_owned());
        for mode in ["plan", "default"] {
            live.handle(json!({ "method": "session/update", "params": { "update": { "sessionUpdate": "current_mode_update", "currentModeId": mode } } }), false).await;
            assert_eq!(live.transcript.settings.chosen.mode.as_deref(), Some("full-access"));
            assert_eq!(live.transcript.mode.as_deref(), Some(mode));
        }
    }
}
