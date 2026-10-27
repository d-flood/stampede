use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use github_copilot_sdk::{
    Attachment, Client, MessageOptions, PermissionRequestData, PermissionRequestKind, RequestId, ResumeSessionConfig,
    SessionConfig, SessionEvent, SessionId, SessionListFilter, SetModelOptions,
    handler::{PermissionHandler, PermissionResult, UserInputHandler, UserInputResponse},
    session::Session as CopilotSession,
    rpc::{EventsReadDirection, ModelListRequest, SessionsReadPersistedEventsRequest},
    session_events::{AiCreditsStatus, ProviderMonthlyUsageScope, ProviderMonthlyUsageState, ProviderQuotaState, ProviderQuotaUnit, SessionQuotaObservationData, UsageGetMetricsResult},
};
use serde_json::{Value, json};
use tokio::{
    process::{Child, Command},
    sync::{mpsc::{self, UnboundedReceiver, UnboundedSender}, oneshot},
    time::timeout,
};
use uuid::Uuid;

use super::{
    AgentCommand, Answer, Builtin, CLOSING, Capabilities, Chat, Choice, ChoiceDescription, CommandStatus,
    Context, Driver, Event, Image, Item, Notice, Phase, Question, Request, RequestKind, Session,
    Settings, Status, Tokens, ToolCall, ToolKind, html, relative, unified, words,
};
use crate::{config::{Agent, AgentSettings}, usage::{Consumption, Reading, Unit, Window}};

const RPC_TIMEOUT: Duration = Duration::from_secs(15);

pub struct Copilot {
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

impl Driver for Copilot {
    fn send(&self, text: &str, images: &[Image]) { let _ = self.requests.send(Action::Send(text.to_owned(), images.to_vec())); }
    fn retry(&self) { let _ = self.requests.send(Action::Retry); }
    fn interrupt(&self) { let _ = self.requests.send(Action::Interrupt); }
    fn answer(&self, id: &str, answer: Answer) { let _ = self.requests.send(Action::Answer(id.to_owned(), answer)); }
    fn run(&self, builtin: Builtin) { let _ = self.requests.send(Action::Run(builtin)); }
    fn delegated(&self, item: Item) { let _ = self.requests.send(Action::Delegated(item)); }
    fn close(&self) { let _ = self.requests.send(Action::Close); }
    fn shell(&self, command: &str) { let _ = self.requests.send(Action::Shell(command.to_owned())); }
}

struct Connection {
    client: Client,
    child: Child,
}

impl Connection {
    async fn open(cwd: &Path, env: &BTreeMap<String, String>) -> io::Result<Self> {
        let executable = env.get("COPILOT_CLI_PATH").cloned().or_else(|| std::env::var("COPILOT_CLI_PATH").ok()).unwrap_or_else(|| "copilot".to_owned());
        let mut child = Command::new(executable)
            .args(["--server", "--stdio"])
            .current_dir(cwd).envs(env)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
            .kill_on_drop(true).spawn()
            .map_err(|error| io::Error::new(error.kind(), format!("Cannot start GitHub Copilot CLI: {error}. Install Copilot CLI and run `copilot login`.")))?;
        let client = Client::from_streams(child.stdout.take().unwrap(), child.stdin.take().unwrap(), cwd.to_owned()).map_err(io::Error::other)?;
        rpc(client.verify_protocol_version()).await?;
        if !rpc(client.get_auth_status()).await?.is_authenticated {
            return Err(io::Error::other("GitHub Copilot is not signed in. Run `copilot login` on the Stampede host, then try again."));
        }
        Ok(Self { client, child })
    }

    async fn close(&mut self) {
        let _ = timeout(Duration::from_secs(5), self.client.stop()).await;
        let _ = self.child.kill().await;
    }
}

async fn rpc<T>(future: impl std::future::Future<Output = Result<T, github_copilot_sdk::Error>>) -> io::Result<T> {
    timeout(RPC_TIMEOUT, future).await.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "GitHub Copilot took too long to respond"))?.map_err(io::Error::other)
}

pub async fn options(cwd: &Path, env: &BTreeMap<String, String>, settings: &AgentSettings) -> io::Result<(Vec<Value>, AgentSettings)> {
    let mut connection = Connection::open(cwd, env).await?;
    let result = async {
        let mut config = SessionConfig::default().deny_all_permissions().with_working_directory(cwd);
        if let Some(model) = &settings.model { config = config.with_model(model); }
        if let Some(effort) = &settings.effort { config = config.with_reasoning_effort(effort); }
        let session = rpc(connection.client.create_session(config)).await?;
        let result = async {
            let models = models(&session).await?;
            let current = rpc(session.rpc().model().get_current()).await?;
            let chosen = AgentSettings {
                model: current.model_id.or_else(|| settings.model.clone()),
                effort: current.reasoning_effort.or_else(|| settings.effort.clone()),
                mode: settings.mode.clone(),
            };
            Ok((models, chosen))
        }.await;
        let _ = rpc(session.disconnect()).await;
        let _ = rpc(connection.client.delete_session(session.id())).await;
        result
    }.await;
    connection.close().await;
    result
}

async fn models(session: &CopilotSession) -> io::Result<Vec<Value>> {
    rpc(session.rpc().model().list_with_params(ModelListRequest { skip_cache: Some(true) })).await.map(|catalog| catalog.list)
}

pub async fn usage(env: &BTreeMap<String, String>) -> io::Result<Vec<Reading>> {
    let mut connection = Connection::open(&crate::home(), env).await?;
    let result = async {
        let session = rpc(connection.client.create_session(SessionConfig::default().deny_all_permissions().with_working_directory(crate::home()))).await?;
        let result = rpc(session.rpc().quota().refresh()).await.map(|quota| readings(quota.provider_quotas.as_deref().unwrap_or_default()));
        let _ = rpc(session.disconnect()).await;
        let _ = rpc(connection.client.delete_session(session.id())).await;
        let mut readings = result?;
        match recorded_costs(&connection.client).await {
            Ok(costs) => readings.extend(costs),
            Err(error) => eprintln!("cannot read Copilot recorded usage: {error}"),
        }
        Ok(readings)
    }.await;
    connection.close().await;
    result
}

fn readings(providers: &[ProviderQuotaState]) -> Vec<Reading> {
    let mut reading = Reading::new(Agent::Copilot, Window::Monthly, 0.0, None);
    reading.used = None;
    if let Some(monthly) = providers.iter().filter(|quota| quota.provider.id == "copilot").find_map(|quota| quota.monthly_usage.as_ref())
        && monthly.scope == ProviderMonthlyUsageScope::User {
        reading.starts_at = monthly.cycle_start.as_deref().and_then(super::epoch_ms);
        reading.resets_at = monthly.reset_on.as_deref().and_then(super::epoch_ms);
        reading.read_at = monthly.queried_at.as_deref().and_then(super::epoch_ms).unwrap_or(reading.read_at);
        if monthly.state == ProviderMonthlyUsageState::Available
            && let Some(amount) = monthly.consumed_quantity.filter(|amount| amount.is_finite() && *amount >= 0.0)
            && let Some(unit) = match monthly.unit {
                ProviderQuotaUnit::AiCredits => Some(Unit::Credits),
                ProviderQuotaUnit::Requests => Some(Unit::Requests),
                ProviderQuotaUnit::Tokens => Some(Unit::Tokens),
                _ => None,
            } {
            reading.consumption = Some(Consumption { amount, unit });
        }
    }
    vec![reading]
}

async fn report_cost(session: &CopilotSession, chat_id: &str, events: &UnboundedSender<Event>) {
    let Ok(metrics) = rpc(session.rpc().usage().get_metrics()).await else { return };
    let reading = session_cost(session.id(), Some(chat_id), &metrics);
    let _ = events.send(Event::Usage(vec![reading]));
}

fn session_cost(session_id: &SessionId, chat_id: Option<&str>, metrics: &UsageGetMetricsResult) -> Reading {
    let mut reading = Reading::new(Agent::Copilot, Window::Session, 0.0, None);
    reading.used = None;
    reading.chat_id = chat_id.map(str::to_owned);
    reading.session_id = Some(session_id.to_string());
    reading.partial = metrics.ai_credits_status != Some(AiCreditsStatus::Complete);
    if !matches!(metrics.ai_credits_status, Some(AiCreditsStatus::Unavailable | AiCreditsStatus::Unknown))
        && let Some(amount) = metrics.total_nano_aiu.filter(|amount| amount.is_finite() && *amount >= 0.0)
        && (amount > 0.0 || metrics.ai_credits_status.is_some()) {
        reading.consumption = Some(Consumption { amount: amount / 1e9, unit: Unit::Credits });
    }
    if reading.consumption.is_none() && metrics.total_user_requests >= 0 {
        reading.consumption = Some(Consumption { amount: metrics.total_user_requests as f64, unit: Unit::Requests });
        reading.partial = false;
    }
    reading
}

async fn recorded_costs(client: &Client) -> io::Result<Vec<Reading>> {
    let mut readings = Vec::new();
    for session in rpc(client.list_sessions(None)).await?.into_iter().filter(|session| !session.is_remote) {
        let mut cursor = None;
        loop {
            let page = match rpc(client.rpc().sessions().read_persisted_events(SessionsReadPersistedEventsRequest {
                session_id: session.session_id.clone(), direction: Some(EventsReadDirection::Backward), max: Some(100), cursor,
            })).await {
                Ok(page) => page,
                Err(error) => { eprintln!("cannot read Copilot session usage: {error}"); break; }
            };
            let snapshot = page.events.iter().rev()
                .filter(|event| matches!(event.event_type.as_str(), "session.usage_checkpoint" | "session.shutdown"))
                .find_map(|event| serde_json::from_value::<UsageGetMetricsResult>(event.data["accountingSnapshot"].clone()).ok());
            if let Some(metrics) = snapshot {
                readings.push(session_cost(&session.session_id, None, &metrics));
                break;
            }
            if !page.has_more { break; }
            cursor = Some(page.cursor);
        }
    }
    Ok(readings)
}

pub async fn sessions(cwd: &Path, env: &BTreeMap<String, String>) -> io::Result<Vec<Session>> {
    let mut connection = Connection::open(cwd, env).await?;
    let filter = SessionListFilter { working_directory: Some(cwd.display().to_string()), ..Default::default() };
    let result = rpc(connection.client.list_sessions(Some(filter))).await.map(|sessions| sessions.into_iter().filter_map(|session| {
        if session.is_remote { return None; }
        let title = session.summary.as_deref().filter(|title| !title.is_empty())?;
        Some(Session::new(Agent::Copilot, session.session_id.as_str(), title, super::epoch_ms(&session.modified_time)?))
    }).collect());
    connection.close().await;
    result
}

struct Interaction {
    request: RequestKind,
    managed: bool,
    response: oneshot::Sender<Answer>,
}

struct Handlers(UnboundedSender<Interaction>);

impl Handlers {
    async fn ask(&self, request: RequestKind, managed: bool) -> Answer {
        let (response, receiver) = oneshot::channel();
        let _ = self.0.send(Interaction { request, managed, response });
        receiver.await.unwrap_or(Answer::Deny { instead: None })
    }
}

#[async_trait]
impl PermissionHandler for Handlers {
    async fn handle(&self, _session: SessionId, id: RequestId, data: PermissionRequestData) -> PermissionResult {
        let id = id.to_string();
        let value = data.extra.get("permissionRequest").unwrap_or(&data.extra);
        let request = match data.kind {
            Some(PermissionRequestKind::Shell) => RequestKind::Command {
                id, words: words(value["fullCommandText"].as_str().unwrap_or("Shell command")), suggestion: Vec::new(), session_prefixes: false, allow_rules: Some(false),
            },
            Some(PermissionRequestKind::Write) => RequestKind::Edit {
                id, path: value["fileName"].as_str().unwrap_or_default().to_owned(), diff: value["diff"].as_str().unwrap_or_default().to_owned(),
            },
            _ => RequestKind::Other {
                id, title: "Copilot requests permission".to_owned(),
                detail: value["intention"].as_str().map(str::to_owned).unwrap_or_else(|| value.to_string()),
            },
        };
        match self.ask(request, data.managed_settings_enabled || data.managed_approval_required == Some(true)).await {
            Answer::Allow => PermissionResult::approve_once(),
            Answer::Deny { instead } => PermissionResult::reject(instead),
            _ => PermissionResult::reject(None),
        }
    }
}

#[async_trait]
impl UserInputHandler for Handlers {
    async fn handle(&self, _session: SessionId, question: String, choices: Option<Vec<String>>, _freeform: Option<bool>) -> Option<UserInputResponse> {
        let choices = choices.unwrap_or_default();
        let request = RequestKind::Question { id: Uuid::new_v4().to_string(), questions: vec![Question {
            id: String::new(), header: String::new(), question,
            options: choices.iter().map(|label| ChoiceDescription { id: String::new(), label: label.clone(), description: String::new(), recommended: false, image: None, preview: None }).collect(),
            multi_select: false, answer_type: None, image: None,
        }] };
        let answer = match self.ask(request, false).await {
            Answer::Answers(answers) => answers.into_iter().flatten().collect::<Vec<_>>().join(", "),
            Answer::Deny { instead: Some(text) } => text,
            _ => return None,
        };
        Some(UserInputResponse { was_freeform: !choices.contains(&answer), answer })
    }
}

async fn open_session(client: &Client, data_dir: &Path, chat: &Chat, handlers: Arc<Handlers>) -> io::Result<CopilotSession> {
    let mut server = html::config(data_dir, &chat.id);
    server["type"] = json!("stdio");
    server["timeout"] = json!(86400000);
    let servers = serde_json::from_value(json!({ html::SERVER: server }))?;
    if let Some(id) = &chat.agent_session {
        let mut config = ResumeSessionConfig::new(SessionId::new(id))
            .with_working_directory(&chat.cwd).with_streaming(true).with_mcp_servers(servers)
            .with_permission_handler(handlers.clone()).with_user_input_handler(handlers);
        if let Some(model) = &chat.settings.chosen.model { config = config.with_model(model); }
        if let Some(effort) = &chat.settings.chosen.effort { config = config.with_reasoning_effort(effort); }
        rpc(client.resume_session(config)).await
    } else {
        let mut config = SessionConfig::default()
            .with_working_directory(&chat.cwd).with_streaming(true).with_mcp_servers(servers)
            .with_permission_handler(handlers.clone()).with_user_input_handler(handlers);
        if let Some(model) = &chat.settings.chosen.model { config = config.with_model(model); }
        if let Some(effort) = &chat.settings.chosen.effort { config = config.with_reasoning_effort(effort); }
        rpc(client.create_session(config)).await
    }
}

pub async fn start(data_dir: &Path, chat: &Chat, env: &BTreeMap<String, String>) -> io::Result<(Copilot, UnboundedReceiver<Event>)> {
    let connection = Connection::open(&chat.cwd, env).await?;
    let (interactions, pending) = mpsc::unbounded_channel();
    let handlers = Arc::new(Handlers(interactions));
    let session = open_session(&connection.client, data_dir, chat, handlers.clone()).await?;
    let subscribed = session.subscribe();
    let models = models(&session).await?;
    let history = if chat.agent_session.is_some() { rpc(session.get_events()).await? } else { Vec::new() };
    let (events, received) = mpsc::unbounded_channel();
    let mut transcript = Transcript::new(&chat.cwd, events, chat.settings.clone());
    transcript.capabilities.models = models.iter().filter_map(|model| model["id"].as_str().map(str::to_owned)).collect();
    transcript.capabilities.modes = Agent::Copilot.modes().iter().map(|mode| Choice::mode(mode)).collect();
    if let Ok(current) = rpc(session.rpc().model().get_current()).await {
        transcript.settings.chosen.model = current.model_id.or(transcript.settings.chosen.model);
        transcript.settings.chosen.effort = current.reasoning_effort.or(transcript.settings.chosen.effort);
    }
    transcript.efforts = models.iter().filter_map(|model| {
        let id = model["id"].as_str()?.to_owned();
        let efforts = model["supportedReasoningEfforts"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect();
        Some((id, efforts))
    }).collect();
    let _ = transcript.events.send(Event::Session(session.id().to_string()));
    if let Ok(quota) = rpc(session.rpc().quota().get()).await {
        let _ = transcript.events.send(Event::Usage(readings(quota.provider_quotas.as_deref().unwrap_or_default())));
    }
    report_cost(&session, &chat.id, &transcript.events).await;
    for event in history { transcript.read(&event, true); }
    transcript.stop_commands();
    transcript.finish();
    if chat.agent_session.is_some() && chat.status != Status::Done {
        transcript.push(Item::interrupted());
        let _ = transcript.events.send(Event::Recoverable);
    }
    transcript.report_settings();
    let (requests, actions) = mpsc::unbounded_channel();
    let location = Location { data_dir: data_dir.to_owned(), chat: chat.clone(), handlers };
    tokio::spawn(drive(connection, session, subscribed, transcript, pending, actions, location));
    Ok((Copilot { requests }, received))
}

struct Location {
    data_dir: PathBuf,
    chat: Chat,
    handlers: Arc<Handlers>,
}

async fn drive(
    mut connection: Connection,
    mut session: CopilotSession,
    mut subscribed: github_copilot_sdk::subscription::EventSubscription,
    mut transcript: Transcript,
    mut interactions: UnboundedReceiver<Interaction>,
    mut actions: UnboundedReceiver<Action>,
    location: Location,
) {
    let mut pending = HashMap::<String, oneshot::Sender<Answer>>::new();
    let mut shells = HashSet::<String>::new();
    let (shelled, mut finished) = mpsc::unbounded_channel::<(String, Result<Value, String>)>();
    loop {
        let result: io::Result<()> = tokio::select! {
            action = actions.recv() => match action {
                Some(Action::Send(text, images)) => send(&session, &mut transcript, &text, &images).await,
                Some(Action::Shell(command)) => {
                    let request = Uuid::new_v4().to_string();
                    shells.insert(request.clone());
                    let (client, shelled) = (connection.client.clone(), shelled.clone());
                    let params = json!({ "sessionId": session.id().to_string(), "command": command, "requestId": request });
                    tokio::spawn(async move {
                        let result = client.call("session.shell.executeUserRequested", Some(params)).await.map_err(|error| error.to_string());
                        let _ = shelled.send((request, result));
                    });
                    Ok(())
                }
                Some(Action::Retry) => {
                    if let Some((text, images)) = transcript.last_input.clone() { send(&session, &mut transcript, &text, &images).await } else { Ok(()) }
                }
                Some(Action::Interrupt) => {
                    for request in &shells {
                        let _ = connection.client.call("session.shell.cancelUserRequested", Some(json!({ "sessionId": session.id().to_string(), "requestId": request }))).await;
                    }
                    resolve(&mut pending, &transcript.events);
                    rpc(session.abort()).await.map(|_| { transcript.stop_commands(); transcript.finish(); transcript.push(Item::interrupted()); })
                }
                Some(Action::Answer(id, answer)) => {
                    if let Some(response) = pending.remove(&id) {
                        let _ = response.send(answer);
                        let _ = transcript.events.send(Event::Resolved(id));
                        transcript.status(if pending.is_empty() { Status::Working } else { Status::Waiting });
                    }
                    Ok(())
                }
                Some(Action::Run(builtin)) => {
                    let result = run(&connection.client, &location, &mut session, &mut subscribed, &mut transcript, builtin).await;
                    if result.is_ok() { report_cost(&session, &location.chat.id, &transcript.events).await; }
                    result
                }
                Some(Action::Delegated(item)) => { transcript.push(item); Ok(()) }
                Some(Action::Close) | None => {
                    pending.clear();
                    let _ = timeout(CLOSING, async {
                        if transcript.status != Status::Done { let _ = session.abort().await; }
                        let _ = session.disconnect().await;
                    }).await;
                    let _ = connection.child.kill().await;
                    break;
                }
            },
            Some((request, result)) = finished.recv() => {
                shells.remove(&request);
                result.map(|_| ()).map_err(io::Error::other)
            },
            interaction = interactions.recv() => {
                let Some(interaction) = interaction else { break };
                if transcript.settings.chosen.mode.as_deref() == Some("full-access") && !interaction.managed && !matches!(interaction.request, RequestKind::Question { .. }) {
                    let _ = interaction.response.send(Answer::Allow);
                } else {
                    pending.insert(interaction.request.id().to_owned(), interaction.response);
                    let _ = transcript.events.send(Event::Request(Request { kind: interaction.request, subagent: None }));
                    transcript.status(Status::Waiting);
                }
                Ok(())
            },
            event = subscribed.recv() => match event {
                Ok(event) => {
                    transcript.read(&event, false);
                    if matches!(event.event_type.as_str(), "assistant.usage" | "session.idle") {
                        report_cost(&session, &location.chat.id, &transcript.events).await;
                    }
                    if transcript.status == Status::Done { resolve(&mut pending, &transcript.events); }
                    Ok(())
                }
                Err(error) => { transcript.error(error.to_string()); break; }
            },
        };
        if let Err(error) = result { transcript.error(error.to_string()); }
    }
    transcript.stop_commands();
    transcript.finish();
    connection.close().await;
}

fn resolve(pending: &mut HashMap<String, oneshot::Sender<Answer>>, events: &UnboundedSender<Event>) {
    for (id, _) in pending.drain() { let _ = events.send(Event::Resolved(id)); }
}

async fn send(session: &CopilotSession, transcript: &mut Transcript, text: &str, images: &[Image]) -> io::Result<()> {
    let attachments = images.iter().map(|image| Attachment::Blob { data: image.data.clone(), mime_type: image.media_type.clone(), display_name: None }).collect();
    transcript.last_input = Some((text.to_owned(), images.to_vec()));
    transcript.status(Status::Working);
    match rpc(session.send(MessageOptions::new(text).with_attachments(attachments))).await {
        Ok(_) => Ok(()),
        Err(error) => { transcript.finish(); Err(error) }
    }
}

async fn run(client: &Client, location: &Location, session: &mut CopilotSession, subscribed: &mut github_copilot_sdk::subscription::EventSubscription, transcript: &mut Transcript, builtin: Builtin) -> io::Result<()> {
    match builtin {
        Builtin::Model(model) => {
            rpc(session.set_model(&model, None)).await?;
            transcript.settings.chosen.model = Some(model);
            transcript.settings.chosen.effort = rpc(session.rpc().model().get_current()).await?.reasoning_effort;
        }
        Builtin::Effort(effort) => {
            let model = transcript.settings.chosen.model.as_deref().ok_or_else(|| io::Error::other("Choose a model before setting its effort"))?;
            rpc(session.set_model(model, Some(SetModelOptions::default().with_reasoning_effort(&effort)))).await?;
            transcript.settings.chosen.effort = Some(effort);
        }
        Builtin::Mode(mode) if Agent::Copilot.modes().contains(&mode.as_str()) => transcript.settings.chosen.mode = Some(mode),
        Builtin::Compact => { rpc(session.rpc().history().compact()).await?; }
        Builtin::Clear if transcript.status == Status::Done => {
            let mut chat = location.chat.clone();
            chat.agent_session = None;
            chat.settings = transcript.settings.clone();
            let next = open_session(client, &location.data_dir, &chat, location.handlers.clone()).await?;
            let _ = rpc(session.disconnect()).await;
            *session = next;
            *subscribed = session.subscribe();
            transcript.reset();
            let _ = transcript.events.send(Event::Session(session.id().to_string()));
        }
        Builtin::Rewind(index) if transcript.status == Status::Done => {
            let event_id = transcript.user_events.get(&index).ok_or_else(|| io::Error::other("Choose a user message to rewind to"))?.clone();
            transcript.status(Status::Working);
            rpc(session.rpc().history().truncate(github_copilot_sdk::rpc::HistoryTruncateRequest { event_id })).await?;
            let history = rpc(session.get_events()).await?;
            transcript.reset();
            for event in history { transcript.read(&event, true); }
            transcript.finish();
            transcript.push(Item::Notice { notice: super::Notice::Rewound, text: "Selected message and everything after it removed. Files are left unchanged.".to_owned() });
        }
        _ => return Err(io::Error::other("GitHub Copilot does not support this command")),
    }
    transcript.report_settings();
    Ok(())
}

struct Transcript {
    cwd: PathBuf,
    events: UnboundedSender<Event>,
    items: Vec<Item>,
    seen: HashSet<String>,
    messages: HashMap<String, usize>,
    user_events: HashMap<usize, String>,
    last_input: Option<(String, Vec<Image>)>,
    tools: HashMap<String, (usize, Value)>,
    thinking: Option<(usize, Instant)>,
    settings: Settings,
    capabilities: Capabilities,
    efforts: HashMap<String, Vec<String>>,
    status: Status,
    tokens: Tokens,
}

impl Transcript {
    fn new(cwd: &Path, events: UnboundedSender<Event>, settings: Settings) -> Self {
        Self { cwd: cwd.to_owned(), events, items: Vec::new(), seen: HashSet::new(), messages: HashMap::new(), user_events: HashMap::new(), last_input: None, tools: HashMap::new(), thinking: None, settings, capabilities: Capabilities::default(), efforts: HashMap::new(), status: Status::Done, tokens: Tokens { input: 0, output: 0 } }
    }

    fn push(&mut self, item: Item) -> usize {
        let index = self.items.len();
        self.items.push(item.clone());
        let _ = self.events.send(Event::Item(index, item));
        index
    }

    fn changed(&self, index: usize) { let _ = self.events.send(Event::Item(index, self.items[index].clone())); }
    fn status(&mut self, status: Status) { self.status = status; let _ = self.events.send(Event::Status(status)); }
    fn error(&mut self, text: String) { self.push(Item::Notice { notice: Notice::Error, text }); }
    fn report_settings(&self) {
        let _ = self.events.send(Event::Settings(self.settings.clone()));
        let mut capabilities = self.capabilities.clone();
        capabilities.efforts = self.settings.chosen.model.as_ref().and_then(|model| self.efforts.get(model)).cloned().unwrap_or_default();
        let _ = self.events.send(Event::Capabilities(capabilities));
    }

    fn reset(&mut self) {
        self.items.clear(); self.messages.clear(); self.user_events.clear(); self.tools.clear(); self.thinking = None;
        self.seen.clear(); self.last_input = None;
        self.tokens = Tokens { input: 0, output: 0 };
        let _ = self.events.send(Event::Truncate(0));
        let _ = self.events.send(Event::Reset);
    }

    fn finish_thinking(&mut self) {
        if let Some((index, started)) = self.thinking.take() {
            if let Item::Thinking { seconds, .. } = &mut self.items[index] { *seconds = started.elapsed().as_secs(); }
            self.changed(index);
        }
    }

    fn finish(&mut self) {
        self.finish_thinking();
        for index in 0..self.items.len() {
            if let Item::Text { streaming, .. } = &mut self.items[index] && *streaming { *streaming = false; self.changed(index); }
        }
        if let Some(index) = self.items.iter().rposition(|item| matches!(item, Item::Text { .. })) {
            if let Item::Text { phase, .. } = &mut self.items[index] { *phase = Phase::Final; }
            self.changed(index);
        }
        let _ = self.events.send(Event::Activity(None));
        self.status(Status::Done);
    }

    fn stop_commands(&mut self) {
        for index in 0..self.items.len() {
            if let Item::Command { status, .. } | Item::Shell { status, .. } = &mut self.items[index]
                && *status == CommandStatus::Running
            {
                *status = CommandStatus::Failed;
                self.changed(index);
            }
        }
    }

    fn read(&mut self, event: &SessionEvent, history: bool) {
        if !self.seen.insert(event.id.clone()) { return; }
        let data = &event.data;
        match event.event_type.as_str() {
            "user.message" => {
                self.finish_thinking();
                let text = data["content"].as_str().unwrap_or_default().to_owned();
                let images: Vec<Image> = data["attachments"].as_array().into_iter().flatten().filter_map(|attachment| Some(Image {
                    media_type: attachment["mimeType"].as_str()?.to_owned(), data: attachment["data"].as_str()?.to_owned(),
                })).collect();
                self.last_input = Some((text.clone(), images.clone()));
                let image_urls = images.iter().map(|image| format!("data:{};base64,{}", image.media_type, image.data)).collect();
                let index = self.push(Item::User { text, images: images.len(), image_urls, template: None, rewind_from: None });
                self.user_events.insert(index, event.id.clone());
            }
            "assistant.message_delta" | "assistant.message" => {
                self.finish_thinking();
                let id = data["messageId"].as_str().unwrap_or(&event.id).to_owned();
                let complete = event.event_type == "assistant.message";
                let text = data[if complete { "content" } else { "deltaContent" }].as_str().unwrap_or_default();
                if text.is_empty() { return; }
                let index = if let Some(index) = self.messages.get(&id) { *index } else {
                    let index = self.push(Item::Text { text: String::new(), streaming: !history, phase: Phase::Commentary });
                    self.messages.insert(id, index); index
                };
                if let Item::Text { text: content, streaming, .. } = &mut self.items[index] {
                    if complete { *content = text.to_owned(); } else { content.push_str(text); }
                    *streaming = !complete && !history;
                }
                self.changed(index);
            }
            "assistant.reasoning_delta" | "assistant.reasoning" => {
                let complete = event.event_type == "assistant.reasoning";
                let index = if let Some((index, _)) = self.thinking { index } else {
                    let index = self.push(Item::Thinking { seconds: 0, text: Some(String::new()) });
                    self.thinking = Some((index, Instant::now())); index
                };
                if let Item::Thinking { text: Some(text), .. } = &mut self.items[index] {
                    let content = data[if complete { "content" } else { "deltaContent" }].as_str().unwrap_or_default();
                    if complete { *text = content.to_owned(); } else { text.push_str(content); }
                }
                self.changed(index);
            }
            "tool.execution_start" => self.tool_start(data),
            "tool.user_requested" => {
                let (Some(id), Some(command)) = (data["toolCallId"].as_str(), data["arguments"]["command"].as_str()) else { return };
                if self.tools.contains_key(id) { return; }
                let index = self.push(Item::Shell { command: command.to_owned(), status: CommandStatus::Running, output: String::new(), pending: false });
                self.tools.insert(id.to_owned(), (index, data.clone()));
            }
            "tool.execution_complete" => self.tool_complete(data),
            "tool.execution_partial_result" | "tool.shell_output" => {
                if let Some((index, _)) = data["toolCallId"].as_str().and_then(|id| self.tools.get(id)) {
                    let index = *index;
                    let partial = event.event_type == "tool.execution_partial_result";
                    let text = data["partialOutput"].as_str().or(data["text"].as_str()).unwrap_or_default();
                    match &mut self.items[index] {
                        Item::Command { output, .. } if partial => *output = text.to_owned(),
                        Item::Command { output, .. } | Item::Shell { output, .. } if !partial => output.push_str(text),
                        _ => return,
                    }
                    self.changed(index);
                }
            }
            "session.idle" => self.finish(),
            "session.error" => {
                self.error(data["message"].as_str().unwrap_or("GitHub Copilot failed").to_owned());
                if !event.is_transient_error() { self.finish(); }
            }
            "assistant.usage" => {
                self.tokens.input += data["inputTokens"].as_u64().unwrap_or(0);
                self.tokens.output += data["outputTokens"].as_u64().unwrap_or(0);
                let _ = self.events.send(Event::Meters(None, Some(self.tokens)));
            }
            "session.quota_observation" if !history => {
                if let Some(data) = event.typed_data::<SessionQuotaObservationData>()
                    && data.observation.provider.id == "copilot" && data.observation.monthly_usage.is_some() {
                    let _ = self.events.send(Event::Usage(readings(&[data.observation])));
                }
            }
            "session.usage_info" => {
                if let (Some(used), Some(window)) = (data["currentTokens"].as_u64(), data["tokenLimit"].as_u64()) {
                    let _ = self.events.send(Event::Meters(Some(Context { used, window }), None));
                }
            }
            "session.compaction_complete" if data["success"] == true => {
                self.push(Item::compacted());
                if !history && let (Some(used), Some(window)) = (data["postCompactionTokens"].as_u64(), data["tokenLimit"].as_u64()) {
                    let _ = self.events.send(Event::Meters(Some(Context { used, window }), None));
                }
            }
            "session.model_change" => {
                if let Some(model) = data["newModel"].as_str() { self.settings.chosen.model = Some(model.to_owned()); self.report_settings(); }
            }
            "session.commands_changed" => {
                self.capabilities.commands = data["commands"].as_array().into_iter().flatten().filter_map(|command| Some(AgentCommand {
                    name: command["name"].as_str()?.to_owned(), description: command["description"].as_str().unwrap_or_default().to_owned(), hint: String::new(),
                })).collect();
                self.report_settings();
            }
            _ => {}
        }
    }

    fn tool_start(&mut self, data: &Value) {
        self.finish_thinking();
        let Some(id) = data["toolCallId"].as_str() else { return };
        if self.tools.contains_key(id) { return; }
        let name = data["toolName"].as_str().unwrap_or("tool");
        let args = &data["arguments"];
        let item = if let Some(command) = args["command"].as_str().filter(|_| matches!(name, "bash" | "powershell" | "shell" | "local_shell")) {
            Item::Command { command: command.to_owned(), status: CommandStatus::Running, output: String::new() }
        } else if data["mcpServerName"] == html::SERVER && data["mcpToolName"] == html::TOOL {
            html::item(args).unwrap_or_else(|| tool(name, args, ToolKind::Mcp))
        } else {
            tool(name, args, match name { "view" | "read_file" => ToolKind::Read, "grep" | "glob" => ToolKind::Search, "web_fetch" => ToolKind::Fetch, "web_search" => ToolKind::Web, _ if data["mcpServerName"].is_string() => ToolKind::Mcp, _ => ToolKind::Other })
        };
        let index = self.push(item);
        self.tools.insert(id.to_owned(), (index, data.clone()));
    }

    fn tool_complete(&mut self, data: &Value) {
        let Some((index, start)) = data["toolCallId"].as_str().and_then(|id| self.tools.get(id)).cloned() else { return };
        if let Item::Shell { status, output, .. } = &mut self.items[index] {
            *status = if data["success"] == true { CommandStatus::Ok } else { CommandStatus::Failed };
            if output.is_empty() && let Some(content) = data["result"]["content"].as_str().or(data["error"]["message"].as_str()) { *output = content.to_owned(); }
            self.changed(index);
        } else if let Item::Command { status, output, .. } = &mut self.items[index] {
            *status = if data["success"] == true { CommandStatus::Ok } else { CommandStatus::Failed };
            if let Some(content) = data["result"]["content"].as_str().or(data["error"]["message"].as_str()) { *output = content.to_owned(); }
            self.changed(index);
        } else if data["success"] == true && start["mcpServerName"] == html::SERVER && start["mcpToolName"] == crate::media::TOOL {
            if let Some(item) = crate::media::item(&data["result"]) { self.items[index] = item; self.changed(index); }
        } else if data["success"] == true && matches!(start["toolName"].as_str(), Some("edit" | "create")) {
            let args = &start["arguments"];
            if let Some(path) = args["path"].as_str() {
                let (old, new) = if start["toolName"] == "create" { ("", args["file_text"].as_str().unwrap_or_default()) } else {
                    (args["old_str"].as_str().unwrap_or_default(), args["new_str"].as_str().unwrap_or_default())
                };
                self.items[index] = Item::edit(relative(&self.cwd, path), unified(old, new), start["toolName"] == "create");
                self.changed(index);
            }
        }
    }
}

fn tool(name: &str, args: &Value, kind: ToolKind) -> Item {
    let detail = args["path"].as_str().or(args["pattern"].as_str()).or(args["url"].as_str()).map(str::to_owned).unwrap_or_else(|| args.to_string());
    Item::Tools { calls: vec![ToolCall { kind, name: name.to_owned(), detail }] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lost_commands_keep_output_and_completed_results() {
        let (events, _) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new(Path::new("/project"), events, Settings::default());
        for (index, (kind, data)) in [
            ("tool.execution_start", json!({ "toolCallId": "agent", "toolName": "bash", "arguments": { "command": "pnpm dev" } })),
            ("tool.execution_partial_result", json!({ "toolCallId": "agent", "partialOutput": "Ready" })),
            ("tool.execution_start", json!({ "toolCallId": "done", "toolName": "bash", "arguments": { "command": "true" } })),
            ("tool.execution_complete", json!({ "toolCallId": "done", "success": true })),
            ("tool.user_requested", json!({ "toolCallId": "user", "arguments": { "command": "make" } })),
            ("tool.shell_output", json!({ "toolCallId": "user", "text": "cc main.c\n" })),
        ].into_iter().enumerate() {
            let event = serde_json::from_value::<SessionEvent>(json!({ "id": index.to_string(), "timestamp": "", "parentId": null, "type": kind, "data": data })).unwrap();
            transcript.read(&event, true);
        }
        transcript.stop_commands();
        assert!(matches!(&transcript.items[0], Item::Command { status: CommandStatus::Failed, output, .. } if output == "Ready"));
        assert!(matches!(&transcript.items[1], Item::Command { status: CommandStatus::Ok, .. }));
        assert!(matches!(&transcript.items[2], Item::Shell { status: CommandStatus::Failed, output, .. } if output == "cc main.c\n"));
    }

    #[test]
    fn user_shell_commands_stream_only_shell_output_and_keep_it_when_cancelled() {
        let (events, mut received) = mpsc::unbounded_channel();
        let mut transcript = Transcript::new(Path::new("/project"), events, Settings::default());
        let event = |id: &str, kind: &str, data: Value| serde_json::from_value::<SessionEvent>(json!({ "id": id, "timestamp": "", "parentId": null, "type": kind, "data": data })).unwrap();
        transcript.read(&event("1", "tool.user_requested", json!({ "toolCallId": "call", "toolName": "local_shell", "arguments": { "command": "make" } })), false);
        transcript.read(&event("2", "tool.shell_output", json!({ "toolCallId": "call", "stream": "stdout", "text": "cc main.c\n" })), false);
        transcript.read(&event("3", "tool.execution_partial_result", json!({ "toolCallId": "call", "partialOutput": "cc main.c\n" })), false);
        transcript.read(&event("4", "tool.execution_complete", json!({ "toolCallId": "call", "isUserRequested": true, "success": false, "error": { "message": "cc main.c\ncc main.c\nShell command canceled." } })), false);
        let mut items = BTreeMap::new();
        while let Ok(event) = received.try_recv() {
            if let Event::Item(index, item) = event { items.insert(index, item); }
        }
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[&0], Item::Shell { command, status: CommandStatus::Failed, output, .. } if command == "make" && output == "cc main.c\n"));
    }
}
