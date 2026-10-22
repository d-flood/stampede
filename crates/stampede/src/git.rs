use std::{
    collections::{BTreeMap, HashMap, btree_map::Entry},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use axum::{
    Json,
    extract::{Path as Param, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use tokio::{
    process::Command,
    sync::{
        broadcast::{self, error::RecvError},
        mpsc, watch,
    },
    time::sleep,
};

use crate::{
    AppState,
    files::{Failure, root},
    projects::{List, Project},
    watcher,
};

pub mod actions;

const THROTTLE: Duration = Duration::from_millis(250);
const PAGE: usize = 100;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Change {
    pub path: String,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename = "git.changed", rename_all = "camelCase")]
pub struct Changed {
    pub project_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    is_repo: bool,
    is_root: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    head: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    detached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    upstream: Option<String>,
    ahead: u32,
    behind: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    origin: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<Operation>,
    /// Everything uncommitted against HEAD, one entry per path.
    changes: Vec<Change>,
    staged: Vec<Change>,
    unstaged: Vec<Change>,
    conflicted: Vec<Change>,
}

#[derive(Serialize)]
pub struct Refs {
    local: Vec<String>,
    remote: Vec<String>,
}

#[derive(Serialize)]
pub struct Commit {
    sha: String,
    short: String,
    subject: String,
    author: String,
    date: String,
}

#[derive(Serialize)]
pub struct Log {
    commits: Vec<Commit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
}

#[derive(Deserialize)]
pub struct PathQuery {
    path: String,
}

#[derive(Deserialize)]
pub struct LogQuery {
    cursor: Option<String>,
}

#[derive(Deserialize)]
pub struct ShowQuery {
    sha: String,
    path: String,
}

pub async fn status(State(state): State<AppState>, Param(id): Param<String>) -> Result<Json<Status>, Failure> {
    let dir = root(&state, &id)?;
    Ok(Json(state.git.status(&id, &dir).await))
}

pub async fn refs(State(state): State<AppState>, Param(id): Param<String>) -> Result<Json<Refs>, Failure> {
    let output = git(&root(&state, &id)?, &["for-each-ref", "--sort=-committerdate", "--format=%(refname)", "refs/heads", "refs/remotes"]).await.unwrap_or_default();
    let mut refs = Refs { local: Vec::new(), remote: Vec::new() };
    for name in String::from_utf8_lossy(&output).lines() {
        if let Some(local) = name.strip_prefix("refs/heads/") {
            refs.local.push(local.to_owned());
        } else if let Some(remote) = name.strip_prefix("refs/remotes/").filter(|remote| !remote.ends_with("/HEAD")) {
            refs.remote.push(remote.to_owned());
        }
    }
    Ok(Json(refs))
}

pub async fn history(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Query(query): Query<LogQuery>,
) -> Result<Json<Log>, Failure> {
    let dir = root(&state, &id)?;
    let cursor = query.cursor.as_deref().filter(|cursor| !cursor.is_empty());
    let cursor = cursor
        .map(|cursor| page(cursor).ok_or_else(|| (StatusCode::BAD_REQUEST, format!("{cursor} is not a cursor"))))
        .transpose()?;
    Ok(Json(log(&dir, cursor).await))
}

pub async fn commit(
    State(state): State<AppState>,
    Param((id, commit)): Param<(String, String)>,
) -> Result<Json<Vec<Change>>, Failure> {
    let dir = root(&state, &id)?;
    let commit = sha(&commit).ok_or_else(|| (StatusCode::BAD_REQUEST, format!("{commit} is not a sha")))?;
    let files = commit_files(&dir, commit).await;
    files.map(Json).ok_or_else(|| (StatusCode::NOT_FOUND, format!("No commit {commit}")))
}

pub async fn show(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Query(query): Query<ShowQuery>,
) -> Result<Vec<u8>, Failure> {
    let dir = root(&state, &id)?;
    inside(&query.path)?;
    if sha(query.sha.strip_suffix('^').unwrap_or(&query.sha)).is_none() {
        return Err((StatusCode::BAD_REQUEST, format!("{} is not a sha", query.sha)));
    }
    Ok(content_at(&dir, &query.sha, &query.path).await)
}

pub async fn index(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Query(query): Query<PathQuery>,
) -> Result<Vec<u8>, Failure> {
    let dir = root(&state, &id)?;
    inside(&query.path)?;
    Ok(content_at(&dir, "", &query.path).await)
}

pub async fn message(State(state): State<AppState>, Param((id, commit)): Param<(String, String)>) -> Result<String, Failure> {
    let dir = root(&state, &id)?;
    let commit = sha(&commit).ok_or_else(|| (StatusCode::BAD_REQUEST, format!("{commit} is not a sha")))?;
    let output = git(&dir, &["log", "-1", "--format=%B", commit, "--"]).await;
    let message = output.ok_or_else(|| (StatusCode::NOT_FOUND, format!("No commit {commit}")))?;
    Ok(String::from_utf8_lossy(&message).trim_end().to_owned())
}

pub async fn read(dir: &Path) -> Option<Status> {
    Repo::default().read(dir, true).await
}

/// What a Project's status needs besides `git status`: where its repository is, found once, and whether it has an
/// origin, read again only when its git directory or the repository's config changes.
#[derive(Default)]
struct Repo {
    located: Option<(PathBuf, PathBuf, PathBuf)>,
    origin: Option<(Option<SystemTime>, bool)>,
}

impl Repo {
    async fn read(&mut self, dir: &Path, git_changed: bool) -> Option<Status> {
        if self.located.is_none() {
            self.located = locate(dir).await;
        }
        let (toplevel, git_dir, config) = self.located.as_ref()?;
        let prefix = dir.strip_prefix(toplevel).ok()?;
        let configured = tokio::fs::metadata(config).await.and_then(|metadata| metadata.modified()).ok();
        let known = self.origin.filter(|(at, _)| !git_changed && *at == configured).map(|(_, origin)| origin);
        let origin = async move {
            match known {
                Some(origin) => origin,
                None => git(dir, &["config", "--get", "remote.origin.url"]).await.is_some(),
            }
        };
        let args = ["status", "--porcelain=v2", "-z", "--branch", "--untracked-files=all", "--find-renames", "--", "."];
        let (output, origin) = tokio::join!(git(dir, &args), origin);
        let Some(output) = output else {
            *self = Self::default();
            return None;
        };
        let mut status = parse(&output, prefix);
        status.origin = origin;
        status.operation = operation(git_dir);
        self.origin = Some((configured, origin));
        Some(status)
    }
}

fn operation(git_dir: &Path) -> Option<Operation> {
    [
        ("rebase-merge", Operation::Rebase),
        ("rebase-apply", Operation::Rebase),
        ("MERGE_HEAD", Operation::Merge),
        ("CHERRY_PICK_HEAD", Operation::CherryPick),
        ("REVERT_HEAD", Operation::Revert),
    ]
    .into_iter()
    .find_map(|(name, operation)| git_dir.join(name).exists().then_some(operation))
}

/// A page of the current branch. The cursor pins the first page's tip, so new commits never shift later pages.
pub async fn log(dir: &Path, cursor: Option<(&str, usize)>) -> Log {
    let (start, skip) = cursor.unwrap_or(("HEAD", 0));
    let (count, skip_arg) = (format!("--max-count={PAGE}"), format!("--skip={skip}"));
    let args = ["log", "-z", "--format=%H%x00%h%x00%s%x00%an%x00%ar", &count, &skip_arg, start, "--"];
    let output = git(dir, &args).await.unwrap_or_default();
    let mut fields = output.split(|&byte| byte == 0).map(|field| String::from_utf8_lossy(field).into_owned());
    let mut commits = Vec::new();
    while let (Some(sha), Some(short), Some(subject), Some(author), Some(date)) =
        (fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
    {
        commits.push(Commit { sha, short, subject, author, date });
    }
    let cursor = (commits.len() == PAGE).then(|| {
        let tip = cursor.map_or(commits[0].sha.as_str(), |(tip, _)| tip);
        format!("{tip}:{}", skip + PAGE)
    });
    Log { commits, cursor }
}

pub async fn commit_files(dir: &Path, sha: &str) -> Option<Vec<Change>> {
    let args = [
        "diff-tree",
        "-r",
        "-z",
        "--no-commit-id",
        "--name-status",
        "--find-renames",
        "--relative",
        "--root",
        "--diff-merges=first-parent",
        sha,
        "--",
    ];
    let output = git(dir, &args).await?;
    let mut fields = output.split(|&byte| byte == 0).map(|field| String::from_utf8_lossy(field).into_owned());
    let mut files = Vec::new();
    while let Some(status) = fields.next() {
        let Some(path) = fields.next() else { break };
        files.push(match status.chars().next() {
            Some('A') => Change { path, kind: Kind::Added, from: None },
            Some('D') => Change { path, kind: Kind::Deleted, from: None },
            Some('R') => Change { path: fields.next()?, kind: Kind::Renamed, from: Some(path) },
            Some('C') => Change { path: fields.next()?, kind: Kind::Added, from: None },
            _ => Change { path, kind: Kind::Modified, from: None },
        });
    }
    Some(files)
}

pub async fn content_at(dir: &Path, rev: &str, path: &str) -> Vec<u8> {
    git(dir, &["show", &format!("{rev}:./{path}")]).await.unwrap_or_default()
}

fn inside(path: &str) -> Result<(), Failure> {
    if Path::new(path).components().all(|component| matches!(component, Component::Normal(_))) {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, format!("{path} is outside the Project")))
    }
}

fn page(cursor: &str) -> Option<(&str, usize)> {
    let (tip, skip) = cursor.split_once(':')?;
    Some((sha(tip)?, skip.parse().ok()?))
}

fn sha(text: &str) -> Option<&str> {
    (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(text)
}

pub async fn common_dir(dir: &Path) -> Option<PathBuf> {
    let output = git(dir, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).await?;
    let common = PathBuf::from(String::from_utf8(output).ok()?.trim_end_matches('\n'));
    tokio::fs::canonicalize(common).await.ok()
}

/// The toplevel, the git directory, and the config file, which linked worktrees share with their main repository.
async fn locate(dir: &Path) -> Option<(PathBuf, PathBuf, PathBuf)> {
    let args = ["rev-parse", "--show-toplevel", "--absolute-git-dir", "--path-format=absolute", "--git-common-dir"];
    let output = String::from_utf8(git(dir, &args).await?).ok()?;
    let mut lines = output.trim_end_matches('\n').split('\n');
    let (toplevel, git_dir, common_dir) = (PathBuf::from(lines.next()?), lines.next()?, lines.next()?);
    dir.starts_with(&toplevel).then(|| (toplevel, PathBuf::from(git_dir), Path::new(common_dir).join("config")))
}

/// Runs git in `dir`, giving its trimmed output, or what it printed when it failed.
pub async fn run(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|error| format!("cannot run git: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim_end().to_owned();
    if output.status.success() {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if stderr.is_empty() { format!("git {} failed: {stdout}", args.join(" ")) } else { stderr })
}

async fn git(dir: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .await
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn parse(output: &[u8], prefix: &Path) -> Status {
    let relative = |path: &str| Some(Path::new(path).strip_prefix(prefix).ok()?.to_str()?.to_owned());
    let mut status = Status { is_repo: true, is_root: prefix.as_os_str().is_empty(), ..Status::default() };
    let (mut head, mut oid) = (None, None);
    let mut changes = BTreeMap::new();
    let mut records = output.split(|&byte| byte == 0).map(String::from_utf8_lossy);
    while let Some(record) = records.next() {
        let fields = |count| record.splitn(count, ' ').collect::<Vec<_>>();
        let (kind, path, from) = match record.split_once(' ') {
            Some(("#", header)) => {
                if let Some(name) = header.strip_prefix("branch.head ") {
                    head = Some(name.to_owned());
                } else if let Some(commit) = header.strip_prefix("branch.oid ") {
                    status.head = (commit != "(initial)").then(|| commit.to_owned());
                    oid = Some(commit.chars().take(7).collect());
                } else if let Some(upstream) = header.strip_prefix("branch.upstream ") {
                    status.upstream = Some(upstream.to_owned());
                } else if let Some(counts) = header.strip_prefix("branch.ab ") {
                    let mut counts = counts.split(' ').map(|count| count.get(1..).and_then(|count| count.parse().ok()).unwrap_or(0));
                    (status.ahead, status.behind) = (counts.next().unwrap_or(0), counts.next().unwrap_or(0));
                }
                continue;
            }
            Some(("1", _)) => {
                let [_, xy, .., path] = fields(9)[..] else { continue };
                let Some(path) = relative(path) else { continue };
                sides(&mut status, xy, &path, None);
                match (xy.contains('A'), xy.contains('D')) {
                    (true, true) => continue,
                    (true, false) => (Kind::Added, path, None),
                    (false, true) => (Kind::Deleted, path, None),
                    (false, false) => (Kind::Modified, path, None),
                }
            }
            Some(("2", _)) => {
                let [_, xy, .., path] = fields(10)[..] else { continue };
                let Some(from) = records.next() else { break };
                let (Some(path), from) = (relative(path), relative(&from)) else { continue };
                sides(&mut status, xy, &path, from.clone());
                match (xy.starts_with('C'), xy.ends_with('D')) {
                    (true, true) => continue,
                    (true, false) => (Kind::Added, path, None),
                    (false, true) => match from {
                        Some(from) => (Kind::Deleted, from, None),
                        None => continue,
                    },
                    (false, false) => (Kind::Renamed, path, from),
                }
            }
            Some(("u", _)) => {
                let [.., path] = fields(11)[..] else { continue };
                let Some(path) = relative(path) else { continue };
                status.conflicted.push(Change { path: path.clone(), kind: Kind::Conflicted, from: None });
                (Kind::Modified, path, None)
            }
            Some(("?", path)) => {
                let Some(path) = relative(path) else { continue };
                status.unstaged.push(Change { path: path.clone(), kind: Kind::Untracked, from: None });
                (Kind::Untracked, path, None)
            }
            _ => continue,
        };
        match changes.entry(path.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(Change { path, kind, from });
            }
            // Removed from the index yet still on disk, the path is listed both deleted and untracked.
            Entry::Occupied(mut entry) => entry.get_mut().kind = Kind::Modified,
        }
    }
    status.detached = head.as_deref() == Some("(detached)");
    status.branch = if status.detached { oid } else { head };
    status.changes = changes.into_values().collect();
    for list in [&mut status.staged, &mut status.unstaged, &mut status.conflicted] {
        list.sort_by(|a, b| a.path.cmp(&b.path));
    }
    status
}

/// Records one porcelain entry's index (X) and worktree (Y) halves.
fn sides(status: &mut Status, xy: &str, path: &str, from: Option<String>) {
    let mut codes = xy.chars();
    let (staged, unstaged) = (codes.next().unwrap_or('.'), codes.next().unwrap_or('.'));
    let change = |kind, from| Change { path: path.to_owned(), kind, from };
    match staged {
        '.' => {}
        'A' | 'C' => status.staged.push(change(Kind::Added, None)),
        'D' => status.staged.push(change(Kind::Deleted, None)),
        'R' => status.staged.push(change(Kind::Renamed, from)),
        _ => status.staged.push(change(Kind::Modified, None)),
    }
    match unstaged {
        '.' => {}
        'A' => status.unstaged.push(change(Kind::Added, None)),
        'D' => status.unstaged.push(change(Kind::Deleted, None)),
        _ => status.unstaged.push(change(Kind::Modified, None)),
    }
}

/// Each open Project's git status, read by its throttle once per change while a Device is connected and served to
/// every Device from that read.
#[derive(Clone)]
pub struct Git {
    changed: broadcast::Sender<Changed>,
    throttles: Arc<Mutex<HashMap<String, Throttle>>>,
    devices: watch::Receiver<usize>,
}

struct Throttle {
    signal: mpsc::UnboundedSender<bool>,
    status: watch::Sender<Option<Status>>,
}

impl Git {
    pub fn subscribe(&self) -> broadcast::Receiver<Changed> {
        self.changed.subscribe()
    }

    /// Has the Project read again after git changed it; Devices hear `git.changed` once that read is shared.
    pub fn reread(&self, id: &str) {
        match self.throttles.lock().unwrap().get(id) {
            Some(throttle) => {
                let _ = throttle.signal.send(true);
            }
            None => {
                let _ = self.changed.send(Changed { project_id: id.to_owned() });
            }
        }
    }

    /// The shared status while throttles keep it fresh, that is while a Device is connected; otherwise a new read.
    async fn status(&self, id: &str, dir: &Path) -> Status {
        let shared = self.throttles.lock().unwrap().get(id).map(|throttle| throttle.status.clone());
        if *self.devices.borrow() > 0
            && let Some(status) = shared.as_ref().and_then(|shared| shared.borrow().clone())
        {
            return status;
        }
        let status = read(dir).await.unwrap_or_default();
        if let Some(shared) = shared {
            shared.send_if_modified(|current| {
                let empty = current.is_none();
                if empty {
                    *current = Some(status.clone());
                }
                empty
            });
        }
        status
    }
}

pub fn spawn(
    mut projects: watch::Receiver<List>,
    disk: &broadcast::Sender<watcher::Changed>,
    mut devices: watch::Receiver<usize>,
) -> Git {
    let (changes, _) = broadcast::channel(256);
    let git = Git { changed: changes.clone(), throttles: Arc::default(), devices: devices.clone() };
    let throttles = Arc::clone(&git.throttles);
    let mut disk = disk.subscribe();
    projects.mark_changed();
    let mut connected = *devices.borrow_and_update();
    tokio::spawn(async move {
        let signal = |git: bool| {
            for throttle in throttles.lock().unwrap().values() {
                let _ = throttle.signal.send(git);
            }
        };
        loop {
            tokio::select! {
                opened = projects.changed() => {
                    if opened.is_err() {
                        break;
                    }
                    let list = projects.borrow_and_update().clone();
                    let mut throttles = throttles.lock().unwrap();
                    throttles.retain(|id, _| list.iter().any(|project| project.id == *id));
                    for project in list.iter() {
                        if !throttles.contains_key(&project.id) {
                            throttles.insert(project.id.clone(), throttle(project.clone(), changes.clone(), devices.clone()));
                        }
                    }
                }
                changed = disk.recv() => match changed {
                    Ok(changed) => {
                        if let Some(throttle) = throttles.lock().unwrap().get(&changed.project_id) {
                            let _ = throttle.signal.send(changed.git);
                        }
                    }
                    Err(RecvError::Lagged(_)) => signal(true),
                    Err(RecvError::Closed) => break,
                },
                Ok(()) = devices.changed() => {
                    let count = *devices.borrow_and_update();
                    if connected == 0 && count > 0 {
                        signal(false);
                    }
                    connected = count;
                }
            }
        }
    });
    git
}

/// Reads the Project on each batch of signals while a Device is connected, at most once per THROTTLE, sharing the
/// result before telling Devices. Without a Device it drops the shared status instead, remembering whether git changed.
fn throttle(project: Project, changed: broadcast::Sender<Changed>, devices: watch::Receiver<usize>) -> Throttle {
    let (signal, mut signals) = mpsc::unbounded_channel();
    let (status, _) = watch::channel(None);
    let shared = status.clone();
    tokio::spawn(async move {
        let dir = PathBuf::from(&project.path);
        let mut repo = Repo::default();
        let mut missed = false;
        while let Some(mut git) = signals.recv().await {
            while let Ok(more) = signals.try_recv() {
                git |= more;
            }
            if *devices.borrow() == 0 {
                missed |= git;
                shared.send_replace(None);
                continue;
            }
            git |= std::mem::take(&mut missed);
            let read = repo.read(&dir, git).await.unwrap_or_default();
            let differs = shared.borrow().as_ref() != Some(&read);
            shared.send_replace(Some(read));
            if git || differs {
                let _ = changed.send(Changed { project_id: project.id.clone() });
            }
            sleep(THROTTLE).await;
        }
    });
    Throttle { signal, status }
}
