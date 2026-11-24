use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    path::{Path, PathBuf},
    process::Stdio,
};

use axum::{
    Json,
    body::Body,
    extract::{Path as Param, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
    process::Command,
    sync::mpsc,
};

use super::{Commit, Kind, Operation, Status, common_dir, inside, read, run_alone, sha, trunk};
use crate::{
    AppState,
    files::{Failure, root},
    origin, rigs, templates,
};

const DIFF_LIMIT: usize = 60_000;
const REGROUP_LIMIT: usize = 200;
const SIDE_LIMIT: usize = 50;

#[derive(Deserialize)]
pub struct Paths {
    paths: Vec<String>,
}

#[derive(Deserialize)]
pub struct Message {
    message: String,
}

#[derive(Deserialize)]
pub struct Reword {
    sha: String,
    message: String,
}

#[derive(Deserialize)]
pub struct Squash {
    sha: String,
    head: String,
    message: String,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize)]
pub struct RegroupQuery {
    sha: String,
}

#[derive(Serialize)]
pub struct Regroupable {
    head: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trunk: Option<String>,
    commits: Vec<Regroupee>,
}

#[derive(Serialize)]
pub struct Regroupee {
    #[serde(flatten)]
    commit: Commit,
    message: String,
    pushed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    merge: Option<Merged>,
}

/// What a merge brought in: a side branch's own commits, or the trunk's.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Merged {
    Side { commits: Vec<Commit>, more: usize },
    #[serde(rename_all = "camelCase")]
    Sync { count: usize, onto: String, foldable: bool },
}

#[derive(Deserialize)]
pub struct Regroup {
    head: String,
    oldest: String,
    groups: Vec<Group>,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize)]
pub struct Group {
    newest: String,
    message: Option<String>,
}

#[derive(Deserialize)]
pub struct Branch {
    branch: String,
}

#[derive(Serialize)]
pub struct Stash {
    message: String,
    date: String,
}

pub async fn stage(State(state): State<AppState>, Param(id): Param<String>, Json(request): Json<Paths>) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    let paths = chosen(&request.paths)?;
    for conflict in status.conflicted.iter().filter(|conflict| paths.contains(&conflict.path.as_str())) {
        if marked(&dir.join(&conflict.path)).await {
            return Err(refuse(format!("{} still has conflict markers. Resolve them before staging it.", conflict.path)));
        }
    }
    change(&state, &id, &dir, &[&["--literal-pathspecs", "add", "-A", "--"], &paths[..]].concat()).await
}

pub async fn unstage(State(state): State<AppState>, Param(id): Param<String>, Json(request): Json<Paths>) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    let paths = chosen(&request.paths)?;
    unconflicted(&status, &paths)?;
    let unborn = exec(&dir, &["rev-parse", "--verify", "-q", "HEAD"]).await.is_err();
    let command: &[&str] = if unborn {
        &["--literal-pathspecs", "rm", "--cached", "-r", "-q", "--ignore-unmatch", "--"]
    } else {
        &["--literal-pathspecs", "restore", "--staged", "--"]
    };
    change(&state, &id, &dir, &[command, &paths[..]].concat()).await
}

pub async fn discard(State(state): State<AppState>, Param(id): Param<String>, Json(request): Json<Paths>) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    let paths = chosen(&request.paths)?;
    unconflicted(&status, &paths)?;
    let (untracked, tracked): (Vec<_>, Vec<_>) = status.unstaged.iter()
        .filter(|change| paths.contains(&change.path.as_str()))
        .map(|change| (change.kind, change.path.as_str()))
        .partition(|(kind, _)| *kind == Kind::Untracked);
    let [untracked, tracked] = [untracked, tracked].map(|changes| changes.into_iter().map(|(_, path)| path).collect::<Vec<_>>());
    let mut result = Ok(String::new());
    if !tracked.is_empty() {
        result = exec(&dir, &[&["--literal-pathspecs", "restore", "--worktree", "--"], &tracked[..]].concat()).await;
    }
    if result.is_ok() && !untracked.is_empty() {
        result = exec(&dir, &[&["--literal-pathspecs", "clean", "-f", "-q", "--"], &untracked[..]].concat()).await;
    }
    changed(&state, &id);
    result.map(|_| StatusCode::NO_CONTENT).map_err(|output| (StatusCode::BAD_REQUEST, output))
}

/// Commits, streaming what git and its hooks print as JSON lines that end with whether it succeeded.
pub async fn commit(State(state): State<AppState>, Param(id): Param<String>, Json(Message { message }): Json<Message>) -> Result<Response, Failure> {
    let (dir, status) = open(&state, &id).await?;
    if message.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Write a commit message first.".to_owned()));
    }
    if !status.conflicted.is_empty() {
        return Err(refuse("Resolve and stage every conflicted file first."));
    }
    if status.operation == Some(Operation::Rebase) {
        return Err(refuse("A rebase is in progress. Continue or abort it in a terminal."));
    }
    if status.staged.is_empty() && status.operation.is_none() {
        return Err(refuse("Stage changes to commit first."));
    }
    let mut command = git(&dir, &["commit", "-q", "-m", &message]);
    command.stdin(Stdio::null());
    Ok(streamed(state, id, command))
}

/// Undoes the last commit, keeping its changes staged, and gives back its message.
pub async fn undo(State(state): State<AppState>, Param(id): Param<String>) -> Result<Json<String>, Failure> {
    let (dir, status) = open(&state, &id).await?;
    settled(&status)?;
    let head = exec(&dir, &["rev-parse", "--verify", "-q", "HEAD"]).await.map_err(|_| refuse("There is no commit to undo."))?;
    unpushed(&dir, &head).await?;
    let message = exec(&dir, &["log", "-1", "--format=%B", &head]).await.unwrap_or_default();
    match exec(&dir, &["rev-parse", "--verify", "-q", "HEAD^"]).await {
        Ok(parent) => change(&state, &id, &dir, &["reset", "--soft", "-q", &parent]).await?,
        Err(_) => change(&state, &id, &dir, &["update-ref", "-d", "HEAD", &head]).await?,
    };
    Ok(Json(message))
}

/// Changes the message of a commit on this branch, recreating its descendants on top.
/// Trees are untouched, so the index and working tree stay as they are.
pub async fn reword(State(state): State<AppState>, Param(id): Param<String>, Json(request): Json<Reword>) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    settled(&status)?;
    let message = written(&request.message)?;
    let (commit, head) = on_branch(&dir, &request.sha).await?;
    unpushed(&dir, &commit).await?;
    let tip = rewrite(&dir, &commit, &head, message).await.map_err(|output| (StatusCode::BAD_REQUEST, output))?;
    let reason = format!("reword: {}", message.lines().next().unwrap_or_default());
    change(&state, &id, &dir, &["update-ref", "-m", &reason, "HEAD", &tip, &head]).await
}

/// Squashes a commit and every newer one on this branch, merges included, into one with its author, parents
/// and the tip's tree, so merged-in history flattens and the index and working tree stay as they are.
pub async fn squash(State(state): State<AppState>, Param(id): Param<String>, Json(request): Json<Squash>) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    settled(&status)?;
    let message = written(&request.message)?;
    let (commit, head) = on_branch(&dir, &request.sha).await?;
    if head != request.head {
        return Err(refuse("The branch has new commits. Look them over and squash again."));
    }
    if commit == head {
        return Err(refuse("There are no newer commits to squash."));
    }
    if !request.force {
        unpushed(&dir, &commit).await.map_err(|(_, reason)| (StatusCode::PRECONDITION_REQUIRED, reason))?;
    }
    let tip = squashed(&dir, &commit, &head, message).await.map_err(|output| (StatusCode::BAD_REQUEST, output))?;
    let reason = format!("squash: {}", message.lines().next().unwrap_or_default());
    change(&state, &id, &dir, &["update-ref", "-m", &reason, "HEAD", &tip, &head]).await
}

/// The commits from one on this branch to its tip along first parents, newest first, with what each merge brought in.
pub async fn regroupable(State(state): State<AppState>, Param(id): Param<String>, Query(query): Query<RegroupQuery>) -> Result<Json<Regroupable>, Failure> {
    let (dir, _) = open(&state, &id).await?;
    let (picked, head) = on_branch(&dir, &query.sha).await?;
    let format = "--format=%H%x00%h%x00%s%x00%an%x00%ar%x00%P%x00%B";
    let raw = output(&dir, &["log", "--first-parent", "-z", format, &format!("--max-count={}", REGROUP_LIMIT + 1), &head], None).await.map_err(bad)?;
    let text = String::from_utf8_lossy(&raw);
    let mut fields = text.split('\0');
    let mut listed = Vec::new();
    while let (Some(sha), Some(short), Some(subject), Some(author), Some(date), Some(parents), Some(message)) =
        (fields.next(), fields.next(), fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
    {
        let parents: Vec<String> = parents.split(' ').filter(|parent| !parent.is_empty()).map(str::to_owned).collect();
        let commit = Commit { sha: sha.to_owned(), short: short.to_owned(), subject: subject.to_owned(), author: author.to_owned(), date: date.to_owned() };
        listed.push((commit, parents, message.trim_end().to_owned()));
    }
    let shas: Vec<&str> = listed.iter().map(|(commit, ..)| commit.sha.as_str()).collect();
    let end = reach(&dir, &picked, &head, &shas).await?;
    listed.truncate(end + 1);
    let unpushed = exec(&dir, &["rev-list", "--first-parent", &format!("--max-count={}", end + 1), &head, "--not", "--remotes"]).await.unwrap_or_default();
    let unpushed: HashSet<&str> = unpushed.lines().collect();
    let base = listed.last().and_then(|(_, parents, _)| parents.first().cloned());
    let trunk = trunk(&dir).await;
    let mut commits = Vec::new();
    for (commit, parents, message) in listed {
        let merge = match parents.split_first() {
            Some((first, others)) if !others.is_empty() => Some(merged(&dir, &commit.sha, first, others, trunk.as_deref(), base.as_deref()).await),
            _ => None,
        };
        let pushed = !unpushed.contains(commit.sha.as_str());
        commits.push(Regroupee { commit, message, pushed, merge });
    }
    Ok(Json(Regroupable { head, trunk, commits }))
}

/// Rewrites the commits from `oldest` to the tip into the given runs of consecutive first-parent commits, newest first.
/// A run of several becomes one commit with its newest tree, so nothing replays and the work is untouched; a lone commit keeps its parents.
pub async fn regroup(State(state): State<AppState>, Param(id): Param<String>, Json(request): Json<Regroup>) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    settled(&status)?;
    let (oldest, head) = on_branch(&dir, &request.oldest).await?;
    if head != request.head {
        return Err(refuse("The branch has new commits. Look them over and regroup again."));
    }
    let listed = exec(&dir, &["log", "--first-parent", "--format=%H %P", &format!("--max-count={}", REGROUP_LIMIT + 1), &head]).await.map_err(bad)?;
    let lines: Vec<Vec<&str>> = listed.lines().map(|line| line.split(' ').collect()).collect();
    let shas: Vec<&str> = lines.iter().map(|line| line[0]).collect();
    let end = reach(&dir, &oldest, &head, &shas).await?;
    if shas[end] != oldest {
        return Err((StatusCode::BAD_REQUEST, format!("{oldest} is not on this branch's first-parent line")));
    }
    let mut runs = Vec::new();
    let mut at = 0;
    for (index, group) in request.groups.iter().enumerate() {
        let next = request.groups.get(index + 1).map(|next| shas[..=end].iter().position(|sha| *sha == next.newest));
        let stop = match next {
            Some(Some(stop)) => stop,
            Some(None) => return Err((StatusCode::BAD_REQUEST, format!("{} is not among the commits", request.groups[index + 1].newest))),
            None => end + 1,
        };
        if shas.get(at) != Some(&group.newest.as_str()) || stop <= at {
            return Err((StatusCode::BAD_REQUEST, "Groups must cover every commit, newest first.".to_owned()));
        }
        runs.push((at..stop, group.message.as_deref()));
        at = stop;
    }
    if at != end + 1 {
        return Err((StatusCode::BAD_REQUEST, "Groups must cover every commit, newest first.".to_owned()));
    }
    let base = lines[end].get(1).map(|parent| (*parent).to_owned());
    let trunk = trunk(&dir).await;
    let mut parent = base.clone();
    let mut kept = true;
    for (index, (run, message)) in runs.iter().enumerate().rev() {
        let message = message.map(written).transpose()?;
        let (newest, last) = (shas[run.start], shas[run.end - 1]);
        if run.len() == 1 && kept && message.is_none() {
            parent = Some(newest.to_owned());
            continue;
        }
        if kept && !request.force {
            unpushed(&dir, last).await.map_err(|(_, reason)| (StatusCode::PRECONDITION_REQUIRED, reason))?;
        }
        kept = false;
        let raw = output(&dir, &["cat-file", "commit", last], None).await.map_err(bad)?;
        let object = if run.len() == 1 {
            let original = lines[run.start].get(1).map(|parent| (*parent).to_owned());
            let rewritten = original.zip(parent.clone()).into_iter().collect();
            recommit(&raw, &rewritten, None, message)
        } else {
            let message = message.ok_or_else(|| (StatusCode::BAD_REQUEST, "Write a message for each run of commits.".to_owned()))?;
            let mut onto = parent.clone();
            for line in &lines[run.clone()] {
                let Some((first, others)) = line[1..].split_first().filter(|(_, others)| !others.is_empty()) else { continue };
                let others: Vec<String> = others.iter().map(|other| (*other).to_owned()).collect();
                if let Merged::Sync { foldable, .. } = merged(&dir, line[0], first, &others, trunk.as_deref(), base.as_deref()).await {
                    if index != runs.len() - 1 || !foldable {
                        return Err(refuse("A run with a sync merge must reach the oldest commit."));
                    }
                    if onto == parent {
                        onto = others.first().cloned();
                    }
                }
            }
            let tree = exec(&dir, &["rev-parse", &format!("{newest}^{{tree}}")]).await.map_err(bad)?;
            squashed_onto(&raw, onto.as_deref(), &tree, message)
        };
        parent = Some(store(&dir, &object).await.map_err(bad)?);
    }
    if kept {
        return Err(refuse("Group some commits first."));
    }
    let tip = parent.expect("a rewritten group leaves a commit");
    let reason = format!("regroup: {} commits into {}", end + 1, runs.len());
    change(&state, &id, &dir, &["update-ref", "-m", &reason, "HEAD", &tip, &head]).await
}

/// Where along the first-parent `line` from the tip the commit containing `picked` sits.
async fn reach(dir: &Path, picked: &str, head: &str, line: &[&str]) -> Result<usize, Failure> {
    let end = match line.iter().position(|sha| *sha == picked) {
        Some(end) => end,
        None => {
            let descendants = exec(dir, &["rev-list", "--ancestry-path", &format!("{picked}..{head}")]).await.map_err(bad)?;
            let descendants: HashSet<&str> = descendants.lines().collect();
            line.iter().take_while(|sha| descendants.contains(**sha)).count().saturating_sub(1)
        }
    };
    if end >= REGROUP_LIMIT {
        return Err(refuse(format!("Regroup takes up to {REGROUP_LIMIT} commits. Start from a newer one.")));
    }
    if end == 0 {
        return Err(refuse("There are no newer commits to regroup."));
    }
    Ok(end)
}

/// Classifies a merge: a sync brought in commits already on the trunk; anything else is a side branch.
async fn merged(dir: &Path, merge: &str, first: &str, others: &[String], trunk: Option<&str>, base: Option<&str>) -> Merged {
    let ancestor = async |older: &str, newer: &str| exec(dir, &["merge-base", "--is-ancestor", older, newer]).await.is_ok();
    let brought: Vec<&str> = others.iter().map(String::as_str).chain(["--not", first]).collect();
    let count = async || exec(dir, &[&["rev-list", "--count"][..], &brought].concat()).await.ok().and_then(|count| count.parse::<usize>().ok()).unwrap_or(0);
    let onto = &others[0];
    if let Some(trunk) = trunk
        && !ancestor(merge, trunk).await
        && ancestor(onto, trunk).await
    {
        let foldable = match base {
            Some(base) => ancestor(base, onto).await,
            None => true,
        };
        let short = exec(dir, &["rev-parse", "--short", onto]).await.unwrap_or_else(|_| onto.clone());
        return Merged::Sync { count: count().await, onto: short, foldable };
    }
    let limit = format!("--max-count={}", SIDE_LIMIT + 1);
    let args = [&["log", "-z", "--format=%H%x00%h%x00%s%x00%an%x00%ar", &limit][..], &brought].concat();
    let raw = output(dir, &args, None).await.unwrap_or_default();
    let text = String::from_utf8_lossy(&raw);
    let mut fields = text.split('\0');
    let mut commits = Vec::new();
    while let (Some(sha), Some(short), Some(subject), Some(author), Some(date)) = (fields.next(), fields.next(), fields.next(), fields.next(), fields.next()) {
        commits.push(Commit { sha: sha.to_owned(), short: short.to_owned(), subject: subject.to_owned(), author: author.to_owned(), date: date.to_owned() });
    }
    let more = if commits.len() > SIDE_LIMIT { count().await - SIDE_LIMIT } else { 0 };
    commits.truncate(SIDE_LIMIT);
    Merged::Side { commits, more }
}

/// A commit with `raw`'s author and committer on a single parent, or none, so merged-in history flattens.
fn squashed_onto(raw: &[u8], parent: Option<&str>, tree: &str, message: &str) -> Vec<u8> {
    let split = raw.windows(2).position(|pair| pair == b"\n\n").map_or(raw.len(), |end| end + 1);
    let mut object = format!("tree {tree}\n").into_bytes();
    if let Some(parent) = parent {
        object.extend_from_slice(format!("parent {parent}\n").as_bytes());
    }
    let mut skipping = false;
    for line in raw[..split].split_inclusive(|&byte| byte == b'\n') {
        if !line.starts_with(b" ") {
            skipping = [&b"tree "[..], b"parent ", b"gpgsig ", b"gpgsig-sha256 ", b"mergetag "].iter().any(|header| line.starts_with(header));
        }
        if !skipping {
            object.extend_from_slice(line);
        }
    }
    object.extend_from_slice(format!("\n{message}\n").as_bytes());
    object
}

fn bad(output: String) -> Failure {
    (StatusCode::BAD_REQUEST, output)
}

/// Asks the commit Rig to draft a message for the staged changes.
pub async fn draft(State(state): State<AppState>, Param(id): Param<String>) -> Result<Json<String>, Failure> {
    let (dir, status) = open(&state, &id).await?;
    if status.staged.is_empty() {
        return Err(refuse("Stage changes to draft a message for them."));
    }
    let (rig, instructions) = {
        let config = state.config.borrow();
        let rig = config.commit_rig().cloned().ok_or_else(|| refuse("Add a Rig in Settings first."))?;
        (rig, config.commit_instructions().to_owned())
    };
    let (summary, diff, recent) = tokio::join!(
        exec(&dir, &["diff", "--cached", "--stat"]),
        exec(&dir, &["diff", "--cached", "--no-color", "--no-ext-diff"]),
        exec(&dir, &["log", "-10", "--format=%s"]),
    );
    let mut diff = diff.unwrap_or_default();
    if diff.len() > DIFF_LIMIT {
        diff.truncate(diff.floor_char_boundary(DIFF_LIMIT));
        diff.push_str("\n… diff truncated");
    }
    let prompt = format!(
        "{instructions}\n\n{}\n\nRecent commit subjects:\n{}\n\nStaged changes:\n{}\n\n```diff\n{diff}\n```",
        templates::COMMIT.trim_end(),
        recent.unwrap_or_default(),
        summary.unwrap_or_default(),
    );
    let reply = rigs::ask(&dir, &rig, &prompt).await.map_err(|error| (StatusCode::BAD_GATEWAY, error.to_string()))?;
    Ok(Json(unfenced(&reply)))
}

pub async fn stashes(State(state): State<AppState>, Param(id): Param<String>) -> Result<Json<Vec<Stash>>, Failure> {
    let dir = root(&state, &id)?;
    let output = output(&dir, &["stash", "list", "-z", "--format=%gs%x00%cr"], None).await.unwrap_or_default();
    let mut fields = output.split(|&byte| byte == 0).map(|field| String::from_utf8_lossy(field).into_owned());
    let mut stashes = Vec::new();
    while let (Some(message), Some(date)) = (fields.next(), fields.next()) {
        stashes.push(Stash { message, date });
    }
    Ok(Json(stashes))
}

pub async fn stash(State(state): State<AppState>, Param(id): Param<String>) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    settled(&status)?;
    change(&state, &id, &dir, &["stash", "push", "--include-untracked", "-q"]).await
}

pub async fn unstash(
    State(state): State<AppState>,
    Param((id, index, action)): Param<(String, usize, String)>,
) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    match action.as_str() {
        "pop" | "apply" => settled(&status)?,
        "drop" => {}
        _ => return Err((StatusCode::NOT_FOUND, format!("Stashes cannot {action}"))),
    }
    change(&state, &id, &dir, &["stash", &action, "-q", &format!("stash@{{{index}}}")]).await
}

pub async fn fetch(State(state): State<AppState>, Param(id): Param<String>) -> Result<StatusCode, Failure> {
    let (dir, _) = open(&state, &id).await?;
    change(&state, &id, &dir, &["fetch", "-q"]).await
}

pub async fn pull(State(state): State<AppState>, Param(id): Param<String>) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    settled(&status)?;
    if status.upstream.is_none() {
        return Err(refuse("This branch has no upstream to pull from."));
    }
    change(&state, &id, &dir, &["pull", "-q", "--no-rebase", "--ff-only"]).await
}

pub async fn push(State(state): State<AppState>, Param(id): Param<String>) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    if status.upstream.is_none() {
        return Err(refuse("Publish this branch first."));
    }
    change(&state, &id, &dir, &["push", "-q"]).await
}

pub async fn publish(State(state): State<AppState>, Param(id): Param<String>) -> Result<StatusCode, Failure> {
    let (dir, status) = open(&state, &id).await?;
    let Some(branch) = status.branch.as_deref().filter(|_| !status.detached) else {
        return Err(refuse("Check out a branch to publish."));
    };
    if !status.origin {
        return Err(refuse("This repository has no origin remote."));
    }
    change(&state, &id, &dir, &["push", "-q", "-u", "origin", branch]).await
}

/// Fast-forwards the default branch to origin's without leaving the current one, pulling wherever it is checked out.
pub async fn pull_trunk(State(state): State<AppState>, Param(id): Param<String>) -> Result<StatusCode, Failure> {
    let dir = root(&state, &id)?;
    let branch = origin::default_branch(&dir).await.ok_or_else(|| refuse("origin has no default branch."))?;
    let worktrees = run_alone(&dir, &["worktree", "list", "--porcelain"]).await.map_err(|output| (StatusCode::BAD_REQUEST, output))?;
    let line = format!("branch refs/heads/{branch}");
    let checkout = worktrees.split("\n\n").find(|block| block.lines().any(|entry| entry == line)).and_then(|block| block.lines().next()?.strip_prefix("worktree "));
    let result = match checkout {
        Some(checkout) => {
            let checkout = Path::new(checkout);
            settled(&read(checkout).await.unwrap_or_default())?;
            exec(checkout, &["pull", "-q", "--no-rebase", "--ff-only", "origin", &branch]).await
        }
        None => exec(&dir, &["fetch", "-q", "origin", &format!("{branch}:{branch}")]).await,
    };
    let common = common_dir(&dir).await;
    for project in state.projects.list().iter() {
        if project.id == id || common_dir(Path::new(&project.path)).await == common {
            changed(&state, &project.id);
        }
    }
    result.map(|_| StatusCode::NO_CONTENT).map_err(|output| (StatusCode::BAD_REQUEST, output))
}

/// Checks out a local branch, or a remote one as the local branch of its name, tracking it when new.
pub async fn switch(State(state): State<AppState>, Param(id): Param<String>, Json(Branch { branch }): Json<Branch>) -> Result<StatusCode, Failure> {
    let dir = movable(&state, &id).await?;
    if exists(&dir, "heads", &branch).await {
        return change(&state, &id, &dir, &["switch", "-q", &branch]).await;
    }
    let local = match branch.split_once('/') {
        Some((_, local)) if local != "HEAD" && exists(&dir, "remotes", &branch).await => local,
        _ => return Err((StatusCode::NOT_FOUND, format!("No branch {branch}"))),
    };
    if exists(&dir, "heads", local).await {
        return change(&state, &id, &dir, &["switch", "-q", local]).await;
    }
    change(&state, &id, &dir, &["switch", "-q", "-c", local, "--track", &branch]).await
}

pub async fn branch(State(state): State<AppState>, Param(id): Param<String>, Json(Branch { branch }): Json<Branch>) -> Result<StatusCode, Failure> {
    let dir = movable(&state, &id).await?;
    change(&state, &id, &dir, &["switch", "-q", "-c", &branch]).await
}

async fn movable(state: &AppState, id: &str) -> Result<PathBuf, Failure> {
    if state.projects.list().iter().any(|project| project.id == id && project.worktree.is_some()) {
        return Err(refuse("A worktree stays on its own branch."));
    }
    let (dir, status) = open(state, id).await?;
    settled(&status)?;
    Ok(dir)
}

async fn open(state: &AppState, id: &str) -> Result<(PathBuf, Status), Failure> {
    let dir = root(state, id)?;
    let status = read(&dir).await.ok_or_else(|| (StatusCode::BAD_REQUEST, "This Project is not a git repository.".to_owned()))?;
    Ok((dir, status))
}

fn changed(state: &AppState, id: &str) {
    state.git.reread(id);
}

/// Runs git to change the repository, then has every Device refresh, whether or not it succeeded.
async fn change(state: &AppState, id: &str, dir: &Path, args: &[&str]) -> Result<StatusCode, Failure> {
    let result = exec(dir, args).await;
    changed(state, id);
    result.map(|_| StatusCode::NO_CONTENT).map_err(|output| (StatusCode::BAD_REQUEST, output))
}

/// Runs git to change the repository, streaming each line it prints, then has every Device refresh.
fn streamed(state: AppState, id: String, mut command: Command) -> Response {
    let (sender, receiver) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        let ok = match command.spawn() {
            Ok(mut child) => {
                let (stdout, stderr) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
                tokio::join!(relay(stdout, &sender), relay(stderr, &sender));
                child.wait().await.is_ok_and(|status| status.success())
            }
            Err(error) => {
                let _ = sender.send(json!({ "output": format!("cannot run git: {error}") }).to_string());
                false
            }
        };
        changed(&state, &id);
        let _ = sender.send(json!({ "ok": ok }).to_string());
    });
    let lines = futures_util::stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|line| (Ok::<_, Infallible>(format!("{line}\n")), receiver))
    });
    ([(header::CONTENT_TYPE, "application/x-ndjson")], Body::from_stream(lines)).into_response()
}

async fn relay(reader: impl AsyncRead + Unpin, sender: &mpsc::UnboundedSender<String>) {
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    while reader.read_until(b'\n', &mut line).await.is_ok_and(|read| read > 0) {
        let _ = sender.send(json!({ "output": String::from_utf8_lossy(&line) }).to_string());
        line.clear();
    }
}

fn refuse(reason: impl Into<String>) -> Failure {
    (StatusCode::CONFLICT, reason.into())
}

fn written(message: &str) -> Result<&str, Failure> {
    let message = message.trim_end();
    if message.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Write a commit message first.".to_owned()));
    }
    Ok(message)
}

fn chosen(paths: &[String]) -> Result<Vec<&str>, Failure> {
    if paths.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Choose files first.".to_owned()));
    }
    for path in paths {
        inside(path)?;
    }
    Ok(paths.iter().map(String::as_str).collect())
}

pub(crate) fn settled(status: &Status) -> Result<(), Failure> {
    let operation = match status.operation {
        Some(Operation::Merge) => "A merge",
        Some(Operation::Rebase) => "A rebase",
        Some(Operation::CherryPick) => "A cherry-pick",
        Some(Operation::Revert) => "A revert",
        None if status.conflicted.is_empty() => return Ok(()),
        None => return Err(refuse("Resolve the merge conflicts first.")),
    };
    Err(refuse(format!("{operation} is in progress. Finish or abort it first.")))
}

fn unconflicted(status: &Status, paths: &[&str]) -> Result<(), Failure> {
    match status.conflicted.iter().find(|conflict| paths.contains(&conflict.path.as_str())) {
        Some(conflict) => Err(refuse(format!("{} has merge conflicts. Resolve them in the file, then stage it.", conflict.path))),
        None => Ok(()),
    }
}

async fn exists(dir: &Path, kind: &str, name: &str) -> bool {
    !name.starts_with('-') && exec(dir, &["show-ref", "--verify", "-q", &format!("refs/{kind}/{name}")]).await.is_ok()
}

async fn marked(path: &Path) -> bool {
    let Ok(content) = tokio::fs::read(path).await else { return false };
    String::from_utf8_lossy(&content).lines().any(|line| line.starts_with("<<<<<<<") || line.starts_with(">>>>>>>"))
}

/// Resolves a commit on this branch, along with the branch's tip.
async fn on_branch(dir: &Path, text: &str) -> Result<(String, String), Failure> {
    let short = sha(text).ok_or_else(|| (StatusCode::BAD_REQUEST, format!("{text} is not a sha")))?;
    let commit = exec(dir, &["rev-parse", "--verify", "-q", &format!("{short}^{{commit}}")])
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, format!("No commit {short}")))?;
    let head = exec(dir, &["rev-parse", "--verify", "-q", "HEAD"]).await.map_err(|_| refuse("There are no commits yet."))?;
    if exec(dir, &["merge-base", "--is-ancestor", &commit, &head]).await.is_err() {
        return Err(refuse("That commit is not on this branch."));
    }
    Ok((commit, head))
}

async fn unpushed(dir: &Path, commit: &str) -> Result<(), Failure> {
    let remotes = exec(dir, &["for-each-ref", "--contains", commit, "--format=%(refname)", "refs/remotes"]).await.unwrap_or_default();
    match remotes.lines().filter(|remote| !remote.ends_with("/HEAD")).find_map(|remote| remote.strip_prefix("refs/remotes/")) {
        Some(remote) => Err(refuse(format!("This commit is already on {remote}. Changing it would need a force push."))),
        None => Ok(()),
    }
}

async fn rewrite(dir: &Path, commit: &str, head: &str, message: &str) -> Result<String, String> {
    let descendants = exec(dir, &["rev-list", "--reverse", "--topo-order", "--ancestry-path", &format!("{commit}..{head}")]).await?;
    let mut rewritten = HashMap::new();
    for (index, original) in std::iter::once(commit).chain(descendants.lines()).enumerate() {
        let raw = output(dir, &["cat-file", "commit", original], None).await?;
        let object = recommit(&raw, &rewritten, None, (index == 0).then_some(message));
        rewritten.insert(original.to_owned(), store(dir, &object).await?);
    }
    rewritten.remove(head).ok_or_else(|| "The branch changed while rewording.".to_owned())
}

async fn squashed(dir: &Path, commit: &str, head: &str, message: &str) -> Result<String, String> {
    let tree = exec(dir, &["rev-parse", &format!("{head}^{{tree}}")]).await?;
    let raw = output(dir, &["cat-file", "commit", commit], None).await?;
    store(dir, &recommit(&raw, &HashMap::new(), Some(&tree), Some(message))).await
}

async fn store(dir: &Path, object: &[u8]) -> Result<String, String> {
    let sha = output(dir, &["hash-object", "-t", "commit", "-w", "--stdin"], Some(object)).await?;
    Ok(String::from_utf8_lossy(&sha).trim().to_owned())
}

/// A raw commit object with rewritten parents and, when given, a new tree and message. Signatures no longer match, so they go.
fn recommit(raw: &[u8], rewritten: &HashMap<String, String>, tree: Option<&str>, message: Option<&str>) -> Vec<u8> {
    let split = raw.windows(2).position(|pair| pair == b"\n\n").map_or(raw.len(), |end| end + 1);
    let (headers, body) = raw.split_at(split);
    let mut object = Vec::new();
    let mut signature = false;
    for line in headers.split_inclusive(|&byte| byte == b'\n') {
        if line.starts_with(b" ") {
            if !signature {
                object.extend_from_slice(line);
            }
            continue;
        }
        signature = line.starts_with(b"gpgsig ") || line.starts_with(b"gpgsig-sha256 ");
        if signature {
            continue;
        }
        let parent = line.strip_prefix(b"parent ").and_then(|parent| rewritten.get(String::from_utf8_lossy(parent).trim_end()));
        match (parent, tree.filter(|_| line.starts_with(b"tree "))) {
            (Some(parent), _) => object.extend_from_slice(format!("parent {parent}\n").as_bytes()),
            (_, Some(tree)) => object.extend_from_slice(format!("tree {tree}\n").as_bytes()),
            _ => object.extend_from_slice(line),
        }
    }
    match message {
        Some(message) => object.extend_from_slice(format!("\n{message}\n").as_bytes()),
        None => object.extend_from_slice(body),
    }
    object
}

fn unfenced(text: &str) -> String {
    let text = text.trim();
    let fenced = text.strip_prefix("```").and_then(|inner| inner.strip_suffix("```"));
    fenced.map_or(text, |inner| inner.split_once('\n').map_or(inner, |(_, body)| body)).trim().to_owned()
}

async fn exec(dir: &Path, args: &[&str]) -> Result<String, String> {
    output(dir, args, None).await.map(|stdout| String::from_utf8_lossy(&stdout).trim().to_owned())
}

fn git(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Without a controlling terminal, ssh fails at a passphrase or host key prompt instead of waiting on it forever.
    unsafe {
        command.pre_exec(|| nix::unistd::setsid().map(drop).map_err(std::io::Error::from));
    }
    command
}

/// Runs git, giving its stdout, or everything it printed when it failed.
async fn output(dir: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut command = git(dir, args);
    command.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = command.spawn().map_err(|error| format!("cannot run git: {error}"))?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(input).await.map_err(|error| format!("cannot write to git: {error}"))?;
    }
    let output = child.wait_with_output().await.map_err(|error| format!("git {} failed: {error}", args.join(" ")))?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let printed: Vec<_> = [&output.stdout, &output.stderr]
        .map(|bytes| String::from_utf8_lossy(bytes).trim().to_owned())
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect();
    Err(if printed.is_empty() { format!("git {} failed", args.join(" ")) } else { printed.join("\n") })
}
