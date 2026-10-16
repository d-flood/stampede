mod claude;
mod codex;
mod holder;
mod protocol;

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc::UnboundedReceiver};
use uuid::Uuid;

use crate::{
    attention::{Attention, Feed, Source, Tab},
    config::{Agent, AgentSettings, Rig},
    hooks::Status,
    projects::Project,
    usage::{Reading, Usage},
};
pub use holder::{HoldChatArgs, hold_chat};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Item {
    User { text: String, images: usize },
    Text { text: String, streaming: bool },
    Thinking {
        seconds: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    Tools { calls: Vec<ToolCall> },
    Edit { path: String, diff: String, added: usize, removed: usize },
    Command { command: String, status: CommandStatus, output: String },
    Notice { notice: Notice, text: String },
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
    Other,
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
}

impl Item {
    fn compacted() -> Self {
        Item::Notice { notice: Notice::Compacted, text: "Conversation compacted".to_owned() }
    }

    fn interrupted() -> Self {
        Item::Notice { notice: Notice::Interrupted, text: "Interrupted".to_owned() }
    }

    fn edit(path: String, diff: String) -> Self {
        let count = |sign: char| diff.lines().skip_while(|line| !line.starts_with("@@")).filter(|line| line.starts_with(sign)).count();
        Item::Edit { path, added: count('+'), removed: count('-'), diff }
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
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Request {
    Command { id: String, words: Vec<Vec<String>> },
    Edit { id: String, path: String, diff: String },
    Other { id: String, title: String, detail: String },
}

impl Request {
    pub fn id(&self) -> &str {
        match self {
            Request::Command { id, .. } | Request::Edit { id, .. } | Request::Other { id, .. } => id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(from = "Decision")]
pub enum Answer {
    Allow,
    Deny { instead: Option<String> },
}

#[derive(Deserialize)]
struct Decision {
    allow: bool,
    #[serde(default)]
    instead: Option<String>,
}

impl From<Decision> for Answer {
    fn from(Decision { allow, instead }: Decision) -> Self {
        if allow { Answer::Allow } else { Answer::Deny { instead } }
    }
}

pub enum Event {
    Session(String),
    Status(Status),
    /// Appends the Item when its index is the transcript's length, and replaces the one there otherwise.
    Item(usize, Item),
    /// Replaces whichever of the Chat's context fill and token counts it carries.
    Meters(Option<Context>, Option<Tokens>),
    Usage(Vec<Reading>),
    Truncate(usize),
    Request(Request),
    Resolved(String),
}

/// One Agent's wire protocol. A driver's events end when its Agent has stopped.
pub trait Driver: Send + Sync {
    /// Starts a turn, or steers the running one.
    fn send(&self, text: &str);
    fn interrupt(&self);
    fn answer(&self, request_id: &str, answer: Answer);
    fn close(&self);
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
    pub status: Status,
    pub viewed_since_done: bool,
    #[serde(default)]
    pub context: Option<Context>,
    #[serde(default)]
    pub tokens: Option<Tokens>,
    #[serde(default)]
    pub settings: AgentSettings,
    #[serde(default)]
    pub queued: Vec<String>,
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

/// Both Agents' past sessions in `cwd`, newest first, found through each Agent's Rig.
pub async fn sessions(cwd: &Path, claude: &Rig, codex: &Rig) -> Vec<Session> {
    let mut sessions = Vec::new();
    for (agent, listed) in [
        (Agent::ClaudeCode, claude::sessions(cwd, &claude.env)),
        (Agent::Codex, codex::sessions(cwd, &codex.env).await),
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
    Queued(String, Vec<String>),
    Truncated(String),
    Item { chat_id: String, seq: u64, index: usize, item: Item },
    Meters { chat_id: String, context: Option<Context>, tokens: Option<Tokens> },
    Request { chat_id: String, seq: u64, request: Request },
    Resolved { chat_id: String, seq: u64, request_id: String },
}

pub struct Chats {
    data_dir: PathBuf,
    chats: Mutex<Vec<Arc<Entry>>>,
    changed: broadcast::Sender<Change>,
    views: Mutex<HashMap<Uuid, (Option<String>, Vec<String>)>>,
    attention: Arc<Feed>,
    usage: Arc<Usage>,
}

struct Entry {
    chat: Mutex<Chat>,
    transcript: Mutex<Transcript>,
    driver: Box<dyn Driver>,
}

struct Transcript {
    seq: u64,
    items: Vec<Item>,
    requests: Vec<Request>,
    reported: Status,
}

impl Chats {
    pub fn new(data_dir: PathBuf, attention: Arc<Feed>, usage: Arc<Usage>) -> Self {
        Self {
            data_dir,
            chats: Mutex::default(),
            changed: broadcast::Sender::new(1024),
            views: Mutex::default(),
            attention,
            usage,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Change> {
        self.changed.subscribe()
    }

    pub fn list(&self) -> Vec<Chat> {
        self.chats.lock().unwrap().iter().map(|entry| entry.chat.lock().unwrap().clone()).collect()
    }

    pub fn owned(&self, project_id: &str) -> Vec<Chat> {
        self.list().into_iter().filter(|chat| chat.project_id == project_id).collect()
    }

    pub async fn create(self: &Arc<Self>, project: &Project, rig: &Rig, session: Option<&str>) -> io::Result<Chat> {
        let chat = Chat {
            id: format!("{:016x}", Uuid::new_v4().as_u64_pair().1),
            project_id: project.id.clone(),
            rig: rig.name.clone(),
            agent: rig.agent,
            cwd: PathBuf::from(&project.path),
            created: SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos(),
            agent_session: session.map(str::to_owned),
            status: Status::Done,
            viewed_since_done: true,
            context: None,
            tokens: None,
            settings: rig.settings(),
            queued: Vec::new(),
        };
        fs::create_dir_all(self.data_dir.join("chats"))?;
        write_record(&self.data_dir, &chat)?;
        let started = match rig.agent {
            Agent::ClaudeCode => claude::start(&self.data_dir, &chat.id, &chat.cwd, &rig.env, &chat.settings, session)
                .await
                .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
            Agent::Codex if session.is_some() => codex::resume(&chat, &rig.env)
                .await
                .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
            Agent::Codex => codex::start(&chat.cwd, &rig.env, &chat.settings)
                .await
                .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
        };
        let (driver, events) = started.inspect_err(|_| {
            let _ = fs::remove_file(record_path(&self.data_dir, &chat.id));
        })?;
        self.adopt(chat.clone(), driver, events);
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
                Agent::Codex => codex::resume(&chat, &env)
                    .await
                    .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
            };
            match reattached {
                Ok((driver, events)) => self.adopt(chat, driver, events),
                Err(error) => eprintln!("cannot restore chat {}: {error}", chat.id),
            }
        }
        self.refresh_attention();
        Ok(())
    }

    fn adopt(self: &Arc<Self>, chat: Chat, driver: Box<dyn Driver>, events: UnboundedReceiver<Event>) {
        let transcript = Transcript { seq: 0, items: Vec::new(), requests: Vec::new(), reported: chat.status };
        let entry = Arc::new(Entry { chat: Mutex::new(chat), transcript: Mutex::new(transcript), driver });
        self.chats.lock().unwrap().push(Arc::clone(&entry));
        tokio::spawn(Arc::clone(self).follow(entry, events));
    }

    async fn follow(self: Arc<Self>, entry: Arc<Entry>, mut events: UnboundedReceiver<Event>) {
        let (id, project_id) = {
            let chat = entry.chat.lock().unwrap();
            (chat.id.clone(), chat.project_id.clone())
        };
        while let Some(event) = events.recv().await {
            match event {
                Event::Session(session) => {
                    let mut chat = entry.chat.lock().unwrap();
                    chat.agent_session = Some(session);
                    self.save(&chat);
                }
                Event::Status(status) => {
                    let interrupted = matches!(
                        entry.transcript.lock().unwrap().items.last(),
                        Some(Item::Notice { notice: Notice::Interrupted, .. })
                    );
                    let mut chat = entry.chat.lock().unwrap();
                    if status == Status::Done && !chat.queued.is_empty() && !interrupted {
                        let text = chat.queued.remove(0);
                        self.queued(&chat);
                        entry.driver.send(&text);
                        continue;
                    }
                    drop(chat);
                    let mut transcript = entry.transcript.lock().unwrap();
                    transcript.reported = status;
                    if status == Status::Done {
                        let pending: Vec<_> =
                            transcript.requests.iter().map(|request| request.id().to_owned()).collect();
                        for request_id in pending {
                            self.resolve(&mut transcript, &id, &request_id);
                        }
                    }
                    self.settle(&entry, &transcript);
                }
                Event::Request(request) => {
                    let mut transcript = entry.transcript.lock().unwrap();
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
                Event::Item(index, item) => {
                    let mut transcript = entry.transcript.lock().unwrap();
                    match transcript.items.get_mut(index) {
                        Some(existing) => *existing = item.clone(),
                        None => transcript.items.push(item.clone()),
                    }
                    transcript.seq += 1;
                    let seq = transcript.seq;
                    let _ = self.changed.send(Change::Item { chat_id: id.clone(), seq, index, item });
                }
                Event::Usage(readings) => self.usage.record(readings),
                Event::Truncate(len) => {
                    let mut transcript = entry.transcript.lock().unwrap();
                    transcript.items.truncate(len);
                    transcript.requests.clear();
                    transcript.seq += 1;
                    let _ = self.changed.send(Change::Truncated(id.clone()));
                }
            }
        }
        self.chats.lock().unwrap().retain(|other| !Arc::ptr_eq(other, &entry));
        let _ = fs::remove_file(record_path(&self.data_dir, &id));
        let _ = self.changed.send(Change::Chats(project_id));
        self.refresh_attention();
    }

    fn settle(&self, entry: &Entry, transcript: &Transcript) {
        let status = if transcript.requests.is_empty() { transcript.reported } else { Status::Waiting };
        let mut chat = entry.chat.lock().unwrap();
        if chat.status == status {
            return;
        }
        chat.status = status;
        let id = chat.id.clone();
        if status == Status::Done {
            chat.viewed_since_done = self.shown(&id);
        }
        self.save(&chat);
        let item = chat.attention();
        drop(chat);
        let _ = self.changed.send(Change::Status(id.clone(), status));
        self.refresh_attention();
        if status != Status::Working {
            self.attention.call(item, self.showing(&id));
        }
    }

    fn resolve(&self, transcript: &mut Transcript, chat_id: &str, request_id: &str) -> bool {
        let pending = transcript.requests.len();
        transcript.requests.retain(|request| request.id() != request_id);
        if transcript.requests.len() == pending {
            return false;
        }
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

    pub fn attach(&self, id: &str) -> Option<(u64, Vec<Item>, Vec<Request>)> {
        let transcript = self.get(id)?;
        let transcript = transcript.transcript.lock().unwrap();
        Some((transcript.seq, transcript.items.clone(), transcript.requests.clone()))
    }

    /// Sends `text` now, or with `queue` holds it until the running turn ends.
    pub fn send(&self, id: &str, text: &str, queue: bool) {
        let Some(entry) = self.get(id) else { return };
        let mut chat = entry.chat.lock().unwrap();
        if queue && chat.status != Status::Done {
            chat.queued.push(text.to_owned());
            self.queued(&chat);
        } else {
            drop(chat);
            entry.driver.send(text);
        }
    }

    /// Stops the running turn and drops what was queued behind it.
    pub fn interrupt(&self, id: &str) {
        let Some(entry) = self.get(id) else { return };
        let mut chat = entry.chat.lock().unwrap();
        if !chat.queued.is_empty() {
            chat.queued.clear();
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
        let Some(entry) = self.get(id) else { return };
        let mut transcript = entry.transcript.lock().unwrap();
        if self.resolve(&mut transcript, id, request_id) {
            entry.driver.answer(request_id, answer);
            self.settle(&entry, &transcript);
        }
    }

    pub fn close(&self, id: &str) {
        if let Some(entry) = self.get(id) {
            entry.driver.close();
        }
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
pub async fn usage(rig: &Rig) -> io::Result<Vec<Reading>> {
    match rig.agent {
        Agent::ClaudeCode => claude::usage(&rig.env).await,
        Agent::Codex => codex::usage(&rig.env).await,
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
