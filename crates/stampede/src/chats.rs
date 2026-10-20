pub(crate) mod claude;
pub(crate) mod codex;
pub(crate) mod copilot;
mod delegation;
mod terminal_tools;
mod holder;
mod html;
mod protocol;
mod questions;

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use base64ct::{Base64, Encoding};
use tokio::sync::{
    broadcast,
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    oneshot,
};
use uuid::Uuid;

use crate::{
    attention::{Attention, Feed, Source, Tab},
    config::{Agent, AgentSettings, Rig},
    projects::Project,
    templates,
    usage::{Reading, Snapshot, Usage},
};
pub use holder::{HoldChatArgs, hold_chat};
pub use html::{ChatToolsArgs, serve as chat_tools};
pub use delegation::listen as listen_delegations;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub enum Item {
    User {
        text: String,
        images: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        template: Option<&'static str>,
    },
    Text { text: String, streaming: bool, phase: Phase },
    Thinking {
        seconds: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    Tools { calls: Vec<ToolCall> },
    Html { title: String, html: String },
    Media { title: String, asset: String },
    Edit {
        path: String,
        diff: String,
        added: usize,
        removed: usize,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        write: bool,
    },
    Command { command: String, status: CommandStatus, output: String },
    Subagent {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        task: String,
        status: Status,
        #[serde(skip_serializing_if = "Option::is_none")]
        outcome: Option<Outcome>,
        #[serde(skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        report: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        started_at: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        totals: Option<Totals>,
        #[serde(skip_serializing_if = "Option::is_none")]
        activity: Option<String>,
        items: Vec<Item>,
    },
    Notice { notice: Notice, text: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Completed,
    Failed,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub duration_ms: u64,
    pub tools: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ToolCall {
    pub kind: ToolKind,
    pub name: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolKind {
    Read,
    Search,
    Fetch,
    Web,
    Mcp,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Commentary,
    Final,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandStatus {
    Running,
    Ok,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Notice {
    Compacted,
    Error,
    Limit,
    Interrupted,
    Rewound,
}

impl Item {
    fn compacted() -> Self {
        Item::Notice { notice: Notice::Compacted, text: "Conversation compacted".to_owned() }
    }

    fn interrupted() -> Self {
        Item::Notice { notice: Notice::Interrupted, text: "Interrupted".to_owned() }
    }

    fn edit(path: String, diff: String, write: bool) -> Self {
        let count = |sign: char| diff.lines().skip_while(|line| !line.starts_with("@@")).filter(|line| line.starts_with(sign)).count();
        Item::Edit { path, added: count('+'), removed: count('-'), diff, write }
    }
}

fn seconds(milliseconds: u64) -> u64 {
    (milliseconds + 500) / 1000
}

fn relative(cwd: &Path, path: &str) -> String {
    Path::new(path).strip_prefix(cwd).map_or_else(|_| path.to_owned(), |path| path.display().to_string())
}

fn unified(old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }
    let (old, new): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let prefix = old.iter().zip(&new).take_while(|(old, new)| old == new).count();
    let suffix = old[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(old, new)| old == new).count();
    let (start, after) = (prefix.saturating_sub(3), suffix.min(3));
    let (old_end, new_end) = (old.len() - suffix, new.len() - suffix);
    let range = |end: usize| {
        let count = end + after - start;
        format!("{},{count}", if count == 0 { start } else { start + 1 })
    };
    let mut diff = format!("@@ -{} +{} @@\n", range(old_end), range(new_end));
    let lines = old[start..prefix].iter().map(|line| (' ', line))
        .chain(old[prefix..old_end].iter().map(|line| ('-', line)))
        .chain(new[prefix..new_end].iter().map(|line| ('+', line)))
        .chain(old[old_end..old_end + after].iter().map(|line| (' ', line)));
    for (sign, line) in lines {
        diff.push(sign);
        diff.push_str(line);
        diff.push('\n');
    }
    diff
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Todo {
    pub text: String,
    pub status: TodoStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    Pending,
    Active,
    Done,
}

struct Subagent {
    index: usize,
    id: String,
    name: Option<String>,
    task: String,
    status: Status,
    outcome: Option<Outcome>,
    prompt: Option<String>,
    report: Option<String>,
    model: Option<String>,
    started_at: u64,
    finished_at: u64,
    duration: Option<u64>,
    tools: Option<usize>,
    tokens: Option<u64>,
    activity: Option<String>,
    events: UnboundedReceiver<Event>,
    items: Vec<Item>,
}

impl Subagent {
    fn new(id: &str, task: String, started_at: u64) -> (Self, UnboundedSender<Event>) {
        let (sender, events) = mpsc::unbounded_channel();
        let subagent = Self {
            index: 0,
            id: id.to_owned(),
            name: None,
            task,
            status: Status::Working,
            outcome: None,
            prompt: None,
            report: None,
            model: None,
            started_at,
            finished_at: started_at,
            duration: None,
            tools: None,
            tokens: None,
            activity: None,
            events,
            items: Vec::new(),
        };
        (subagent, sender)
    }

    fn update(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Item(index, item) => {
                    match self.items.get_mut(index) {
                        Some(existing) => *existing = item,
                        None => self.items.push(item),
                    }
                }
                Event::Activity(activity) => self.activity = activity,
                Event::Status(Status::Done) => self.finish(None),
                Event::Status(status) => self.status = status,
                Event::Meters(_, Some(tokens)) => self.tokens = Some(tokens.input + tokens.output),
                _ => continue,
            }
            changed = true;
        }
        changed
    }

    fn finish(&mut self, outcome: Option<Outcome>) {
        if self.status != Status::Done {
            self.finished_at = now();
        }
        self.status = Status::Done;
        self.outcome = outcome.or(self.outcome);
    }

    /// A final-phase text is the Subagent's report, not one of its items.
    fn item(&self) -> Item {
        let done = self.status == Status::Done;
        let report = self.items.iter().rev().find_map(|item| match item {
            Item::Text { text, phase: Phase::Final, .. } => Some(text.clone()),
            _ => None,
        });
        let items: Vec<Item> = self.items.iter().filter(|item| !matches!(item, Item::Text { phase: Phase::Final, .. })).cloned().collect();
        let totals = done.then(|| Totals {
            duration_ms: self.duration.unwrap_or(self.finished_at.saturating_sub(self.started_at)),
            tools: self.tools.unwrap_or_else(|| items.iter().map(|item| match item {
                Item::Tools { calls } => calls.len(),
                Item::Command { .. } | Item::Edit { .. } | Item::Html { .. } => 1,
                _ => 0,
            }).sum()),
            tokens: self.tokens,
        });
        Item::Subagent {
            id: self.id.clone(),
            name: self.name.clone(),
            task: self.task.clone(),
            status: self.status,
            outcome: self.outcome.filter(|_| done),
            prompt: self.prompt.clone(),
            report: self.report.clone().or(report),
            model: self.model.clone(),
            started_at: self.started_at,
            totals,
            activity: self.activity.clone().filter(|_| !done),
            items,
        }
    }
}

fn appended(existing: &Item, item: &Item) -> Option<String> {
    let (before, after) = match (existing, item) {
        (Item::Text { text: before, streaming: true, phase }, Item::Text { text: after, streaming: true, phase: same }) if phase == same => (before, after),
        (Item::Command { command, status, output: before }, Item::Command { command: same, status: unchanged, output: after })
            if command == same && status == unchanged => (before, after),
        _ => return None,
    };
    after.strip_prefix(before.as_str()).map(str::to_owned)
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64)
}

#[derive(Default)]
struct Activity(Vec<(String, String)>);

impl Activity {
    fn start(&mut self, id: &str, label: String, events: &UnboundedSender<Event>) {
        self.0.retain(|(other, _)| other != id);
        self.0.push((id.to_owned(), label));
        self.report(events);
    }

    fn finish(&mut self, id: &str, events: &UnboundedSender<Event>) {
        let before = self.0.len();
        self.0.retain(|(other, _)| other != id);
        if before != self.0.len() { self.report(events); }
    }

    fn clear(&mut self, events: &UnboundedSender<Event>) {
        if self.0.is_empty() { return; }
        self.0.clear();
        self.report(events);
    }

    fn report(&self, events: &UnboundedSender<Event>) {
        let _ = events.send(Event::Activity(self.0.last().map(|(_, label)| label.clone())));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Context {
    pub used: u64,
    pub window: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Request {
    #[serde(flatten)]
    pub kind: RequestKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub enum RequestKind {
    Command {
        id: String,
        words: Vec<Vec<String>>,
        suggestion: Vec<Vec<String>>,
        session_prefixes: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        allow_rules: Option<bool>,
    },
    Question { id: String, questions: Vec<Question> },
    Plan { id: String, plan: String },
    Edit { id: String, path: String, diff: String },
    Other { id: String, title: String, detail: String },
}

impl Request {
    pub fn id(&self) -> &str {
        self.kind.id()
    }
}

impl RequestKind {
    pub fn id(&self) -> &str {
        match self {
            RequestKind::Command { id, .. } | RequestKind::Edit { id, .. } | RequestKind::Other { id, .. }
            | RequestKind::Question { id, .. } | RequestKind::Plan { id, .. } => id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(from = "Decision")]
pub enum Answer {
    Allow,
    Rules(Vec<Rule>),
    Answers(Vec<Vec<String>>),
    Responses(HashMap<String, questions::Response>),
    Deny { instead: Option<String> },
}

#[derive(Deserialize)]
struct Decision {
    #[serde(default)]
    allow: bool,
    #[serde(default)]
    rule: Vec<Rule>,
    answers: Option<Vec<Vec<String>>>,
    responses: Option<HashMap<String, questions::Response>>,
    #[serde(default)]
    instead: Option<String>,
}

impl From<Decision> for Answer {
    fn from(Decision { allow, instead, rule, answers, responses }: Decision) -> Self {
        if let Some(responses) = responses { Answer::Responses(responses) }
        else if let Some(answers) = answers { Answer::Answers(answers) }
        else if allow && !rule.is_empty() { Answer::Rules(rule) }
        else if allow { Answer::Allow } else { Answer::Deny { instead } }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    #[serde(default)]
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(default)]
    pub options: Vec<ChoiceDescription>,
    #[serde(default)]
    pub multi_select: bool,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub answer_type: Option<questions::Type>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<questions::Image>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceDescription {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recommended: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<questions::Image>,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnsweredQuestions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub after: usize,
    pub questions: Vec<Question>,
    pub answers: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub responses: HashMap<String, questions::Response>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Rule {
    pub prefix: Vec<String>,
    pub scope: Scope,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope { Chat, Always }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub media_type: String,
    pub data: String,
}

impl Image {
    fn valid(&self) -> bool {
        matches!(self.media_type.as_str(), "image/png" | "image/jpeg" | "image/gif" | "image/webp")
            && self.data.len() <= 5 * 1024 * 1024 * 4 / 3 + 4
            && Base64::decode_vec(&self.data).is_ok_and(|data| !data.is_empty() && data.len() <= 5 * 1024 * 1024)
    }
}

pub enum Event {
    Recoverable,
    Session(String),
    ResumeAt(Option<String>),
    PlanResolved(String),
    QuestionsTruncated(usize),
    QuestionsRecovered(AnsweredQuestions),
    Status(Status),
    Activity(Option<String>),
    /// Appends the Item when its index is the transcript's length, and replaces the one there otherwise.
    Item(usize, Item),
    /// Replaces whichever of the Chat's context fill and token counts it carries.
    Meters(Option<Context>, Option<Tokens>),
    Usage(Vec<Reading>),
    Truncate(usize),
    Request(Request),
    Resolved(String),
    Todos(Vec<Todo>),
    Reset,
    Settings(Settings),
    Capabilities(Capabilities),
}

/// One Agent's wire protocol. A driver's events end when its Agent has stopped.
pub trait Driver: Send + Sync {
    /// Starts a turn, or steers the running one.
    fn send(&self, text: &str, images: &[Image]);
    fn retry(&self);
    fn interrupt(&self);
    fn answer(&self, request_id: &str, answer: Answer);
    fn run(&self, builtin: Builtin);
    fn close(&self);
    fn delegated(&self, item: Item);
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub rewinds_files: bool,
    pub models: Vec<String>,
    pub efforts: Vec<String>,
    pub modes: Vec<Choice>,
    pub commands: Vec<AgentCommand>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

impl Choice {
    pub fn mode(value: &str) -> Self {
        let mut label = String::new();
        for character in value.chars() {
            match character {
                '-' => label.push(' '),
                upper if upper.is_ascii_uppercase() => label.extend([' ', upper.to_ascii_lowercase()]),
                other => label.push(other),
            }
        }
        Self { value: value.to_owned(), label }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentCommand {
    pub name: String,
    pub description: String,
    pub hint: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(flatten)]
    pub chosen: AgentSettings,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending: Vec<Setting>,
}

impl Settings {
    pub fn get(&self, setting: Setting) -> &Option<String> {
        match setting {
            Setting::Model => &self.chosen.model,
            Setting::Effort => &self.chosen.effort,
            Setting::Mode => &self.chosen.mode,
        }
    }

    pub fn get_mut(&mut self, setting: Setting) -> &mut Option<String> {
        match setting {
            Setting::Model => &mut self.chosen.model,
            Setting::Effort => &mut self.chosen.effort,
            Setting::Mode => &mut self.chosen.mode,
        }
    }

    pub fn mark_pending(&mut self, setting: Setting) {
        if !self.pending.contains(&setting) {
            self.pending.push(setting);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Setting {
    Model,
    Effort,
    Mode,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", content = "value", rename_all = "lowercase")]
pub enum Builtin {
    Model(String),
    Effort(String),
    Mode(String),
    Compact,
    Clear,
    Rewind(usize),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: String,
    pub project_id: String,
    pub rig: String,
    pub agent: Agent,
    pub cwd: PathBuf,
    pub created: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_resolved: Option<String>,
    pub status: Status,
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    pub viewed_since_done: bool,
    #[serde(default)]
    pub context: Option<Context>,
    #[serde(default)]
    pub tokens: Option<Tokens>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub queued: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued_images: Vec<Vec<Image>>,
    #[serde(default)]
    pub todos: Vec<Todo>,
    #[serde(default)]
    pub answered_questions: Vec<AnsweredQuestions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub agent: Agent,
    pub session_id: String,
    pub first_prompt: String,
    pub updated_at: u64,
}

const PROMPT: usize = 200;

impl Session {
    fn new(agent: Agent, session_id: &str, prompt: &str, updated_at: u64) -> Self {
        let first_prompt = prompt.chars().take(PROMPT).collect();
        Self { agent, session_id: session_id.to_owned(), first_prompt, updated_at }
    }
}

pub async fn sessions(cwd: &Path, claude: &Rig, codex: &Rig, copilot: &Rig) -> Vec<Session> {
    let mut sessions = Vec::new();
    for (agent, listed) in [
        (Agent::ClaudeCode, claude::sessions(cwd, &claude.env)),
        (Agent::Codex, codex::sessions(cwd, &codex.env).await),
        (Agent::Copilot, copilot::sessions(cwd, &copilot.env).await),
    ] {
        match listed {
            Ok(listed) => sessions.extend(listed),
            Err(error) => eprintln!("cannot list {} sessions: {error}", agent.name()),
        }
    }
    sessions.sort_by_key(|session| std::cmp::Reverse(session.updated_at));
    sessions
}

#[derive(Clone)]
pub enum Change {
    Chats(String),
    Status(String, Status),
    Activity(String, Option<String>),
    Queued(String, Vec<String>),
    Truncated(String),
    Item { chat_id: String, seq: u64, index: usize, item: Item },
    Appended { chat_id: String, seq: u64, index: usize, text: String },
    Meters { chat_id: String, context: Option<Context>, tokens: Option<Tokens> },
    Request { chat_id: String, seq: u64, request: Request },
    Resolved { chat_id: String, seq: u64, request_id: String },
    Todos { chat_id: String, todos: Vec<Todo> },
    Settings(String, Settings),
    Capabilities(String, Capabilities),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Working,
    Waiting,
    Done,
}

/// A step in a Chat's life, the same for every Agent: what the loop follows instead of the drivers' events.
#[derive(Clone)]
#[expect(dead_code, reason = "scheduling needs only which Chat changed; releasing and recovering Chats read what changed")]
pub enum Lifecycle {
    Status(String, Status),
    Session(String, String),
    /// An interrupted turn, or one cut off by a usage limit.
    Notice(String, Notice),
    /// The Chat has stopped and its record is gone; this is its last state.
    Ended(Box<Chat>),
}

impl Lifecycle {
    pub fn chat(&self) -> &str {
        match self {
            Lifecycle::Status(chat, _) | Lifecycle::Session(chat, _) | Lifecycle::Notice(chat, _) => chat,
            Lifecycle::Ended(chat) => &chat.id,
        }
    }
}

pub struct Chats {
    data_dir: PathBuf,
    chats: Mutex<Vec<Arc<Entry>>>,
    changed: broadcast::Sender<Change>,
    lifecycle: broadcast::Sender<Lifecycle>,
    views: Mutex<HashMap<Uuid, (Option<String>, Vec<String>)>>,
    attention: Arc<Feed>,
    usage: Arc<Usage>,
}

struct Entry {
    chat: Mutex<Chat>,
    transcript: Mutex<Transcript>,
    capabilities: Mutex<Capabilities>,
    driver: Box<dyn Driver>,
    _storage: crate::storage::Session,
}

struct Transcript {
    recoverable: bool,
    seq: u64,
    items: Vec<Item>,
    requests: Vec<Request>,
    reported: Status,
    handoff: Option<Handoff>,
    questions: HashMap<String, oneshot::Sender<questions::Reply>>,
}

struct Handoff {
    after: usize,
    reply: oneshot::Sender<io::Result<String>>,
}

impl Chats {
    pub fn new(data_dir: PathBuf, attention: Arc<Feed>, usage: Arc<Usage>) -> Self {
        Self {
            data_dir,
            chats: Mutex::default(),
            changed: broadcast::Sender::new(1024),
            lifecycle: broadcast::Sender::new(256),
            views: Mutex::default(),
            attention,
            usage,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Change> {
        self.changed.subscribe()
    }

    pub fn lifecycle(&self) -> broadcast::Receiver<Lifecycle> {
        self.lifecycle.subscribe()
    }

    pub fn list(&self) -> Vec<Chat> {
        self.chats.lock().unwrap().iter().map(|entry| entry.chat.lock().unwrap().clone()).filter(|chat| chat.parent.is_none()).collect()
    }

    pub fn rename_rig(&self, original: &str, name: &str) -> io::Result<()> {
        for entry in self.chats.lock().unwrap().iter() {
            let mut chat = entry.chat.lock().unwrap();
            if chat.rig == original {
                let mut renamed = chat.clone();
                name.clone_into(&mut renamed.rig);
                write_record(&self.data_dir, &renamed)?;
                *chat = renamed;
                let _ = self.changed.send(Change::Chats(chat.project_id.clone()));
            }
        }
        self.refresh_attention();
        Ok(())
    }

    fn children(&self, id: &str) -> Vec<String> {
        self.chats.lock().unwrap().iter().filter_map(|entry| {
            let chat = entry.chat.lock().unwrap();
            (chat.parent.as_deref() == Some(id)).then(|| chat.id.clone())
        }).collect()
    }

    pub fn owned(&self, project_id: &str) -> Vec<Chat> {
        self.list().into_iter().filter(|chat| chat.project_id == project_id).collect()
    }

    pub fn hand_over(&self, from: &str, to: &str) {
        for entry in self.chats.lock().unwrap().iter() {
            let mut chat = entry.chat.lock().unwrap();
            if chat.project_id == from {
                to.clone_into(&mut chat.project_id);
                self.save(&chat);
            }
        }
        let _ = self.changed.send(Change::Chats(to.to_owned()));
        let _ = self.changed.send(Change::Chats(from.to_owned()));
        self.refresh_attention();
    }

    /// Starts a Chat owned by `project` that runs in `cwd`: the Project's own directory, or a Ticket Worktree.
    pub async fn create(self: &Arc<Self>, project: &Project, cwd: &Path, rig: &Rig, session: Option<&str>) -> io::Result<Chat> {
        self.start(&project.id, cwd, rig, session, None, None).await
    }

    pub async fn create_handoff(self: &Arc<Self>, project: &Project, source: &Chat, rig: &Rig, text: String) -> io::Result<Chat> {
        self.start(&project.id, &source.cwd, rig, None, Some((source.id.clone(), text)), None).await
    }

    async fn start(self: &Arc<Self>, project_id: &str, cwd: &Path, rig: &Rig, session: Option<&str>, handoff: Option<(String, String)>, parent: Option<String>) -> io::Result<Chat> {
        let (handoff_from, draft) = match handoff {
            Some((source, text)) => (Some(source), Some(text)),
            None => (None, None),
        };
        let chat = Chat {
            id: format!("{:016x}", Uuid::new_v4().as_u64_pair().1),
            project_id: project_id.to_owned(),
            rig: rig.name.clone(),
            agent: rig.agent,
            cwd: cwd.to_owned(),
            created: SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos(),
            agent_session: session.map(str::to_owned),
            resume_at: None,
            plan_resolved: None,
            status: Status::Done,
            activity: None,
            viewed_since_done: true,
            context: None,
            tokens: None,
            settings: Settings { chosen: rig.settings(), pending: Vec::new() },
            queued: Vec::new(),
            queued_images: Vec::new(),
            todos: Vec::new(),
            answered_questions: Vec::new(),
            handoff_from,
            parent,
            draft,
        };
        fs::create_dir_all(self.data_dir.join("chats"))?;
        let storage = crate::storage::Storage::new(&self.data_dir).prepare(&chat.id, cwd, &rig.env).await?;
        write_record(&self.data_dir, &chat)?;
        let started = match rig.agent {
            Agent::ClaudeCode => claude::start(&self.data_dir, &chat.id, &chat.cwd, &storage.env, &chat.settings.chosen, session)
                .await
                .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
            Agent::Codex if session.is_some() => codex::resume(&self.data_dir, &chat, &storage.env)
                .await
                .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
            Agent::Codex => codex::start(&self.data_dir, &chat.id, &chat.cwd, &storage.env, &chat.settings.chosen)
                .await
                .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
            Agent::Copilot => copilot::start(&self.data_dir, &chat, &storage.env)
                .await
                .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
        };
        let (driver, events) = started.inspect_err(|_| {
            let _ = fs::remove_file(record_path(&self.data_dir, &chat.id));
        })?;
        self.adopt(chat.clone(), driver, events, storage);
        let _ = self.changed.send(Change::Chats(chat.project_id.clone()));
        Ok(chat)
    }

    pub async fn restore(self: &Arc<Self>, rigs: &[Rig]) -> io::Result<()> {
        let entries = match fs::read_dir(self.data_dir.join("chats")) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            entries => entries?,
        };
        let mut chats = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path.extension() != Some("json".as_ref()) {
                continue;
            }
            match read_record(&path) {
                Ok(chat) => chats.push(chat),
                Err(error) => eprintln!("cannot read {}: {error}", path.display()),
            }
        }
        chats.sort_by_key(|chat| chat.created);
        for chat in chats {
            let env = rigs.iter().find(|rig| rig.name == chat.rig).map(|rig| rig.env.clone()).unwrap_or_default();
            let storage = match crate::storage::Storage::new(&self.data_dir).prepare(&chat.id, &chat.cwd, &env).await {
                Ok(storage) => storage,
                Err(error) => { eprintln!("cannot restore storage for chat {}: {error}", chat.id); continue }
            };
            let env = &storage.env;
            let reattached = match chat.agent {
                Agent::ClaudeCode => match claude::reattach(&self.data_dir, &chat, &env).await {
                    Err(error)
                        if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) =>
                    {
                        claude::resume(&self.data_dir, &chat, &env).await
                    }
                    reattached => reattached,
                }
                .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
                Agent::Codex => codex::resume(&self.data_dir, &chat, &env)
                    .await
                    .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
                Agent::Copilot => copilot::start(&self.data_dir, &chat, &env)
                    .await
                    .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
            };
            match reattached {
                Ok((driver, events)) => {
                    let parent = chat.parent.clone();
                    let id = chat.id.clone();
                    self.adopt(chat, driver, events, storage);
                    if parent.is_some() { self.close(&id); }
                },
                Err(error) => eprintln!("cannot restore chat {}: {error}", chat.id),
            }
        }
        self.refresh_attention();
        Ok(())
    }

    fn adopt(self: &Arc<Self>, chat: Chat, driver: Box<dyn Driver>, events: UnboundedReceiver<Event>, storage: crate::storage::Session) {
        let transcript = Transcript { recoverable: false, seq: 0, items: Vec::new(), requests: Vec::new(), reported: chat.status, handoff: None, questions: HashMap::new() };
        let entry = Arc::new(Entry {
            _storage: storage,
            chat: Mutex::new(chat),
            transcript: Mutex::new(transcript),
            capabilities: Mutex::default(),
            driver,
        });
        self.chats.lock().unwrap().push(Arc::clone(&entry));
        tokio::spawn(Arc::clone(self).follow(entry, events));
    }

    async fn follow(self: Arc<Self>, entry: Arc<Entry>, mut events: UnboundedReceiver<Event>) {
        let id = entry.chat.lock().unwrap().id.clone();
        while let Some(event) = events.recv().await {
            match event {
                Event::Recoverable => entry.transcript.lock().unwrap().recoverable = true,
                Event::Activity(activity) => {
                    let mut chat = entry.chat.lock().unwrap();
                    if chat.activity != activity {
                        chat.activity = activity.clone();
                        let _ = self.changed.send(Change::Activity(id.clone(), activity));
                    }
                }
                Event::PlanResolved(id) => {
                    let mut chat = entry.chat.lock().unwrap();
                    chat.plan_resolved = Some(id);
                    self.save(&chat);
                }
                Event::QuestionsTruncated(after) => {
                    let mut chat = entry.chat.lock().unwrap();
                    chat.answered_questions.retain(|record| record.after <= after);
                    self.save(&chat);
                    let _ = self.changed.send(Change::Chats(chat.project_id.clone()));
                }
                Event::QuestionsRecovered(record) => {
                    let mut chat = entry.chat.lock().unwrap();
                    let existing = chat.answered_questions.iter_mut().find(|saved| {
                        saved.source == record.source || (saved.source.is_none() && saved.questions == record.questions
                            && saved.answers.iter().map(|answers| answers.join(", ")).eq(record.answers.iter().map(|answers| answers.join(", "))))
                    });
                    if let Some(existing) = existing {
                        if *existing == record { continue; }
                        *existing = record;
                    } else {
                        chat.answered_questions.push(record);
                    }
                    self.save(&chat);
                    let _ = self.changed.send(Change::Chats(chat.project_id.clone()));
                }
                Event::ResumeAt(at) => {
                    let mut chat = entry.chat.lock().unwrap();
                    chat.resume_at = at;
                    self.save(&chat);
                }
                Event::Session(session) => {
                    let mut chat = entry.chat.lock().unwrap();
                    if chat.agent_session.as_ref() != Some(&session) {
                        if chat.agent_session.is_some() { chat.answered_questions.clear(); }
                        chat.agent_session = Some(session.clone());
                        self.save(&chat);
                        let _ = self.changed.send(Change::Chats(chat.project_id.clone()));
                        let _ = self.lifecycle.send(Lifecycle::Session(id.clone(), session));
                    }
                }
                Event::Status(status) => {
                    let cut_off = matches!(
                        entry.transcript.lock().unwrap().items.last(),
                        Some(Item::Notice { notice: Notice::Interrupted | Notice::Limit, .. })
                    );
                    let mut chat = entry.chat.lock().unwrap();
                    if status == Status::Done && !chat.queued.is_empty() && !cut_off {
                        let text = chat.queued.remove(0);
                        let images = if chat.queued_images.is_empty() { Vec::new() } else { chat.queued_images.remove(0) };
                        self.queued(&chat);
                        entry.driver.send(&templates::expand(&text, &self.data_dir), &images);
                        continue;
                    }
                    drop(chat);
                    let mut transcript = entry.transcript.lock().unwrap();
                    if status == Status::Working { transcript.recoverable = false; }
                    transcript.reported = status;
                    if status == Status::Done {
                        let pending: Vec<_> =
                            transcript.requests.iter().map(|request| request.id().to_owned()).collect();
                        for request_id in pending {
                            self.resolve(&mut transcript, &id, &request_id);
                        }
                        if let Some(handoff) = transcript.handoff.take() {
                            let items = &transcript.items[handoff.after.min(transcript.items.len())..];
                            let failed = items.iter().any(|item| matches!(item, Item::Notice { notice: Notice::Error | Notice::Interrupted | Notice::Limit, .. }));
                            let text = items.iter().filter_map(|item| match item {
                                Item::Text { text, streaming: false, phase: Phase::Final } => Some(text.as_str()),
                                _ => None,
                            }).collect::<Vec<_>>().join("\n\n");
                            let result = if failed || text.trim().is_empty() {
                                Err(io::Error::other("The Agent did not finish a handoff. Try again when the Chat is ready."))
                            } else { Ok(text) };
                            let _ = handoff.reply.send(result);
                        }
                    }
                    self.settle(&entry, &transcript);
                }
                Event::Request(request) => {
                    let mut transcript = entry.transcript.lock().unwrap();
                    transcript.recoverable = false;
                    transcript.requests.retain(|other| other.id() != request.id());
                    transcript.requests.push(request.clone());
                    transcript.seq += 1;
                    let seq = transcript.seq;
                    let _ = self.changed.send(Change::Request { chat_id: id.clone(), seq, request });
                    self.settle(&entry, &transcript);
                }
                Event::Resolved(request_id) => {
                    let mut transcript = entry.transcript.lock().unwrap();
                    if self.resolve(&mut transcript, &id, &request_id) {
                        self.settle(&entry, &transcript);
                    }
                }
                Event::Meters(context, tokens) => {
                    let mut chat = entry.chat.lock().unwrap();
                    let before = (chat.context, chat.tokens);
                    chat.context = context.or(chat.context);
                    chat.tokens = tokens.or(chat.tokens);
                    if (chat.context, chat.tokens) == before {
                        continue;
                    }
                    self.save(&chat);
                    let (context, tokens) = (chat.context, chat.tokens);
                    let _ = self.changed.send(Change::Meters { chat_id: id.clone(), context, tokens });
                }
                Event::Todos(todos) => {
                    let mut chat = entry.chat.lock().unwrap();
                    if chat.todos == todos {
                        continue;
                    }
                    chat.todos = todos.clone();
                    self.save(&chat);
                    let _ = self.changed.send(Change::Todos { chat_id: id.clone(), todos });
                }
                Event::Item(index, mut item) => {
                    if let Item::User { text, template, .. } = &mut item
                        && let Some((name, typed)) = templates::recognize(text, &self.data_dir)
                    {
                        (*template, *text) = (Some(name), typed);
                    }
                    let mut transcript = entry.transcript.lock().unwrap();
                    if matches!(&item, Item::User { .. }) { transcript.recoverable = false; }
                    let appended = transcript.items.get(index).and_then(|existing| appended(existing, &item));
                    match transcript.items.get_mut(index) {
                        Some(existing) => *existing = item.clone(),
                        None => {
                            if let Item::Notice { notice: notice @ (Notice::Interrupted | Notice::Limit), .. } = &item {
                                let _ = self.lifecycle.send(Lifecycle::Notice(id.clone(), *notice));
                            }
                            transcript.items.push(item.clone())
                        }
                    }
                    transcript.seq += 1;
                    let seq = transcript.seq;
                    let change = match appended {
                        Some(text) => Change::Appended { chat_id: id.clone(), seq, index, text },
                        None => Change::Item { chat_id: id.clone(), seq, index, item },
                    };
                    let _ = self.changed.send(change);
                }
                Event::Usage(readings) => self.usage.record(readings),
                Event::Truncate(len) => {
                    let mut transcript = entry.transcript.lock().unwrap();
                    transcript.recoverable = false;
                    transcript.items.truncate(len);
                    transcript.requests.clear();
                    transcript.seq += 1;
                    let _ = self.changed.send(Change::Truncated(id.clone()));
                }
                Event::Reset => {
                    let mut chat = entry.chat.lock().unwrap();
                    chat.activity = None;
                    let _ = self.changed.send(Change::Activity(id.clone(), None));
                    chat.context = None;
                    chat.tokens = None;
                    chat.queued.clear();
                    chat.queued_images.clear();
                    chat.todos.clear();
                    self.queued(&chat);
                    let _ = self.changed.send(Change::Todos { chat_id: id.clone(), todos: Vec::new() });
                    let _ = self.changed.send(Change::Meters { chat_id: id.clone(), context: None, tokens: None });
                }
                Event::Settings(settings) => {
                    let mut chat = entry.chat.lock().unwrap();
                    if chat.settings == settings {
                        continue;
                    }
                    chat.settings = settings.clone();
                    self.save(&chat);
                    let _ = self.changed.send(Change::Settings(id.clone(), settings));
                }
                Event::Capabilities(capabilities) => {
                    *entry.capabilities.lock().unwrap() = capabilities.clone();
                    let _ = self.changed.send(Change::Capabilities(id.clone(), capabilities));
                }
            }
        }
        {
            let mut transcript = entry.transcript.lock().unwrap();
            let pending: Vec<_> = transcript.questions.keys().cloned().collect();
            for request in pending { self.resolve(&mut transcript, &id, &request); }
        }
        self.chats.lock().unwrap().retain(|other| !Arc::ptr_eq(other, &entry));
        let _ = fs::remove_file(record_path(&self.data_dir, &id));
        let project_id = entry.chat.lock().unwrap().project_id.clone();
        let _ = self.changed.send(Change::Chats(project_id));
        self.refresh_attention();
        let _ = self.lifecycle.send(Lifecycle::Ended(Box::new(entry.chat.lock().unwrap().clone())));
    }

    fn settle(&self, entry: &Entry, transcript: &Transcript) {
        let status = if transcript.requests.is_empty() { transcript.reported } else { Status::Waiting };
        let mut chat = entry.chat.lock().unwrap();
        if chat.status == status {
            return;
        }
        chat.status = status;
        if status == Status::Done {
            chat.activity = None;
            let _ = self.changed.send(Change::Activity(chat.id.clone(), None));
        }
        let id = chat.id.clone();
        if status == Status::Done {
            chat.viewed_since_done = self.shown(&id);
        }
        self.save(&chat);
        let item = chat.parent.is_none().then(|| chat.attention());
        drop(chat);
        let _ = self.changed.send(Change::Status(id.clone(), status));
        let _ = self.lifecycle.send(Lifecycle::Status(id.clone(), status));
        self.refresh_attention();
        if status != Status::Working && let Some(item) = item {
            self.attention.call(item, self.showing(&id));
        }
    }

    fn resolve(&self, transcript: &mut Transcript, chat_id: &str, request_id: &str) -> bool {
        let pending = transcript.requests.len();
        transcript.requests.retain(|request| request.id() != request_id);
        if transcript.requests.len() == pending {
            return false;
        }
        transcript.questions.remove(request_id);
        transcript.seq += 1;
        let (chat_id, seq, request_id) = (chat_id.to_owned(), transcript.seq, request_id.to_owned());
        let _ = self.changed.send(Change::Resolved { chat_id, seq, request_id });
        true
    }

    fn save(&self, chat: &Chat) {
        if let Err(error) = write_record(&self.data_dir, chat) {
            eprintln!("cannot save chat {}: {error}", chat.id);
        }
    }

    pub fn item(&self, id: &str, index: usize) -> Option<(u64, Item)> {
        let entry = self.get(id)?;
        let transcript = entry.transcript.lock().unwrap();
        Some((transcript.seq, transcript.items.get(index)?.clone()))
    }

    pub fn attach(&self, id: &str) -> Option<(u64, Vec<Item>, Vec<Request>)> {
        let transcript = self.get(id)?;
        let transcript = transcript.transcript.lock().unwrap();
        Some((transcript.seq, transcript.items.clone(), transcript.requests.clone()))
    }

    pub fn cut_off(&self, id: &str) -> Option<String> {
        let entry = self.get(id)?;
        let transcript = entry.transcript.lock().unwrap();
        let chat = entry.chat.lock().unwrap();
        Self::limited_turn(&transcript, &chat)
    }

    fn limited_turn(transcript: &Transcript, chat: &Chat) -> Option<String> {
        if transcript.reported != Status::Done || chat.status != Status::Done || !transcript.requests.is_empty()
            || !matches!(transcript.items.last(), Some(Item::Notice { notice: Notice::Limit, .. }))
        {
            return None;
        }
        let users: Vec<_> = transcript.items.iter().filter(|item| matches!(item, Item::User { .. })).collect();
        Some(Base64::encode_string(&Sha256::digest(serde_json::to_vec(&users).ok()?)))
    }

    pub fn continue_cut_off(&self, id: &str, turn: &str, decide: impl FnOnce() -> bool) -> bool {
        let Some(entry) = self.get(id) else { return false };
        let mut transcript = entry.transcript.lock().unwrap();
        let chat = entry.chat.lock().unwrap();
        if Self::limited_turn(&transcript, &chat).as_deref() != Some(turn) || !decide() {
            return false;
        }
        transcript.recoverable = false;
        transcript.reported = Status::Working;
        entry.driver.send("continue", &[]);
        true
    }

    pub fn retry_interrupted(&self, id: &str, decide: impl FnOnce() -> bool) -> bool {
        let Some(entry) = self.get(id) else { return false };
        let mut transcript = entry.transcript.lock().unwrap();
        let chat = entry.chat.lock().unwrap();
        if !transcript.recoverable || transcript.reported != Status::Done || chat.status != Status::Done || !transcript.requests.is_empty()
            || !matches!(transcript.items.last(), Some(Item::Notice { notice: Notice::Interrupted, .. }))
            || !decide()
        {
            return false;
        }
        transcript.recoverable = false;
        transcript.reported = Status::Working;
        entry.driver.retry();
        true
    }

    pub fn capabilities(&self, id: &str) -> Option<Capabilities> {
        Some(self.get(id)?.capabilities.lock().unwrap().clone())
    }

    pub fn prepare_handoff(&self, id: &str, focus: &str) -> io::Result<(Chat, oneshot::Receiver<io::Result<String>>)> {
        let entry = self.get(id).ok_or_else(|| io::Error::other("The source Chat is no longer open."))?;
        let mut transcript = entry.transcript.lock().unwrap();
        let chat = entry.chat.lock().unwrap();
        if chat.status != Status::Done || transcript.reported != Status::Done || !transcript.requests.is_empty() || !chat.queued.is_empty() || transcript.handoff.is_some() {
            return Err(io::Error::other("Wait for the source Chat to finish before preparing a handoff."));
        }
        if !transcript.items.iter().any(|item| matches!(item, Item::User { .. })) {
            return Err(io::Error::other("Send a message before preparing a handoff."));
        }
        let source = chat.clone();
        drop(chat);
        let mut prompt = templates::HANDOFF.opening(&self.data_dir);
        if !focus.trim().is_empty() { prompt.push_str(&format!("\n\nFocus: {}", focus.trim())); }
        let (reply, result) = oneshot::channel();
        transcript.handoff = Some(Handoff { after: transcript.items.len(), reply });
        transcript.recoverable = false;
        transcript.reported = Status::Working;
        self.settle(&entry, &transcript);
        entry.driver.send(&prompt, &[]);
        Ok((source, result))
    }

    /// Sends `text` now, or with `queue` holds it until the running turn ends.
    pub fn send(&self, id: &str, text: &str, queue: bool, images: &[Image]) {
        if images.iter().map(|image| image.data.len()).sum::<usize>() > 20 * 1024 * 1024 * 4 / 3 + 4 || images.len() > 20 || !images.iter().all(Image::valid) || (text.trim().is_empty() && images.is_empty()) { return; }
        let Some(entry) = self.get(id) else { return };
        let mut transcript = entry.transcript.lock().unwrap();
        let mut chat = entry.chat.lock().unwrap();
        if let Some(handoff) = transcript.handoff.take() {
            let _ = handoff.reply.send(Err(io::Error::other("Handoff cancelled because another message was sent.")));
        }
        if chat.draft.take().is_some() {
            self.save(&chat);
            let _ = self.changed.send(Change::Chats(chat.project_id.clone()));
        }
        if queue && chat.status != Status::Done {
            while chat.queued_images.len() < chat.queued.len() { chat.queued_images.push(Vec::new()); }
            chat.queued.push(text.to_owned());
            chat.queued_images.push(images.to_vec());
            self.queued(&chat);
        } else {
            transcript.reported = Status::Working;
            transcript.recoverable = false;
            drop(chat);
            entry.driver.send(&templates::expand(text, &self.data_dir), images);
        }
    }

    /// Stops the running turn and drops what was queued behind it.
    pub fn interrupt(&self, id: &str) {
        for child in self.children(id) {
            self.close(&child);
        }
        let Some(entry) = self.get(id) else { return };
        let mut transcript = entry.transcript.lock().unwrap();
        if let Some(handoff) = transcript.handoff.take() {
            let _ = handoff.reply.send(Err(io::Error::other("Handoff interrupted.")));
        }
        transcript.recoverable = false;
        let pending: Vec<_> = transcript.questions.keys().cloned().collect();
        for request in pending { self.resolve(&mut transcript, id, &request); }
        let mut chat = entry.chat.lock().unwrap();
        if !chat.queued.is_empty() {
            chat.queued.clear();
            chat.queued_images.clear();
            self.queued(&chat);
        }
        drop(chat);
        entry.driver.interrupt();
    }

    fn queued(&self, chat: &Chat) {
        self.save(chat);
        let _ = self.changed.send(Change::Queued(chat.id.clone(), chat.queued.clone()));
    }

    pub fn answer(&self, id: &str, request_id: &str, answer: Answer) {
        if let Some((child, request)) = request_id.split_once('/')
            && self.get(child).is_some_and(|entry| entry.chat.lock().unwrap().parent.as_deref() == Some(id))
        {
            self.answer(child, request, answer);
            return;
        }
        let Some(entry) = self.get(id) else { return };
        let mut transcript = entry.transcript.lock().unwrap();
        let Some(request) = transcript.requests.iter().find(|request| request.id() == request_id) else { return };
        match (&answer, &request.kind) {
            (Answer::Rules(_), RequestKind::Command { allow_rules: Some(false), .. }) => return,
            (Answer::Rules(rules), RequestKind::Command { words, session_prefixes, .. }) => {
                if rules.len() != words.len() || rules.iter().any(|rule| rule.scope != rules[0].scope) || rules.iter().zip(words).any(|(rule, words)| {
                    rule.prefix.is_empty() || !words.starts_with(&rule.prefix)
                        || (!session_prefixes && rule.scope == Scope::Chat && rule.prefix != *words)
                }) { return; }
            }
            (Answer::Rules(_), _) => return,
            (Answer::Responses(responses), RequestKind::Question { questions, .. })
                if transcript.questions.contains_key(request_id) && questions::valid_responses(questions, responses) => {},
            (Answer::Responses(_), _) => return,
            (_, RequestKind::Question { .. }) if transcript.questions.contains_key(request_id)
                && !matches!(answer, Answer::Deny { .. }) => return,
            (Answer::Answers(answers), RequestKind::Question { questions, .. }) if answers.len() == questions.len() => {},
            (Answer::Answers(_), _) => return,
            _ => {},
        }
        let answered = if transcript.questions.contains_key(request_id) && !matches!(answer, Answer::Responses(_)) { None }
        else if let RequestKind::Question { questions, .. } = &request.kind {
            let answers = match &answer {
                Answer::Answers(answers) => answers.clone(),
                Answer::Deny { instead: Some(text) } => vec![vec![text.clone()]; questions.len()],
                _ => vec![Vec::new(); questions.len()],
            };
            let responses = match &answer { Answer::Responses(responses) => responses.clone(), _ => HashMap::new() };
            Some(AnsweredQuestions { source: None, after: transcript.items.len(), questions: questions.clone(), answers, responses })
        } else { None };
        let pending = transcript.questions.remove(request_id);
        if self.resolve(&mut transcript, id, request_id) {
            if let Some(answered) = answered {
                let mut chat = entry.chat.lock().unwrap();
                chat.answered_questions.push(answered);
                self.save(&chat);
                let _ = self.changed.send(Change::Chats(chat.project_id.clone()));
            }
            if let Some(pending) = pending {
                let reply = match answer {
                    Answer::Responses(answers) => questions::Reply { answers, cancelled: false, instruction: None },
                    Answer::Deny { instead } => questions::Reply { answers: HashMap::new(), cancelled: true, instruction: instead },
                    _ => unreachable!(),
                };
                let _ = pending.send(reply);
            } else { entry.driver.answer(request_id, answer); }
            self.settle(&entry, &transcript);
        }
    }

    pub fn run(&self, id: &str, builtin: Builtin) {
        if let Some(entry) = self.get(id) {
            if matches!(builtin, Builtin::Rewind(_) | Builtin::Clear | Builtin::Compact)
                && entry.chat.lock().unwrap().status != Status::Done { return; }
            entry.driver.run(builtin);
        }
    }

    pub fn close(&self, id: &str) {
        for child in self.children(id) {
            self.close(&child);
        }
        if let Some(entry) = self.get(id) {
            entry.driver.close();
        }
    }

    /// Closes a Chat, or forgets the record of one that did not come back when the server restarted.
    /// Either way `Lifecycle::Ended` follows with its last state; false when there is no such Chat.
    pub fn end(&self, id: &str) -> bool {
        if self.get(id).is_some() {
            self.close(id);
            return true;
        }
        let path = record_path(&self.data_dir, id);
        let Ok(chat) = read_record(&path) else { return false };
        let _ = fs::remove_file(path);
        let _ = self.lifecycle.send(Lifecycle::Ended(Box::new(chat)));
        true
    }

    pub fn view(&self, connection: Uuid, browser: Option<String>, chat_ids: Vec<String>) {
        let entries: Vec<_> = chat_ids.iter().filter_map(|id| self.get(id)).collect();
        if chat_ids.is_empty() {
            self.views.lock().unwrap().remove(&connection);
        } else {
            self.views.lock().unwrap().insert(connection, (browser, chat_ids));
        }
        let mut viewed = false;
        for entry in entries {
            let mut chat = entry.chat.lock().unwrap();
            if chat.status == Status::Done && !chat.viewed_since_done {
                chat.viewed_since_done = true;
                self.save(&chat);
                viewed = true;
            }
        }
        if viewed {
            self.refresh_attention();
        }
    }

    fn shown(&self, id: &str) -> bool {
        self.views.lock().unwrap().values().any(|(_, shown)| shown.iter().any(|shown| shown == id))
    }

    fn showing(&self, id: &str) -> HashSet<String> {
        let views = self.views.lock().unwrap();
        let showing = views.values().filter(|(_, shown)| shown.iter().any(|shown| shown == id));
        showing.filter_map(|(browser, _)| browser.clone()).collect()
    }

    fn refresh_attention(&self) {
        let items = self.list().iter().filter(|chat| chat.needs_attention()).map(Chat::attention).collect();
        self.attention.publish(Source::Chats, items);
    }

    fn get(&self, id: &str) -> Option<Arc<Entry>> {
        let chats = self.chats.lock().unwrap();
        chats.iter().find(|entry| entry.chat.lock().unwrap().id == id).cloned()
    }
}

/// The Agent's Usage windows, read without a model call.
pub async fn usage(rig: &Rig) -> io::Result<Snapshot> {
    match rig.agent {
        Agent::ClaudeCode => Ok(Snapshot { readings: claude::usage(&rig.env).await?, ..Snapshot::default() }),
        Agent::Codex => codex::usage(&rig.env).await,
        Agent::Copilot => Ok(Snapshot { readings: copilot::usage(&rig.env).await?, ..Snapshot::default() }),
    }
}

impl Chat {
    fn needs_attention(&self) -> bool {
        match self.status {
            Status::Waiting => true,
            Status::Done => !self.viewed_since_done,
            Status::Working => false,
        }
    }

    fn attention(&self) -> Attention {
        Attention {
            project_id: self.project_id.clone(),
            tab: Tab::Chat { chat_id: self.id.clone() },
            label: self.rig.clone(),
            status: self.status,
        }
    }
}

fn record_path(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join("chats").join(format!("{id}.json"))
}

fn read_record(path: &Path) -> io::Result<Chat> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn write_record(data_dir: &Path, chat: &Chat) -> io::Result<()> {
    let path = record_path(data_dir, &chat.id);
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec(chat)?)?;
    fs::rename(tmp, path)
}

fn words(command: &str) -> Vec<Vec<String>> {
    let mut parts = vec![Vec::new()];
    let mut word: Option<String> = None;
    let mut chars = command.chars().peekable();
    while let Some(char) = chars.next() {
        match char {
            '\'' => {
                let word = word.get_or_insert_default();
                word.extend(chars.by_ref().take_while(|&char| char != '\''));
            }
            '"' => {
                let word = word.get_or_insert_default();
                while let Some(char) = chars.next() {
                    match char {
                        '"' => break,
                        '\\' if matches!(chars.peek(), Some('"' | '\\' | '$' | '`')) => word.extend(chars.next()),
                        char => word.push(char),
                    }
                }
            }
            '\\' => match chars.next() {
                Some('\n') | None => {}
                Some(char) => word.get_or_insert_default().push(char),
            },
            // `2>&1` and `&>` redirect rather than separate.
            '&' if word.as_ref().is_some_and(|word| word.ends_with(['>', '<'])) || chars.peek() == Some(&'>') => {
                word.get_or_insert_default().push(char);
            }
            ';' | '&' | '|' | '\n' => {
                parts.last_mut().unwrap().extend(word.take());
                while chars.next_if(|char| matches!(char, '&' | '|')).is_some() {}
                if !parts.last().unwrap().is_empty() {
                    parts.push(Vec::new());
                }
            }
            char if char.is_whitespace() => parts.last_mut().unwrap().extend(word.take()),
            char => word.get_or_insert_default().push(char),
        }
    }
    parts.last_mut().unwrap().extend(word);
    parts.retain(|part| !part.is_empty());
    parts
}

fn diff(old: &str, new: &str) -> String {
    let removed = old.lines().map(|line| format!("-{line}\n"));
    let added = new.lines().map(|line| format!("+{line}\n"));
    removed.chain(added).collect()
}

fn epoch_ms(time: &str) -> Option<u64> {
    let time: toml::value::Datetime = time.parse().ok()?;
    let (date, clock) = (time.date?, time.time?);
    let offset = match time.offset? {
        toml::value::Offset::Z => 0,
        toml::value::Offset::Custom { minutes } => i64::from(minutes),
    };
    let (month, day) = (i64::from(date.month), i64::from(date.day));
    let year = i64::from(date.year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let days = era * 146_097 + of_era * 365 + of_era / 4 - of_era / 100 + of_year - 719_468;
    let minutes = (days * 24 + i64::from(clock.hour)) * 60 + i64::from(clock.minute) - offset;
    let seconds = minutes * 60 + i64::from(clock.second.unwrap_or(0));
    u64::try_from(seconds * 1000 + i64::from(clock.nanosecond.unwrap_or(0) / 1_000_000)).ok()
}
