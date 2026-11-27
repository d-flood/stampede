use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs, io,
    io::{BufRead, BufReader},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::UNIX_EPOCH,
};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot, watch};

use super::{Chat, Chats, Lifecycle, Status, antigravity, claude, codex, copilot};
use crate::{config::{Agent, Config, Rig}, epics::Epics, projects::{Project, Projects}};

const VERSION: i32 = 2;
const SNAPSHOT_VERSION: i32 = 1;
const ARCHIVE_VERSION: i32 = 2;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct SessionKey {
    pub agent: Agent,
    pub store: String,
    pub session_id: String,
}

#[derive(Clone, Serialize)]
pub struct SessionProject {
    id: String,
    path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub key: SessionKey,
    project: SessionProject,
    title: String,
    updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ended: Option<String>,
}

#[derive(Serialize)]
pub struct Found {
    #[serde(flatten)]
    session: Session,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<usize>,
    matches: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    excerpt: Option<Excerpt>,
}

#[derive(Serialize)]
struct Excerpt {
    who: String,
    text: String,
    ranges: Vec<(usize, usize)>,
}

#[derive(Default, Serialize)]
pub struct Bytes {
    index: u64,
    archive: u64,
}

#[derive(Default, Serialize)]
pub struct Coverage {
    indexed: usize,
    total: usize,
    stale: Vec<Agent>,
    ended: usize,
    bytes: Bytes,
}

#[derive(Clone)]
struct KnownProject {
    id: String,
    path: String,
    parent: Option<String>,
    common_dir: Option<String>,
}

#[derive(Clone)]
struct Row {
    session_id: String,
    directory: String,
    project: String,
    title: String,
    updated_at: i64,
    revision: String,
}

type Store = (Agent, String);
type Job = Box<dyn FnOnce(&mut Connection, &mut Connection) + Send>;

struct Flight {
    full: bool,
    projects: crate::projects::List,
    pending_full: Option<Rig>,
    pending: BTreeMap<String, (Rig, Chat)>,
}

pub struct Sessions {
    dir: PathBuf,
    writer: mpsc::Sender<Job>,
    changed: watch::Sender<u64>,
    running: Mutex<BTreeMap<Store, Flight>>,
    stale: Mutex<BTreeSet<Store>>,
    resuming: Mutex<HashSet<SessionKey>>,
    projects: Arc<Projects>,
    epics: Arc<Epics>,
    config: watch::Receiver<Arc<Config>>,
    cloud: bool,
}

fn connection(path: &Path) -> io::Result<Connection> {
    configure(Connection::open(path).map_err(io::Error::other)?)
}

fn read_connection(path: &Path) -> io::Result<Connection> {
    configure(Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(io::Error::other)?)
}

fn configure(db: Connection) -> io::Result<Connection> {
    db.pragma_update(None, "foreign_keys", "ON").map_err(io::Error::other)?;
    db.busy_timeout(std::time::Duration::from_secs(5)).map_err(io::Error::other)?;
    Ok(db)
}

fn version(db: &Connection) -> io::Result<i32> {
    db.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(io::Error::other)
}

fn session_schema(archive: bool) -> String {
    let ended = if archive { "ended TEXT NOT NULL CHECK (ended IN ('aged', 'cloud', 'missing')), ended_at INTEGER NOT NULL," } else { "" };
    format!("CREATE TABLE sessions (
                id INTEGER PRIMARY KEY, agent TEXT NOT NULL, store TEXT NOT NULL, session_id TEXT NOT NULL,
                directory TEXT NOT NULL, project TEXT NOT NULL, title TEXT NOT NULL, updated_at INTEGER NOT NULL,
                revision TEXT NOT NULL, snapshot_revision TEXT, {ended} UNIQUE (agent, store, session_id)
            );
            CREATE INDEX sessions_project ON sessions(project, updated_at);
            CREATE VIRTUAL TABLE titles_fts USING fts5(title, content='sessions', content_rowid='id', tokenize='unicode61 remove_diacritics 2');
            CREATE TRIGGER titles_insert AFTER INSERT ON sessions BEGIN
                INSERT INTO titles_fts(rowid, title) VALUES (new.id, new.title);
            END;
            CREATE TRIGGER titles_delete AFTER DELETE ON sessions BEGIN
                INSERT INTO titles_fts(titles_fts, rowid, title) VALUES ('delete', old.id, old.title);
            END;
            CREATE TRIGGER titles_update AFTER UPDATE OF title ON sessions BEGIN
                INSERT INTO titles_fts(titles_fts, rowid, title) VALUES ('delete', old.id, old.title);
                INSERT INTO titles_fts(rowid, title) VALUES (new.id, new.title);
            END;")
}

const SNAPSHOT_SCHEMA: &str = "
    CREATE TABLE snapshots (
        session INTEGER PRIMARY KEY REFERENCES sessions ON DELETE CASCADE,
        version INTEGER NOT NULL, items BLOB NOT NULL
    );
    CREATE TABLE messages (
        id INTEGER PRIMARY KEY, session INTEGER NOT NULL REFERENCES sessions ON DELETE CASCADE,
        item INTEGER NOT NULL, role TEXT NOT NULL CHECK (role IN ('user', 'agent')), text TEXT NOT NULL
    );
    CREATE INDEX messages_session ON messages(session);
    CREATE VIRTUAL TABLE messages_fts USING fts5(text, content='messages', content_rowid='id', tokenize='unicode61 remove_diacritics 2');
    CREATE TRIGGER messages_insert AFTER INSERT ON messages BEGIN
        INSERT INTO messages_fts(rowid, text) VALUES (new.id, new.text);
    END;
    CREATE TRIGGER messages_delete AFTER DELETE ON messages BEGIN
        INSERT INTO messages_fts(messages_fts, rowid, text) VALUES ('delete', old.id, old.text);
    END;
    CREATE TRIGGER messages_update AFTER UPDATE OF text ON messages BEGIN
        INSERT INTO messages_fts(messages_fts, rowid, text) VALUES ('delete', old.id, old.text);
        INSERT INTO messages_fts(rowid, text) VALUES (new.id, new.text);
    END;
";

fn initialize(dir: &Path) -> io::Result<(Connection, Connection)> {
    fs::create_dir_all(dir)?;
    let archive = connection(&dir.join("archive.sqlite"))?;
    archive.pragma_update(None, "journal_mode", "WAL").map_err(io::Error::other)?;
    let archive_version = version(&archive)?;
    if archive_version == 0 {
        archive.execute_batch("BEGIN; CREATE TABLE IF NOT EXISTS projects (id TEXT PRIMARY KEY, path TEXT NOT NULL, parent TEXT, common_dir TEXT); PRAGMA user_version = 1; COMMIT;").map_err(io::Error::other)?;
    }
    if archive_version < ARCHIVE_VERSION {
        archive.execute_batch(&format!("BEGIN; {}; {}; PRAGMA user_version = {ARCHIVE_VERSION}; COMMIT;", session_schema(true), SNAPSHOT_SCHEMA)).map_err(io::Error::other)?;
    }
    let path = dir.join("index.sqlite");
    let index = if path.exists() { Some(connection(&path)?) } else { None };
    let index = if index.as_ref().map(version).transpose()? != Some(VERSION) {
        drop(index);
        for name in ["index.sqlite", "index.sqlite-wal", "index.sqlite-shm"] {
            match fs::remove_file(dir.join(name)) {
                Ok(()) => (),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
        }
        let db = connection(&path)?;
        db.execute_batch(&format!("BEGIN; {}; {}; PRAGMA user_version = {VERSION}; COMMIT;", session_schema(false), SNAPSHOT_SCHEMA)).map_err(io::Error::other)?;
        db
    } else { index.unwrap() };
    index.pragma_update(None, "journal_mode", "WAL").map_err(io::Error::other)?;
    Ok((index, archive))
}

fn store_variable(agent: Agent) -> Option<&'static str> {
    match agent {
        Agent::ClaudeCode => Some("CLAUDE_CONFIG_DIR"),
        Agent::Codex => Some("CODEX_HOME"),
        Agent::Copilot => Some("COPILOT_HOME"),
        Agent::Antigravity => None,
    }
}

pub(super) fn store(agent: Agent, env: &BTreeMap<String, String>) -> String {
    let path = if agent == Agent::ClaudeCode {
        claude::config_dir(env).unwrap_or_else(|| crate::home().join(".claude"))
    } else {
        store_variable(agent).and_then(|variable| env.get(variable).map(PathBuf::from)
            .or_else(|| std::env::var_os(variable).map(PathBuf::from)))
            .unwrap_or_else(|| {
                let home = env.get("HOME").map(PathBuf::from).unwrap_or_else(crate::home);
                home.join(match agent {
                    Agent::Codex => ".codex",
                    Agent::Copilot => ".copilot",
                    Agent::Antigravity => ".gemini/antigravity-cli",
                    Agent::ClaudeCode => unreachable!(),
                })
            })
    };
    let path = if path.is_absolute() { path } else { std::env::current_dir().unwrap_or_default().join(path) };
    canonical(&path).to_string_lossy().into_owned()
}

fn canonical(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        if let Ok(resolved) = fs::canonicalize(ancestor) {
            let rest = path.strip_prefix(ancestor).unwrap();
            return if rest.as_os_str().is_empty() { resolved } else { resolved.join(rest) };
        }
    }
    path.to_owned()
}

pub(super) fn apply_store(agent: Agent, store: &str, env: &mut BTreeMap<String, String>) {
    if store.is_empty() { return; }
    if let Some(variable) = store_variable(agent) {
        env.insert(variable.to_owned(), store.to_owned());
    } else if let Some(home) = Path::new(store).parent().and_then(Path::parent).filter(|home| home.join(".gemini/antigravity-cli") == Path::new(store)) {
        env.insert("HOME".to_owned(), home.to_string_lossy().into_owned());
    }
}

impl Sessions {
    pub async fn new(data: &Path, projects: Arc<Projects>, epics: Arc<Epics>, config: watch::Receiver<Arc<Config>>, cloud: bool) -> io::Result<Arc<Self>> {
        let dir = data.join("sessions");
        let (writer, jobs) = mpsc::channel::<Job>();
        let (ready, opened) = oneshot::channel();
        let path = dir.clone();
        std::thread::Builder::new().name("sessions-writer".to_owned()).spawn(move || {
            match initialize(&path) {
                Ok((mut index, mut archive)) => {
                    let _ = ready.send(Ok(()));
                    while let Ok(job) = jobs.recv() { job(&mut index, &mut archive); }
                }
                Err(error) => { let _ = ready.send(Err(error)); }
            }
        })?;
        opened.await.map_err(io::Error::other)??;
        let service = Arc::new(Self {
            dir, writer, changed: watch::Sender::new(0), running: Mutex::default(), stale: Mutex::default(), resuming: Mutex::default(),
            projects, epics, config, cloud,
        });
        let writer = service.writer.clone();
        service.projects.remember(move |project| {
            let _ = writer.send(Box::new(move |_, archive| {
                if let Err(error) = remember(archive, &project) { eprintln!("cannot remember Project: {error}"); }
            }));
        });
        Ok(service)
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> { self.changed.subscribe() }

    fn notify(&self) { self.changed.send_modify(|version| *version += 1); }

    async fn write<T: Send + 'static>(&self, job: impl FnOnce(&mut Connection, &mut Connection) -> io::Result<T> + Send + 'static) -> io::Result<T> {
        let (reply, result) = oneshot::channel();
        self.writer.send(Box::new(move |index, archive| { let _ = reply.send(job(index, archive)); })).map_err(|_| io::Error::other("Session writer stopped"))?;
        result.await.map_err(io::Error::other)?
    }

    pub fn follow(self: &Arc<Self>, chats: Arc<Chats>) {
        let service = self.clone();
        let mut lifecycle = chats.lifecycle();
        let mut changes = chats.subscribe();
        let mut projects = self.projects.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = lifecycle.recv() => {
                        let chat = match event {
                            Ok(Lifecycle::Session(id, _)) | Ok(Lifecycle::Status(id, Status::Done)) => chats.session_chat(&id),
                            Ok(Lifecycle::Ended(chat)) => Some(*chat),
                            Ok(_) => None,
                            Err(broadcast::error::RecvError::Lagged(_)) => { service.refresh(); service.notify(); None },
                            Err(broadcast::error::RecvError::Closed) => break,
                        };
                        if let Some(chat) = chat {
                            service.notify();
                            service.refresh_chat(chat);
                        }
                    }
                    Ok(()) = projects.changed() => { service.notify(); }
                    event = changes.recv() => {
                        match event {
                            Ok(super::Change::Chats(_)) | Err(broadcast::error::RecvError::Lagged(_)) => service.notify(),
                            Err(broadcast::error::RecvError::Closed) => break,
                            _ => (),
                        }
                    }
                }
            }
        });
        self.refresh_all(true);
    }

    pub fn refresh(self: &Arc<Self>) {
        self.refresh_all(false);
    }

    fn refresh_all(self: &Arc<Self>, startup: bool) {
        let service = self.clone();
        let config = self.config.borrow().clone();
        tokio::spawn(async move {
            let cloud = service.cloud;
            let Ok(rigs) = tokio::task::spawn_blocking(move || {
                let mut rigs = BTreeMap::new();
                for agent in Agent::ALL {
                    if cloud && !matches!(agent, Agent::ClaudeCode | Agent::Codex) { continue; }
                    if agent == Agent::ClaudeCode {
                        for rig in config.rigs.iter().filter(|rig| rig.agent == agent).cloned().chain([Rig::starter(agent)]) {
                            rigs.entry((agent, store(agent, &rig.env))).or_insert(rig);
                        }
                    } else {
                        let rig = config.rig_for(agent);
                        rigs.insert((agent, store(agent, &rig.env)), rig);
                    }
                }
                rigs
            }).await else { return };
            if startup {
                for agent in Agent::ALL {
                    let service = service.clone();
                    let stores: Vec<_> = rigs.iter().filter(|(key, _)| key.0 == agent).map(|(key, rig)| (key.clone(), rig.clone())).collect();
                    tokio::spawn(async move {
                        for (key, rig) in stores {
                            if let Some(scan) = service.queue(key, rig, None) { let _ = scan.await; }
                        }
                    });
                }
            } else {
                for (key, rig) in rigs { let _ = service.queue(key, rig, None); }
            }
        });
    }

    fn refresh_chat(self: &Arc<Self>, chat: Chat) {
        if chat.agent_session.is_none() || self.cloud && !matches!(chat.agent, Agent::ClaudeCode | Agent::Codex) { return; }
        let mut rig = self.config.borrow().rigs.iter().find(|rig| rig.name == chat.rig && rig.agent == chat.agent).cloned()
            .unwrap_or_else(|| Rig::starter(chat.agent));
        apply_store(chat.agent, &chat.session_store, &mut rig.env);
        let _ = self.queue((chat.agent, chat.session_store.clone()), rig, Some(chat));
    }

    fn queue(self: &Arc<Self>, key: Store, rig: Rig, chat: Option<Chat>) -> Option<tokio::task::JoinHandle<()>> {
        {
            let mut running = self.running.lock().unwrap();
            if let Some(flight) = running.get_mut(&key) {
                if let Some(chat) = chat {
                    if let Some(id) = chat.agent_session.clone() { flight.pending.insert(id, (rig, chat)); }
                } else if !flight.full || key.0 == Agent::ClaudeCode || flight.projects != self.projects.list() {
                    flight.pending_full = Some(rig);
                }
                return None;
            }
            running.insert(key.clone(), Flight { full: chat.is_none(), projects: self.projects.list(), pending_full: None, pending: BTreeMap::new() });
        }
        let service = self.clone();
        Some(tokio::spawn(async move {
            let mut work = (rig, chat);
            loop {
                let full = work.1.is_none();
                let result = service.scan(key.clone(), work.0, work.1).await;
                let marked = {
                    let mut stale = service.stale.lock().unwrap();
                    match result {
                        Ok(_) if full => stale.remove(&key),
                        Ok(_) => false,
                        Err(ref error) => {
                            eprintln!("cannot list {} Sessions: {error}", key.0.name());
                            stale.insert(key.clone())
                        }
                    }
                };
                let next = {
                    let mut running = service.running.lock().unwrap();
                    let flight = running.get_mut(&key).unwrap();
                    if let Some(rig) = flight.pending_full.take() {
                        flight.full = true;
                        flight.projects = service.projects.list();
                        Some((rig, None))
                    } else if let Some((_, (rig, chat))) = flight.pending.pop_first() {
                        flight.full = false;
                        Some((rig, Some(chat)))
                    } else {
                        running.remove(&key);
                        None
                    }
                };
                if marked || matches!(result, Ok(true)) { service.notify(); }
                let Some(next) = next else { break };
                work = next;
            }
        }))
    }

    async fn scan(&self, key: Store, rig: Rig, chat: Option<Chat>) -> io::Result<bool> {
        let projects = self.projects.list();
        let epics = self.epics.clone();
        let dir = self.dir.clone();
        let scope = key.clone();
        let target = chat.as_ref().and_then(|chat| chat.agent_session.clone());
        let filter = target.clone();
        // Wait behind Project-open records before taking the catalogue's read snapshot.
        self.write(|_, _| Ok(())).await?;
        let (rows, directories) = tokio::task::spawn_blocking(move || {
            let known = known_projects(&read_connection(&dir.join("archive.sqlite"))?)?;
            let data = dir.parent().unwrap();
            if scope.0 == Agent::ClaudeCode {
                let previous = current(&read_connection(&dir.join("index.sqlite"))?, &scope)?;
                Ok((claude_rows(&scope.1, &known, data, &epics, &previous, filter.as_deref())?, None))
            } else {
                if store(scope.0, &rig.env) != scope.1 {
                    return Err(io::Error::other("The Rig no longer points at this history store"));
                }
                let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
                let mut rows = Vec::new();
                let mut directories = HashSet::new();
                let paths: Vec<_> = if let Some(chat) = &chat {
                    vec![chat.cwd.to_string_lossy().into_owned()]
                } else {
                    projects.iter().map(|project| project.path.clone()).collect()
                };
                for path in paths {
                    let cwd = Path::new(&path);
                    let listed = runtime.block_on(async {
                        match scope.0 {
                            Agent::Codex => codex::sessions(cwd, &rig.env).await,
                            Agent::Copilot => copilot::sessions(cwd, &rig.env).await,
                            Agent::Antigravity => antigravity::sessions(cwd, &rig.env).await,
                            Agent::ClaudeCode => unreachable!(),
                        }
                    })?;
                    directories.insert(path.clone());
                    let Some(parent) = attribute(&path, &known, data, &epics) else { continue };
                    rows.extend(listed.into_iter().filter(|session| filter.as_ref().is_none_or(|id| *id == session.session_id)).map(|session| Row {
                        session_id: session.session_id, directory: path.clone(), project: parent.clone(),
                        revision: session.updated_at.to_string(), title: session.title, updated_at: session.updated_at as i64,
                    }));
                }
                Ok::<_, io::Error>((rows, Some(directories)))
            }
        }).await.map_err(io::Error::other)??;
        let scope = key.clone();
        let filter = target.clone();
        let changed = self.write(move |index, archive| replace(index, archive, &scope, rows, directories, filter.as_deref())).await?;
        if changed { self.notify(); }
        if key.0 == Agent::ClaudeCode {
            self.index_snapshots(&key, target.as_deref()).await?;
        }
        Ok(changed)
    }

    pub async fn list(&self, project: String, chats: Arc<Chats>) -> (Vec<Session>, Coverage) {
        let dir = self.dir.clone();
        let stale: BTreeSet<_> = self.stale.lock().unwrap().iter().map(|(agent, _)| *agent).collect();
        let cloud = self.cloud;
        let open = self.projects.list().iter().find(|entry| entry.id == project).cloned();
        let result = tokio::task::spawn_blocking(move || {
            let mut sessions = Vec::new();
            if let Some(project) = open {
                let index = read_connection(&dir.join("index.sqlite"))?;
                let mut query = index.prepare("SELECT agent, store, session_id, title, updated_at FROM sessions WHERE project = ? ORDER BY updated_at DESC, id").map_err(io::Error::other)?;
                let rows = query.query_map([&project.id], |row| {
                    let agent_name: String = row.get(0)?;
                    let agent = Agent::ALL.into_iter().find(|agent| agent.name() == agent_name).ok_or(rusqlite::Error::InvalidQuery)?;
                    Ok(Session {
                        key: SessionKey { agent, store: row.get(1)?, session_id: row.get(2)? },
                        project: SessionProject { id: project.id.clone(), path: project.path.clone() },
                        title: row.get(3)?, updated_at: row.get(4)?, chat_id: None, ended: None,
                    })
                }).map_err(io::Error::other)?;
                let live = chats.session_keys();
                for row in rows {
                    let row = row.map_err(io::Error::other)?;
                    if !live.contains(&row.key) && (!cloud || matches!(row.key.agent, Agent::ClaudeCode | Agent::Codex)) { sessions.push(row); }
                }
            }
            Ok::<_, io::Error>((sessions, coverage(&dir, stale)?))
        }).await;
        match result {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => { eprintln!("cannot read Session catalogue: {error}"); (Vec::new(), Coverage::default()) }
            Err(error) => { eprintln!("cannot read Session catalogue: {error}"); (Vec::new(), Coverage::default()) }
        }
    }

    pub async fn resume(&self, key: SessionKey, project: String, chats: Arc<Chats>) -> io::Result<Chat> {
        if self.cloud && !matches!(key.agent, Agent::ClaudeCode | Agent::Codex) {
            return Err(io::Error::other("A Cloud Host runs only Claude Code and Codex."));
        }
        if !self.resuming.lock().unwrap().insert(key.clone()) {
            return Err(io::Error::other("Another Chat is already resuming that Session."));
        }
        let _resuming = Resuming { key: key.clone(), pending: &self.resuming };
        let dir = self.dir.clone();
        let config = self.config.borrow().clone();
        let projects = self.projects.list();
        let id = key.session_id.clone();
        let (rig, cwd, project) = tokio::task::spawn_blocking(move || {
            let index = read_connection(&dir.join("index.sqlite"))?;
            let directory: String = index.query_row("SELECT directory FROM sessions WHERE agent = ? AND store = ? AND session_id = ? AND project = ?", params![key.agent.name(), key.store, key.session_id, project], |row| row.get(0))
                .optional().map_err(io::Error::other)?.ok_or_else(|| io::Error::other("Session is not in the catalogue."))?;
            let cwd = PathBuf::from(directory);
            if !cwd.is_dir() { return Err(io::Error::other("The Session's directory no longer exists.")); }
            let rig = config.rigs.iter().filter(|rig| rig.agent == key.agent).cloned().chain([Rig::starter(key.agent)])
                .find(|rig| store(rig.agent, &rig.env) == key.store).ok_or_else(|| io::Error::other("No Rig matches the Session's history store."))?;
            let project = projects.iter().find(|entry| entry.path == cwd.to_string_lossy())
                .or_else(|| projects.iter().find(|entry| entry.id == project)).cloned()
                .ok_or_else(|| io::Error::other("The Project is not open."))?;
            Ok::<_, io::Error>((rig, cwd, project))
        }).await.map_err(io::Error::other)??;
        chats.create(&project, &cwd, &rig, Some(&id), None).await
    }
}

struct Resuming<'a> {
    key: SessionKey,
    pending: &'a Mutex<HashSet<SessionKey>>,
}

impl Drop for Resuming<'_> {
    fn drop(&mut self) { self.pending.lock().unwrap().remove(&self.key); }
}

fn size(dir: &Path, file: &str) -> u64 {
    [file.to_owned(), format!("{file}-wal")].iter().filter_map(|name| fs::metadata(dir.join(name)).ok().map(|meta| meta.len())).sum()
}

fn remember(archive: &Connection, project: &Project) -> io::Result<()> {
    let common_dir = std::process::Command::new("git").current_dir(&project.path)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"]).output().ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|path| fs::canonicalize(path.trim_end_matches('\n')).ok())
        .map(|path| path.to_string_lossy().into_owned());
    archive.execute("INSERT INTO projects(id, path, parent, common_dir) VALUES (?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET path=excluded.path, parent=excluded.parent, common_dir=coalesce(excluded.common_dir, projects.common_dir)",
        params![project.id, project.path, project.worktree.as_ref().map(|worktree| &worktree.parent), common_dir]).map_err(io::Error::other)?;
    Ok(())
}

fn known_projects(archive: &Connection) -> io::Result<Vec<KnownProject>> {
    let mut query = archive.prepare("SELECT id, path, parent, common_dir FROM projects ORDER BY id").map_err(io::Error::other)?;
    let rows = query.query_map([], |row| Ok(KnownProject { id: row.get(0)?, path: row.get(1)?, parent: row.get(2)?, common_dir: row.get(3)? })).map_err(io::Error::other)?;
    rows.collect::<rusqlite::Result<_>>().map_err(io::Error::other)
}

fn attribute(directory: &str, known: &[KnownProject], data: &Path, epics: &Epics) -> Option<String> {
    if let Some(project) = known.iter().find(|project| project.path == directory) {
        return Some(project.parent.as_ref().unwrap_or(&project.id).clone());
    }
    let relative = Path::new(directory).strip_prefix(data.join("worktrees")).ok()?;
    let mut components = relative.components();
    let slug = components.next()?.as_os_str().to_str()?;
    if slug == "_branches" {
        let parent = components.next()?.as_os_str().to_str()?;
        return known.iter().find(|project| project.id == parent).map(|project| project.id.clone());
    }
    let epic = epics.get(slug).ok()?;
    known.iter().find(|project| project.parent.is_none() && project.common_dir.as_deref().map(Path::new) == Some(&epic.project)).map(|project| project.id.clone())
}

fn current(index: &Connection, key: &Store) -> io::Result<HashMap<String, Row>> {
    let mut query = index.prepare("SELECT session_id, directory, project, title, updated_at, revision FROM sessions WHERE agent = ? AND store = ?").map_err(io::Error::other)?;
    let rows = query.query_map(params![key.0.name(), key.1], |row| Ok(Row {
        session_id: row.get(0)?, directory: row.get(1)?, project: row.get(2)?, title: row.get(3)?, updated_at: row.get(4)?, revision: row.get(5)?,
    })).map_err(io::Error::other)?;
    rows.map(|row| row.map(|row| (row.session_id.clone(), row))).collect::<rusqlite::Result<_>>().map_err(io::Error::other)
}

fn entries(path: &Path) -> io::Result<Option<fs::ReadDir>> {
    match fs::read_dir(path) {
        Ok(entries) => Ok(Some(entries)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn claude_rows(store: &str, known: &[KnownProject], data: &Path, epics: &Epics, previous: &HashMap<String, Row>, target: Option<&str>) -> io::Result<Vec<Row>> {
    let mut rows = Vec::new();
    for slug in entries(&Path::new(store).join("projects"))?.into_iter().flatten() {
        let slug = slug?;
        if !fs::metadata(slug.path())?.is_dir() { continue; }
        for transcript in fs::read_dir(slug.path())? {
            let transcript = transcript?;
            let path = transcript.path();
            if path.extension().is_none_or(|extension| extension != "jsonl") { continue; }
            let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else { continue };
            if id.starts_with("agent-") || target.is_some_and(|target| id != target) { continue; }
            let metadata = fs::metadata(&path)?;
            if !metadata.is_file() { continue; }
            let revision = fingerprint(&metadata);
            if let Some(old) = previous.get(id).filter(|row| row.revision == revision) {
                if let Some(project) = attribute(&old.directory, known, data, epics) {
                    rows.push(Row { project, ..old.clone() });
                }
                continue;
            }
            let file = fs::File::open(&path)?;
            let mut reader = BufReader::new(file);
            let mut line = Vec::new();
            let mut directory = None;
            while reader.read_until(b'\n', &mut line)? != 0 {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&line) {
                    if let Some(cwd) = value["cwd"].as_str() { directory = Some(cwd.to_owned()); break; }
                }
                line.clear();
            }
            let directory = directory.or_else(|| {
                let mut matching = known.iter().filter(|project| project.path.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>() == slug.file_name().to_string_lossy());
                let project = matching.next()?;
                matching.next().is_none().then(|| project.path.clone())
            });
            let Some(directory) = directory else { continue };
            let directory = canonical(Path::new(&directory)).to_string_lossy().into_owned();
            let Some(project) = attribute(&directory, known, data, epics) else { continue };
            let Some(title) = claude::catalogue_title(&path)? else { continue };
            let title = crate::templates::recognize(&title, data).map_or(title, |(_, typed)| typed);
            if fingerprint(&fs::metadata(&path)?) != revision { return Err(io::Error::other("Session changed during listing")); }
            let updated_at = metadata.modified()?.duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_millis() as i64;
            rows.push(Row { session_id: id.to_owned(), directory, project, title: title.chars().take(200).collect(), updated_at, revision });
        }
    }
    Ok(rows)
}

fn fingerprint(metadata: &fs::Metadata) -> String {
    format!("{}:{}:{}:{}:{}:{}", metadata.len(), metadata.mtime(), metadata.mtime_nsec(), metadata.ino(), metadata.ctime(), metadata.ctime_nsec())
}

fn replace(index: &mut Connection, archive: &mut Connection, key: &Store, rows: Vec<Row>, directories: Option<HashSet<String>>, target: Option<&str>) -> io::Result<bool> {
    let previous = current(index, key)?;
    let transaction = index.transaction().map_err(io::Error::other)?;
    let mut seen = HashSet::new();
    let mut changed = false;
    for row in rows {
        seen.insert(row.session_id.clone());
        if previous.get(&row.session_id).is_some_and(|old| old.revision == row.revision && old.project == row.project && old.title == row.title && old.directory == row.directory) { continue; }
        transaction.execute("INSERT INTO sessions(agent, store, session_id, directory, project, title, updated_at, revision) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(agent, store, session_id) DO UPDATE SET directory=excluded.directory, project=excluded.project, title=excluded.title, updated_at=excluded.updated_at, revision=excluded.revision",
            params![key.0.name(), key.1, row.session_id, row.directory, row.project, row.title, row.updated_at, row.revision]).map_err(io::Error::other)?;
        changed = true;
    }
    for (id, old) in previous {
        if target.is_none_or(|target| target == id) && !seen.contains(&id) && directories.as_ref().is_none_or(|dirs| dirs.contains(&old.directory)) {
            if key.0 == Agent::ClaudeCode && old.updated_at < (super::now() as i64).saturating_sub(cleanup_days(&key.1).saturating_sub(1) * 86_400_000) {
                archive_session(&transaction, archive, key, &old)?;
            }
            transaction.execute("DELETE FROM sessions WHERE agent = ? AND store = ? AND session_id = ?", params![key.0.name(), key.1, id]).map_err(io::Error::other)?;
            changed = true;
        }
    }
    transaction.commit().map_err(io::Error::other)?;
    Ok(changed)
}

fn cleanup_days(store: &str) -> i64 {
    let days = |path: &Path| -> Option<i64> {
        serde_json::from_slice::<serde_json::Value>(&fs::read(path).ok()?).ok()?["cleanupPeriodDays"].as_i64().filter(|days| *days > 0)
    };
    let managed = Path::new("/etc/claude-code");
    let mut paths: Vec<_> = fs::read_dir(managed.join("managed-settings.d")).into_iter().flatten().flatten()
        .map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|ext| ext == "json") &&
            path.file_name().is_some_and(|name| !name.to_string_lossy().starts_with('.'))).collect();
    paths.sort();
    paths.iter().rev().find_map(|path| days(path))
        .or_else(|| days(&managed.join("managed-settings.json")))
        .or_else(|| days(&Path::new(store).join("settings.json"))).unwrap_or(30).min(i64::MAX / 86_400_000)
}

fn archive_session(index: &Connection, archive: &mut Connection, key: &Store, row: &Row) -> io::Result<()> {
    let (id, snapshot_revision): (i64, Option<String>) = index.query_row(
        "SELECT id, snapshot_revision FROM sessions WHERE agent=? AND store=? AND session_id=?",
        params![key.0.name(), key.1, row.session_id], |row| Ok((row.get(0)?, row.get(1)?))).map_err(io::Error::other)?;
    let transaction = archive.transaction().map_err(io::Error::other)?;
    let archived: i64 = transaction.query_row("
        INSERT INTO sessions(agent, store, session_id, directory, project, title, updated_at, revision, snapshot_revision, ended, ended_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'aged', ?)
        ON CONFLICT(agent, store, session_id) DO UPDATE SET directory=excluded.directory, project=excluded.project,
        title=excluded.title, updated_at=excluded.updated_at, revision=excluded.revision, snapshot_revision=excluded.snapshot_revision
        RETURNING id",
        params![key.0.name(), key.1, row.session_id, row.directory, row.project, row.title, row.updated_at, row.revision, snapshot_revision, super::now() as i64],
        |row| row.get(0)).map_err(io::Error::other)?;
    transaction.execute("DELETE FROM snapshots WHERE session=?", [archived]).map_err(io::Error::other)?;
    transaction.execute("DELETE FROM messages WHERE session=?", [archived]).map_err(io::Error::other)?;
    if let Some((version, items)) = index.query_row("SELECT version, items FROM snapshots WHERE session=?", [id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))).optional().map_err(io::Error::other)? {
        transaction.execute("INSERT INTO snapshots VALUES (?, ?, ?)", params![archived, version, items]).map_err(io::Error::other)?;
    }
    let mut query = index.prepare("SELECT item, role, text FROM messages WHERE session=?").map_err(io::Error::other)?;
    let messages = query.query_map([id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))).map_err(io::Error::other)?;
    for message in messages {
        let (item, role, text) = message.map_err(io::Error::other)?;
        transaction.execute("INSERT INTO messages(session, item, role, text) VALUES (?, ?, ?, ?)", params![archived, item, role, text]).map_err(io::Error::other)?;
    }
    transaction.commit().map_err(io::Error::other)
}

fn coverage(dir: &Path, stale: BTreeSet<Agent>) -> io::Result<Coverage> {
    let index = read_connection(&dir.join("index.sqlite"))?;
    let (total, indexed) = index.query_row("SELECT count(*), count(CASE WHEN snapshot_revision=revision AND snapshots.version=? THEN 1 END) FROM sessions LEFT JOIN snapshots ON snapshots.session=sessions.id", [SNAPSHOT_VERSION], |row| Ok((row.get::<_, i64>(0)? as usize, row.get::<_, i64>(1)? as usize))).map_err(io::Error::other)?;
    let ended = read_connection(&dir.join("archive.sqlite"))?.query_row("SELECT count(*) FROM sessions", [], |row| Ok(row.get::<_, i64>(0)? as usize)).map_err(io::Error::other)?;
    Ok(Coverage { total, indexed, ended, stale: stale.into_iter().collect(), bytes: Bytes { index: size(dir, "index.sqlite"), archive: size(dir, "archive.sqlite") } })
}

fn save_snapshot(index: &mut Connection, id: i64, revision: &str, path: &Path, items: &[super::Item]) -> io::Result<bool> {
    let transaction = index.transaction().map_err(io::Error::other)?;
    let current: Option<String> = transaction.query_row("SELECT revision FROM sessions WHERE id=?", [id], |row| row.get(0)).optional().map_err(io::Error::other)?;
    if current.as_deref() != Some(revision) || fs::metadata(path).ok().is_none_or(|metadata| fingerprint(&metadata) != revision) { return Ok(false); }
    let encoded = serde_json::to_vec(items).map_err(io::Error::other)?;
    transaction.execute("INSERT INTO snapshots(session, version, items) VALUES (?, ?, ?) ON CONFLICT(session) DO UPDATE SET version=excluded.version, items=excluded.items", params![id, SNAPSHOT_VERSION, encoded]).map_err(io::Error::other)?;
    transaction.execute("DELETE FROM messages WHERE session=?", [id]).map_err(io::Error::other)?;
    for (item, record) in items.iter().enumerate() {
        let (role, text) = match record {
            super::Item::User { text, .. } => ("user", text),
            super::Item::Text { text, .. } => ("agent", text),
            _ => continue,
        };
        transaction.execute("INSERT INTO messages(session, item, role, text) VALUES (?, ?, ?, ?)", params![id, item as i64, role, text]).map_err(io::Error::other)?;
    }
    transaction.execute("UPDATE sessions SET snapshot_revision=? WHERE id=?", params![revision, id]).map_err(io::Error::other)?;
    if fs::metadata(path).ok().is_none_or(|metadata| fingerprint(&metadata) != revision) { return Ok(false); }
    transaction.commit().map_err(io::Error::other)?;
    Ok(true)
}

impl Sessions {
    async fn index_snapshots(&self, key: &Store, target: Option<&str>) -> io::Result<()> {
        let dir = self.dir.clone();
        let store = key.1.clone();
        let target = target.map(str::to_owned);
        let rows = tokio::task::spawn_blocking(move || {
            let index = read_connection(&dir.join("index.sqlite"))?;
            let mut query = index.prepare("SELECT sessions.id, session_id, directory, revision FROM sessions LEFT JOIN snapshots ON snapshots.session=sessions.id
                WHERE agent=? AND store=? AND (? IS NULL OR session_id=?) AND (snapshot_revision IS NOT revision OR version IS NOT ?) ORDER BY updated_at DESC").map_err(io::Error::other)?;
            let rows = query.query_map(params![Agent::ClaudeCode.name(), store, target, target, SNAPSHOT_VERSION], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?))).map_err(io::Error::other)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(io::Error::other)
        }).await.map_err(io::Error::other)??;
        for (id, session, cwd, revision) in rows {
            let data = self.dir.parent().unwrap().to_owned();
            let store = key.1.clone();
            let expected = revision.clone();
            let minimum = self.config.borrow().storage.min_free_gib;
            let (send, receive) = oneshot::channel();
            std::thread::Builder::new().name("session-snapshot".to_owned()).spawn(move || {
                // Lower only this dedicated worker, not Tokio's shared blocking pool.
                unsafe { nix::libc::nice(10); }
                let result = (|| {
                    if crate::storage::Storage::new(&data).pressure(&data, minimum)?.is_some() { return Ok(None); }
                    let env = BTreeMap::from([("CLAUDE_CONFIG_DIR".to_owned(), store.clone())]);
                    let path = claude::session_file(&env, &session).ok_or_else(|| io::Error::other("Session disappeared"))?;
                    if fingerprint(&fs::metadata(&path)?) != expected { return Err(io::Error::other("Session changed before snapshot")); }
                    let items = claude::snapshot(&store, &session, Path::new(&cwd), &data)?;
                    if fingerprint(&fs::metadata(&path)?) != expected { return Err(io::Error::other("Session changed during snapshot")); }
                    Ok::<_, io::Error>(Some((path, items)))
                })();
                let _ = send.send(result);
            })?;
            match receive.await.map_err(io::Error::other)? {
                Ok(Some((path, items))) => {
                    if self.write(move |index, _| save_snapshot(index, id, &revision, &path, &items)).await? { self.notify(); }
                }
                Ok(None) => break,
                Err(error) => eprintln!("cannot snapshot Session {id}: {error}"),
            }
            tokio::task::yield_now().await;
        }
        Ok(())
    }
}

const TITLE_WEIGHT: f64 = 4.0;
const MESSAGE_WEIGHT: f64 = 1.0;
const EXCERPT_CHARS: usize = 160;
const OPEN_MARK: char = '\u{1}';
const CLOSE_MARK: char = '\u{2}';

fn fts_query(query: &str) -> String {
    let mut terms = Vec::new();
    let mut part = String::new();
    let mut quoted = false;
    let push = |terms: &mut Vec<(String, bool)>, part: &mut String, quoted| {
        if part.chars().any(char::is_alphanumeric) {
            terms.push((format!("\"{}\"", part.replace('"', "\"\"")), quoted));
        }
        part.clear();
    };
    for ch in query.chars() {
        if ch == '"' {
            push(&mut terms, &mut part, quoted);
            quoted = !quoted;
        } else if quoted || ch.is_alphanumeric() {
            part.push(ch);
        } else {
            push(&mut terms, &mut part, false);
        }
    }
    push(&mut terms, &mut part, quoted);
    if let Some((last, false)) = terms.last_mut() { last.push('*'); }
    terms.into_iter().map(|(term, _)| term).collect::<Vec<_>>().join(" AND ")
}

fn excerpt(who: String, highlighted: &str) -> Excerpt {
    let mut chars = Vec::new();
    let mut matched = false;
    for ch in highlighted.chars() {
        match ch {
            OPEN_MARK => matched = true,
            CLOSE_MARK => matched = false,
            _ => chars.push((ch, matched)),
        }
    }
    let first = chars.iter().position(|(_, matched)| *matched).unwrap_or(0);
    let start = first.saturating_sub(EXCERPT_CHARS / 3).min(chars.len().saturating_sub(EXCERPT_CHARS));
    let end = (start + EXCERPT_CHARS).min(chars.len());
    let mut text = String::new();
    let mut ranges = Vec::new();
    let mut offset = 0;
    let mut range = None;
    if start > 0 { text.push('…'); offset += 1; }
    for &(ch, matched) in &chars[start..end] {
        if matched && range.is_none() { range = Some(offset); }
        if !matched && let Some(begin) = range.take() { ranges.push((begin, offset)); }
        text.push(ch);
        offset += ch.len_utf16();
    }
    if let Some(begin) = range { ranges.push((begin, offset)); }
    if end < chars.len() { text.push('…'); }
    Excerpt { who, text, ranges }
}

#[derive(Default)]
struct Hit {
    title: f64,
    message: f64,
    count: usize,
    best: Option<(i64, usize)>,
}

fn search_db(db: &Connection, known: &[KnownProject], query: &str, archived: bool) -> io::Result<Vec<(Found, f64, Option<i64>)>> {
    let mut hits = HashMap::<i64, Hit>::new();
    let mut titles = db.prepare("SELECT rowid, bm25(titles_fts) FROM titles_fts WHERE titles_fts MATCH ?").map_err(io::Error::other)?;
    let rows = titles.query_map([query], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))).map_err(io::Error::other)?;
    for row in rows {
        let (id, score) = row.map_err(io::Error::other)?;
        hits.entry(id).or_default().title = -score;
    }
    let mut messages = db.prepare("SELECT messages.session, messages.id, messages.item, bm25(messages_fts)
        FROM messages_fts JOIN messages ON messages.id=messages_fts.rowid WHERE messages_fts MATCH ? ORDER BY messages.item").map_err(io::Error::other)?;
    let rows = messages.query_map([query], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)? as usize, row.get::<_, f64>(3)?))).map_err(io::Error::other)?;
    for row in rows {
        let (id, message, item, score) = row.map_err(io::Error::other)?;
        let hit = hits.entry(id).or_default();
        hit.count += 1;
        if hit.best.is_none() || -score > hit.message {
            hit.message = -score;
            hit.best = Some((message, item));
        }
    }
    let ended = if archived { "ended" } else { "NULL" };
    let mut session = db.prepare(&format!("SELECT agent, store, session_id, project, title, updated_at, {ended} FROM sessions WHERE id=?")).map_err(io::Error::other)?;
    let mut found = Vec::new();
    for (id, hit) in hits {
        let row = session.query_row([id], |row| {
            let name: String = row.get(0)?;
            let agent = Agent::ALL.into_iter().find(|agent| agent.name() == name).ok_or(rusqlite::Error::InvalidQuery)?;
            let project: String = row.get(3)?;
            Ok(Session {
                key: SessionKey { agent, store: row.get(1)?, session_id: row.get(2)? },
                project: SessionProject { path: known.iter().find(|known| known.id == project).map(|known| known.path.clone()).unwrap_or_default(), id: project },
                title: row.get(4)?, updated_at: row.get(5)?, chat_id: None, ended: row.get(6)?,
            })
        }).map_err(io::Error::other)?;
        let age = (super::now() as i64 - row.updated_at).max(0) as f64 / 86_400_000.0;
        let score = (TITLE_WEIGHT * hit.title + MESSAGE_WEIGHT * hit.message) / (1.0 + age.ln_1p());
        let (message, item) = hit.best.map_or((None, None), |(message, item)| (Some(message), Some(item)));
        found.push((Found { session: row, item, excerpt: None, matches: hit.count }, score, message));
    }
    Ok(found)
}

impl Sessions {
    pub async fn search(&self, project: String, query: String, chats: Arc<Chats>) -> (Vec<Found>, Coverage) {
        let dir = self.dir.clone();
        let stale = self.stale.lock().unwrap().iter().map(|(agent, _)| *agent).collect();
        let cloud = self.cloud;
        let result = tokio::task::spawn_blocking(move || {
            let query = fts_query(&query);
            let coverage = coverage(&dir, stale)?;
            if query.is_empty() { return Ok((Vec::new(), coverage)); }
            let archive = read_connection(&dir.join("archive.sqlite"))?;
            let index = read_connection(&dir.join("index.sqlite"))?;
            index.execute_batch("BEGIN").map_err(io::Error::other)?;
            // Pin the index read before the archive, matching archive-first moves.
            index.query_row("SELECT id FROM sessions LIMIT 1", [], |row| row.get::<_, i64>(0)).optional().map_err(io::Error::other)?;
            archive.execute_batch("BEGIN").map_err(io::Error::other)?;
            let known = known_projects(&archive)?;
            let focus = known.iter().find(|known| known.id == project).and_then(|known| known.parent.as_ref()).unwrap_or(&project);
            let mut found = search_db(&index, &known, &query, false)?;
            // Prefer the native Session if an archive-first move was interrupted.
            let native: HashSet<_> = found.iter().map(|(found, _, _)| found.session.key.clone()).collect();
            found.extend(search_db(&archive, &known, &query, true)?.into_iter().filter(|(found, _, _)| !native.contains(&found.session.key)));
            let live = chats.session_keys();
            let top: HashMap<_, _> = chats.list().into_iter().filter_map(|chat| Some((chat.session_key()?, chat.id))).collect();
            found.retain_mut(|(found, _, _)| {
                let session = &mut found.session;
                session.chat_id = top.get(&session.key).cloned();
                (!live.contains(&session.key) || session.chat_id.is_some()) && (!cloud || matches!(session.key.agent, Agent::ClaudeCode | Agent::Codex))
            });
            found.sort_by(|(a, sa, _), (b, sb, _)| (b.session.project.id == *focus).cmp(&(a.session.project.id == *focus))
                .then_with(|| sb.total_cmp(sa)).then_with(|| b.session.updated_at.cmp(&a.session.updated_at)).then_with(|| a.session.key.cmp(&b.session.key)));
            let mut results = Vec::new();
            for (mut found, _, message) in found.into_iter().take(50) {
                if let Some(message) = message {
                    let db = if found.session.ended.is_some() { &archive } else { &index };
                    let (who, highlighted): (String, String) = db.query_row(
                        "SELECT messages.role, highlight(messages_fts, 0, ?, ?) FROM messages_fts JOIN messages ON messages.id=messages_fts.rowid
                         WHERE messages_fts MATCH ? AND messages_fts.rowid=?",
                        params![OPEN_MARK.to_string(), CLOSE_MARK.to_string(), query, message], |row| Ok((row.get(0)?, row.get(1)?))).map_err(io::Error::other)?;
                    found.excerpt = Some(excerpt(who, &highlighted));
                }
                results.push(found);
            }
            Ok::<_, io::Error>((results, coverage))
        }).await;
        match result {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => { eprintln!("cannot search Sessions: {error}"); (Vec::new(), Coverage::default()) }
            Err(error) => { eprintln!("cannot search Sessions: {error}"); (Vec::new(), Coverage::default()) }
        }
    }
}
