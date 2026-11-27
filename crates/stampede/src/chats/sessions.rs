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

const VERSION: i32 = 1;
const ARCHIVE_VERSION: i32 = 1;

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

fn initialize(dir: &Path) -> io::Result<(Connection, Connection)> {
    fs::create_dir_all(dir)?;
    let archive = connection(&dir.join("archive.sqlite"))?;
    archive.pragma_update(None, "journal_mode", "WAL").map_err(io::Error::other)?;
    match version(&archive)? {
        0 => archive.execute_batch("BEGIN; CREATE TABLE IF NOT EXISTS projects (id TEXT PRIMARY KEY, path TEXT NOT NULL, parent TEXT, common_dir TEXT); PRAGMA user_version = 1; COMMIT;").map_err(io::Error::other)?,
        ARCHIVE_VERSION => (),
        other => return Err(io::Error::other(format!("Unsupported Session archive version {other}"))),
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
        db.execute_batch("
            BEGIN;
            CREATE TABLE sessions (
                id INTEGER PRIMARY KEY, agent TEXT NOT NULL, store TEXT NOT NULL, session_id TEXT NOT NULL,
                directory TEXT NOT NULL, project TEXT NOT NULL, title TEXT NOT NULL, updated_at INTEGER NOT NULL,
                revision TEXT NOT NULL, snapshot_revision TEXT, UNIQUE (agent, store, session_id)
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
            END;
            PRAGMA user_version = 1;
            COMMIT;
        ").map_err(io::Error::other)?;
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
                } else if !flight.full || flight.projects != self.projects.list() {
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
        self.write(move |index, _| replace(index, &key, rows, directories, target.as_deref())).await
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
                        title: row.get(3)?, updated_at: row.get(4)?,
                    })
                }).map_err(io::Error::other)?;
                let live = chats.session_keys();
                for row in rows {
                    let row = row.map_err(io::Error::other)?;
                    if !live.contains(&row.key) && (!cloud || matches!(row.key.agent, Agent::ClaudeCode | Agent::Codex)) { sessions.push(row); }
                }
            }
            let total = sessions.len();
            let bytes = Bytes { index: size(&dir, "index.sqlite"), archive: size(&dir, "archive.sqlite") };
            Ok::<_, io::Error>((sessions, Coverage { total, indexed: total, stale: stale.into_iter().collect(), ended: 0, bytes }))
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
    [file.to_owned(), format!("{file}-wal"), format!("{file}-shm")].iter().filter_map(|name| fs::metadata(dir.join(name)).ok().map(|meta| meta.len())).sum()
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

fn replace(index: &mut Connection, key: &Store, rows: Vec<Row>, directories: Option<HashSet<String>>, target: Option<&str>) -> io::Result<bool> {
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
            transaction.execute("DELETE FROM sessions WHERE agent = ? AND store = ? AND session_id = ?", params![key.0.name(), key.1, id]).map_err(io::Error::other)?;
            changed = true;
        }
    }
    transaction.commit().map_err(io::Error::other)?;
    Ok(changed)
}
