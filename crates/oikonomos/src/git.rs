use std::{
    collections::{BTreeMap, HashMap, btree_map::Entry},
    path::{Component, Path, PathBuf},
    time::Duration,
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
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename = "git.changed", rename_all = "camelCase")]
pub struct Changed {
    pub project_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    is_repo: bool,
    changes: Vec<Change>,
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
pub struct HeadQuery {
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
    let changes = changes(&root(&state, &id)?).await;
    Ok(Json(Status { is_repo: changes.is_some(), changes: changes.unwrap_or_default() }))
}

pub async fn head(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Query(query): Query<HeadQuery>,
) -> Result<Vec<u8>, Failure> {
    let dir = root(&state, &id)?;
    inside(&query.path)?;
    Ok(content_at(&dir, "HEAD", &query.path).await)
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

/// Every uncommitted change against `HEAD`, or `None` when `dir` is not in a git repo.
pub async fn changes(dir: &Path) -> Option<Vec<Change>> {
    let toplevel = toplevel(dir).await?;
    let prefix = dir.strip_prefix(toplevel).ok()?;
    let args = ["status", "--porcelain=v2", "-z", "--untracked-files=all", "--find-renames", "--", "."];
    Some(parse(&git(dir, &args).await?, prefix))
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

async fn toplevel(dir: &Path) -> Option<PathBuf> {
    let output = git(dir, &["rev-parse", "--show-toplevel"]).await?;
    let toplevel = PathBuf::from(String::from_utf8(output).ok()?.trim_end_matches('\n'));
    dir.starts_with(&toplevel).then_some(toplevel)
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

fn parse(output: &[u8], prefix: &Path) -> Vec<Change> {
    let relative = |path: &str| Some(Path::new(path).strip_prefix(prefix).ok()?.to_str()?.to_owned());
    let mut changes = BTreeMap::new();
    let mut records = output.split(|&byte| byte == 0).map(String::from_utf8_lossy);
    while let Some(record) = records.next() {
        let fields = |count| record.splitn(count, ' ').collect::<Vec<_>>();
        let (kind, path, from) = match record.split_once(' ') {
            Some(("1", _)) => {
                let [_, xy, .., path] = fields(9)[..] else { continue };
                match (xy.contains('A'), xy.contains('D')) {
                    (true, true) => continue,
                    (true, false) => (Kind::Added, relative(path), None),
                    (false, true) => (Kind::Deleted, relative(path), None),
                    (false, false) => (Kind::Modified, relative(path), None),
                }
            }
            Some(("2", _)) => {
                let [_, xy, .., path] = fields(10)[..] else { continue };
                let Some(from) = records.next() else { break };
                match (xy.starts_with('C'), xy.ends_with('D')) {
                    (true, true) => continue,
                    (true, false) => (Kind::Added, relative(path), None),
                    (false, true) => (Kind::Deleted, relative(&from), None),
                    (false, false) => (Kind::Renamed, relative(path), relative(&from)),
                }
            }
            Some(("u", _)) => {
                let [.., path] = fields(11)[..] else { continue };
                (Kind::Modified, relative(path), None)
            }
            Some(("?", path)) => (Kind::Untracked, relative(path), None),
            _ => continue,
        };
        let Some(path) = path else { continue };
        match changes.entry(path.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(Change { path, kind, from });
            }
            // Removed from the index yet still on disk, the path is listed both deleted and untracked.
            Entry::Occupied(mut entry) => entry.get_mut().kind = Kind::Modified,
        }
    }
    changes.into_values().collect()
}

pub fn spawn(
    mut projects: watch::Receiver<List>,
    disk: &broadcast::Sender<watcher::Changed>,
) -> broadcast::Sender<Changed> {
    let (changes, _) = broadcast::channel(256);
    let sender = changes.clone();
    let mut disk = disk.subscribe();
    projects.mark_changed();
    tokio::spawn(async move {
        let mut throttles = HashMap::<String, mpsc::UnboundedSender<bool>>::new();
        loop {
            tokio::select! {
                opened = projects.changed() => {
                    if opened.is_err() {
                        break;
                    }
                    let list = projects.borrow_and_update().clone();
                    throttles.retain(|id, _| list.iter().any(|project| project.id == *id));
                    for project in list.iter() {
                        if !throttles.contains_key(&project.id) {
                            throttles.insert(project.id.clone(), throttle(project.clone(), sender.clone()));
                        }
                    }
                }
                changed = disk.recv() => match changed {
                    Ok(changed) => {
                        if let Some(throttle) = throttles.get(&changed.project_id) {
                            let _ = throttle.send(changed.git);
                        }
                    }
                    Err(RecvError::Lagged(_)) => {
                        for throttle in throttles.values() {
                            let _ = throttle.send(true);
                        }
                    }
                    Err(RecvError::Closed) => break,
                },
            }
        }
    });
    changes
}

fn throttle(project: Project, changed: broadcast::Sender<Changed>) -> mpsc::UnboundedSender<bool> {
    let (signal, mut signals) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let dir = PathBuf::from(&project.path);
        let mut last = None;
        while let Some(mut git) = signals.recv().await {
            while let Ok(more) = signals.try_recv() {
                git |= more;
            }
            let changes = changes(&dir).await;
            if git || changes != last {
                let _ = changed.send(Changed { project_id: project.id.clone() });
            }
            last = changes;
            sleep(THROTTLE).await;
        }
    });
    signal
}
