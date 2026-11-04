use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use axum::{
    Json,
    extract::{Path as Param, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{process::Command, sync::broadcast, time::timeout};
use uuid::Uuid;

use crate::{
    AppState,
    files::{Failure, root},
    git,
    github::{self, Availability, Lookup, Repository},
};

const LIMIT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrReview {
    number: u64,
    title: String,
    url: String,
    author: String,
    mine: bool,
    state: PrState,
    target: Target,
    source: Source,
    merge_base: String,
    viewed: BTreeMap<String, Viewed>,
    drafts: Vec<DraftComment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DraftComment {
    id: String,
    path: String,
    side: Side,
    line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_line: Option<u32>,
    body: String,
    commit: String,
    displaced: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Side {
    Target,
    Source,
}

/// The 1-based inclusive line ranges of a file's hunks, which GitHub accepts comments on, per side.
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct Hunks {
    source: Vec<(u32, u32)>,
    target: Vec<(u32, u32)>,
}

impl Hunks {
    /// Whether `start..=end` lies within one hunk of `side`.
    fn contain(&self, side: Side, start: u32, end: u32) -> bool {
        let ranges = if side == Side::Source { &self.source } else { &self.target };
        start <= end && ranges.iter().any(|&(from, to)| from <= start && end <= to)
    }
}

/// One hunk header's old and new `(start, count)`. A zero count's start is the line the hunk follows.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hunk {
    old: (u32, u32),
    new: (u32, u32),
}

/// Reads the hunk headers (`@@ -a,b +c,d @@`) of one file's unified diff.
fn parse(diff: &str) -> Vec<Hunk> {
    let range = |range: &str| {
        let (start, count) = range.split_once(',').unwrap_or((range, "1"));
        Some((start.parse().ok()?, count.parse().ok()?))
    };
    diff.lines()
        .filter_map(|line| line.strip_prefix("@@ -"))
        .filter_map(|header| {
            let (old, rest) = header.split_once(" +")?;
            let (new, _) = rest.split_once(" @@")?;
            Some(Hunk { old: range(old)?, new: range(new)? })
        })
        .collect()
}

fn hunks(diff: &str) -> Hunks {
    let mut found = Hunks::default();
    for Hunk { old, new } in parse(diff) {
        for ((start, count), ranges) in [(old, &mut found.target), (new, &mut found.source)] {
            if count > 0 {
                ranges.push((start, start + count - 1));
            }
        }
    }
    found
}

/// Maps an inclusive 1-based line range through the hunks of `git diff -U0 old new`.
/// Returns None if any line in the range lies inside a changed hunk, or lines were inserted within it.
fn map_through(hunks: &[Hunk], start: u32, end: u32) -> Option<(u32, u32)> {
    let mut shift = 0i64;
    for &Hunk { old: (at, removed), new: (_, added) } in hunks {
        if removed == 0 {
            if at < start {
                shift += i64::from(added);
            } else if at < end {
                return None;
            }
        } else if at + removed - 1 < start {
            shift += i64::from(added) - i64::from(removed);
        } else if at <= end {
            return None;
        }
    }
    let moved = |line: u32| u32::try_from(i64::from(line) + shift).ok();
    Some((moved(start)?, moved(end)?))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PrState {
    Open,
    Merged,
    Closed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Target {
    branch: String,
    commit: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Source {
    commit: String,
}

/// The Source commit a file was Viewed at and its blob there, empty for a file the PR deletes.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Viewed {
    commit: String,
    blob: String,
}

/// PR review records, one JSON file per pull request under a directory per repository, so every worktree of a
/// repository shares them. Changes broadcast the repository's git common dir.
pub struct PrReviews {
    dir: PathBuf,
    changed: broadcast::Sender<PathBuf>,
    fetching: tokio::sync::Mutex<()>,
}

impl PrReviews {
    pub fn load(data_dir: &Path) -> io::Result<Self> {
        let dir = data_dir.join("pr-reviews");
        fs::create_dir_all(&dir)?;
        Ok(Self { dir, changed: broadcast::channel(64).0, fetching: tokio::sync::Mutex::default() })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PathBuf> {
        self.changed.subscribe()
    }

    fn repository(&self, common_dir: &Path) -> PathBuf {
        let hash: String = Sha256::digest(common_dir.as_os_str().as_encoded_bytes()).iter().take(8).map(|byte| format!("{byte:02x}")).collect();
        self.dir.join(hash)
    }

    pub fn list(&self, common_dir: &Path) -> Vec<PrReview> {
        let Ok(entries) = fs::read_dir(self.repository(common_dir)) else { return Vec::new() };
        let mut reviews: Vec<PrReview> = entries
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "json"))
            .filter_map(|entry| serde_json::from_slice(&fs::read(entry.path()).ok()?).ok())
            .collect();
        reviews.sort_by_key(|review| review.number);
        reviews
    }

    fn get(&self, common_dir: &Path, number: u64) -> Option<PrReview> {
        serde_json::from_slice(&fs::read(self.repository(common_dir).join(format!("{number}.json"))).ok()?).ok()
    }

    fn save(&self, common_dir: &Path, review: &PrReview) -> Result<(), Failure> {
        let dir = self.repository(common_dir);
        let file = dir.join(format!("{}.json", review.number));
        let tmp = dir.join(format!("{}.{}.tmp", review.number, Uuid::new_v4()));
        let saved = fs::create_dir_all(&dir)
            .and_then(|()| fs::write(&tmp, serde_json::to_vec_pretty(review).map_err(io::Error::other)?))
            .and_then(|()| fs::rename(&tmp, &file));
        saved.map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, format!("cannot save {}: {error}", file.display())))?;
        let _ = self.changed.send(common_dir.to_owned());
        Ok(())
    }

    fn remove(&self, common_dir: &Path, number: u64) -> Result<(), Failure> {
        let file = self.repository(common_dir).join(format!("{number}.json"));
        match fs::remove_file(&file) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Err((StatusCode::NOT_FOUND, format!("No PR review of #{number}"))),
            Err(error) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("cannot remove {}: {error}", file.display()))),
        }
        let _ = self.changed.send(common_dir.to_owned());
        Ok(())
    }
}

async fn located(state: &AppState, id: &str) -> Result<(PathBuf, PathBuf), Failure> {
    let dir = root(state, id)?;
    let common_dir = git::common_dir(&dir).await.ok_or_else(|| (StatusCode::BAD_REQUEST, "the Project is not a git repository".to_owned()))?;
    Ok((dir, common_dir))
}

async fn repository(dir: &Path) -> Result<Repository, Failure> {
    match github::resolve(dir).await {
        Availability::Available(repository) => Ok(repository),
        Availability::Unavailable(reason) => Err((StatusCode::BAD_REQUEST, reason)),
        Availability::Hidden => Err((StatusCode::BAD_REQUEST, "the Project has no remote on the repository gh resolves".to_owned())),
    }
}

pub async fn prs(State(state): State<AppState>, Param(id): Param<String>) -> Result<Json<Value>, Failure> {
    let dir = root(&state, &id)?;
    Ok(Json(match github::resolve(&dir).await {
        Availability::Available(_) => {
            let prs = github::prs(&dir).await.map_err(|error| (StatusCode::BAD_GATEWAY, error))?;
            json!({ "available": true, "prs": prs })
        }
        Availability::Unavailable(reason) => json!({ "available": false, "hidden": false, "reason": reason, "prs": [] }),
        Availability::Hidden => json!({ "available": false, "hidden": true, "prs": [] }),
    }))
}

pub async fn list(State(state): State<AppState>, Param(id): Param<String>) -> Result<Json<Vec<PrReview>>, Failure> {
    let (_, common_dir) = located(&state, &id).await?;
    Ok(Json(state.pr_reviews.list(&common_dir)))
}

#[derive(Deserialize)]
pub struct Opening {
    pr: Requested,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Requested {
    Number(u64),
    Text(String),
}

pub async fn open(State(state): State<AppState>, Param(id): Param<String>, Json(opening): Json<Opening>) -> Result<Json<PrReview>, Failure> {
    let (dir, common_dir) = located(&state, &id).await?;
    let repository = repository(&dir).await?;
    let number = match opening.pr {
        Requested::Number(number) => number,
        Requested::Text(text) => match text.trim().trim_start_matches('#').parse() {
            Ok(number) => number,
            Err(_) => named(&text, &repository)?,
        },
    };
    fetch(&state, &dir, &common_dir, &repository, number).await.map(Json)
}

/// The number a pull request URL gives, when it names this repository.
fn named(url: &str, repository: &Repository) -> Result<u64, Failure> {
    let refused = || {
        let Repository { host, owner, name, .. } = repository;
        (StatusCode::BAD_REQUEST, format!("{url} is not a pull request of {host}/{owner}/{name}"))
    };
    let rest = url.trim().strip_prefix("https://").or_else(|| url.trim().strip_prefix("http://")).ok_or_else(refused)?;
    let mut parts = rest.split(['/', '?', '#']);
    let (host, owner, name, pull, number) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next());
    let same = host.is_some_and(|host| host.eq_ignore_ascii_case(&repository.host))
        && owner.is_some_and(|owner| owner.eq_ignore_ascii_case(&repository.owner))
        && name.is_some_and(|name| name.eq_ignore_ascii_case(&repository.name))
        && pull == Some("pull");
    number.filter(|_| same).and_then(|number| number.parse().ok()).ok_or_else(refused)
}

pub async fn refresh(State(state): State<AppState>, Param((id, number)): Param<(String, u64)>) -> Result<Json<PrReview>, Failure> {
    let (dir, common_dir) = located(&state, &id).await?;
    if state.pr_reviews.get(&common_dir, number).is_none() {
        return Err((StatusCode::NOT_FOUND, format!("No PR review of #{number}")));
    }
    let repository = repository(&dir).await?;
    fetch(&state, &dir, &common_dir, &repository, number).await.map(Json)
}

pub async fn discard(State(state): State<AppState>, Param((id, number)): Param<(String, u64)>) -> Result<StatusCode, Failure> {
    let (dir, common_dir) = located(&state, &id).await?;
    let _fetching = state.pr_reviews.fetching.lock().await;
    state.pr_reviews.remove(&common_dir, number)?;
    let prefix = format!("refs/stampede/pr/{number}/");
    let refs = git::run(&dir, &["for-each-ref", "--format=%(refname)", &prefix]).await.unwrap_or_default();
    for name in refs.lines() {
        git::run(&dir, &["update-ref", "-d", name]).await.map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Viewing {
    path: String,
    viewed: bool,
}

pub async fn view(State(state): State<AppState>, Param((id, number)): Param<(String, u64)>, Json(viewing): Json<Viewing>) -> Result<Json<PrReview>, Failure> {
    let (dir, common_dir) = located(&state, &id).await?;
    git::inside(&viewing.path)?;
    let _fetching = state.pr_reviews.fetching.lock().await;
    let mut review = state.pr_reviews.get(&common_dir, number).ok_or_else(|| (StatusCode::NOT_FOUND, format!("No PR review of #{number}")))?;
    if !viewing.viewed {
        review.viewed.remove(&viewing.path);
    } else {
        let commit = review.source.commit.clone();
        let object = async |rev: &str| git::run(&dir, &["rev-parse", "--verify", "--quiet", &format!("{rev}:./{}", viewing.path)]).await.ok();
        let blob = match object(&commit).await {
            Some(blob) => blob,
            None if object(&review.merge_base).await.is_some() => String::new(),
            None => return Err((StatusCode::NOT_FOUND, format!("#{number} has no {}", viewing.path))),
        };
        let kept = format!("refs/stampede/pr/{number}/viewed/{commit}");
        git::run(&dir, &["update-ref", &kept, &commit]).await.map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
        review.viewed.insert(viewing.path, Viewed { commit, blob });
    }
    state.pr_reviews.save(&common_dir, &review)?;
    Ok(Json(review))
}

pub async fn worktree(State(state): State<AppState>, Param((id, number)): Param<(String, u64)>) -> Result<Json<Value>, Failure> {
    let (dir, _) = located(&state, &id).await?;
    let source = format!("refs/stampede/pr/{number}/source^{{commit}}");
    let commit = git::run(&dir, &["rev-parse", "--verify", "--quiet", &source]).await.map_err(|_| (StatusCode::NOT_FOUND, format!("No PR review of #{number}")))?;
    let project_id = state.runs.detach_worktree(&id, &format!("pr-{number}"), &commit).await.map_err(|error| (StatusCode::BAD_REQUEST, error))?;
    Ok(Json(json!({ "projectId": project_id })))
}

/// Fetches the pull request into its hidden refs and records what they now hold, creating the record when new.
async fn fetch(state: &AppState, dir: &Path, common_dir: &Path, repository: &Repository, number: u64) -> Result<PrReview, Failure> {
    let pull = github::pull(dir, number).await.map_err(|lookup| match lookup {
        Lookup::Unknown(message) => (StatusCode::NOT_FOUND, message),
        Lookup::Failed(message) => (StatusCode::BAD_GATEWAY, message),
    })?;
    let _fetching = state.pr_reviews.fetching.lock().await;
    let existing = state.pr_reviews.get(common_dir, number);
    let mine = match &existing {
        Some(review) => review.mine,
        None => github::login(dir, &repository.host).await.map_err(|error| (StatusCode::BAD_GATEWAY, error))? == pull.author.login,
    };
    let (source, target) = (format!("refs/stampede/pr/{number}/source"), format!("refs/stampede/pr/{number}/target"));
    let specs = [format!("+refs/pull/{number}/head:{source}"), format!("+refs/heads/{}:{target}", pull.base_ref_name)];
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(["fetch", "--quiet", "--no-tags", &repository.remote])
        .args(&specs)
        // An empty GIT_ASKPASS also stops git falling back to SSH_ASKPASS.
        .envs([("GIT_TERMINAL_PROMPT", "0"), ("GIT_ASKPASS", ""), ("SSH_ASKPASS_REQUIRE", "never"), ("GCM_INTERACTIVE", "never")])
        .stdin(Stdio::null())
        .kill_on_drop(true);
    match timeout(LIMIT, command.output()).await {
        Err(_) => return Err((StatusCode::GATEWAY_TIMEOUT, format!("fetching #{number} from {} timed out", repository.remote))),
        Ok(Err(error)) => return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("cannot run git: {error}"))),
        Ok(Ok(output)) if !output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err((StatusCode::BAD_GATEWAY, format!("cannot fetch #{number} from {}: {stderr}", repository.remote)));
        }
        Ok(Ok(_)) => {}
    }
    let failed = |error| (StatusCode::INTERNAL_SERVER_ERROR, error);
    let source_commit = git::run(dir, &["rev-parse", "--verify", &format!("{source}^{{commit}}")]).await.map_err(failed)?;
    let target_commit = git::run(dir, &["rev-parse", "--verify", &format!("{target}^{{commit}}")]).await.map_err(failed)?;
    let merge_base = git::run(dir, &["merge-base", &target_commit, &source_commit]).await.map_err(failed)?;
    let state_of = match pull.state.as_str() {
        "MERGED" => PrState::Merged,
        "CLOSED" => PrState::Closed,
        _ => PrState::Open,
    };
    let before = existing.as_ref().map(|review| (review.source.commit.clone(), review.merge_base.clone()));
    let (viewed, drafts) = existing.map(|review| (review.viewed, review.drafts)).unwrap_or_default();
    let mut review = PrReview {
        number,
        title: pull.title,
        url: pull.url,
        author: pull.author.login,
        mine,
        state: state_of,
        target: Target { branch: pull.base_ref_name, commit: target_commit },
        source: Source { commit: source_commit },
        merge_base,
        viewed,
        drafts,
    };
    if let Some((source, base)) = before.filter(|(source, base)| *source != review.source.commit || *base != review.merge_base) {
        remap(dir, &source, &base, &mut review).await?;
    }
    state.pr_reviews.save(common_dir, &review)?;
    Ok(review)
}

#[derive(Deserialize)]
pub struct PathQuery {
    path: String,
}

pub async fn commentable(
    State(state): State<AppState>,
    Param((id, number)): Param<(String, u64)>,
    Query(query): Query<PathQuery>,
) -> Result<Json<Hunks>, Failure> {
    let (dir, common_dir) = located(&state, &id).await?;
    let review = state.pr_reviews.get(&common_dir, number).ok_or_else(|| missing(number))?;
    file_hunks(&dir, &review, &query.path).await.map(Json)
}

/// The hunks of `git diff -U3 <mergeBase> <source>` for `path`, a rename's Target side being its old path.
async fn file_hunks(dir: &Path, review: &PrReview, path: &str) -> Result<Hunks, Failure> {
    let from = renames(dir, &review.merge_base, &review.source.commit).await?.remove(path);
    comparison_hunks(dir, review, path, from.as_deref()).await
}

async fn comparison_hunks(dir: &Path, review: &PrReview, path: &str, from: Option<&str>) -> Result<Hunks, Failure> {
    let (base, source) = (review.merge_base.as_str(), review.source.commit.as_str());
    let mut args = vec!["--literal-pathspecs", "diff", "-U3", "--no-color", "--no-ext-diff", "--find-renames", "--relative", base, source, "--", path];
    args.extend(from);
    Ok(hunks(&git::run(dir, &args).await.map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?))
}

/// Each renamed path of the Comparison `base...source`, with its path at `base`.
async fn renames(dir: &Path, base: &str, source: &str) -> Result<BTreeMap<String, String>, Failure> {
    let names = git::run(dir, &["diff", "-z", "--name-status", "--find-renames", "--relative", base, source, "--"])
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
    let changes = git::name_status(names.as_bytes()).unwrap_or_default();
    Ok(changes.into_iter().filter_map(|change| Some((change.path, change.from?))).collect())
}

/// Moves each placed draft of `review` from the Source commit and merge base it was refreshed from to where its
/// lines are now. A draft whose lines changed, whose file is gone, or which no longer lies within one hunk becomes
/// Displaced.
async fn remap(dir: &Path, old_source: &str, old_base: &str, review: &mut PrReview) -> Result<(), Failure> {
    let mut drafts = std::mem::take(&mut review.drafts);
    let mut placed: BTreeMap<(String, Side), Vec<&mut DraftComment>> = BTreeMap::new();
    for draft in drafts.iter_mut().filter(|draft| !draft.displaced) {
        placed.entry((draft.path.clone(), draft.side)).or_default().push(draft);
    }
    if !placed.is_empty() {
        let was = renames(dir, old_base, old_source).await?;
        let now = renames(dir, &review.merge_base, &review.source.commit).await?;
        let mut commentable = BTreeMap::new();
        for ((path, side), group) in placed {
            let (before, after) = match side {
                Side::Source => (format!("{old_source}:./{path}"), format!("{}:./{path}", review.source.commit)),
                Side::Target => {
                    let named = |renamed: &BTreeMap<String, String>| renamed.get(&path).cloned().unwrap_or_else(|| path.clone());
                    (format!("{old_base}:./{}", named(&was)), format!("{}:./{}", review.merge_base, named(&now)))
                }
            };
            let changed = if before == after {
                Some(Vec::new())
            } else {
                git::run(dir, &["diff", "-U0", "--no-color", "--no-ext-diff", &before, &after]).await.ok().map(|diff| parse(&diff))
            };
            if !commentable.contains_key(&path) {
                commentable.insert(path.clone(), comparison_hunks(dir, review, &path, now.get(&path).map(String::as_str)).await?);
            }
            let hunks = &commentable[&path];
            for draft in group {
                let start = draft.start_line.unwrap_or(draft.line);
                match changed.as_deref().and_then(|changed| map_through(changed, start, draft.line)) {
                    Some((start, line)) if hunks.contain(side, start, line) => {
                        draft.line = line;
                        draft.start_line = draft.start_line.map(|_| start);
                        draft.commit = review.source.commit.clone();
                    }
                    _ => draft.displaced = true,
                }
            }
        }
    }
    review.drafts = drafts;
    Ok(())
}

fn missing(number: u64) -> Failure {
    (StatusCode::NOT_FOUND, format!("No PR review of #{number}"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDraft {
    path: String,
    side: Side,
    line: u32,
    start_line: Option<u32>,
    body: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftEdit {
    body: Option<String>,
    side: Option<Side>,
    line: Option<u32>,
    start_line: Option<u32>,
}

/// Applies `change` to PR review `number` under the fetch lock, so a concurrent fetch never loses it.
async fn change_review(
    state: &AppState,
    id: &str,
    number: u64,
    change: impl AsyncFnOnce(&Path, &mut PrReview) -> Result<(), Failure>,
) -> Result<Json<PrReview>, Failure> {
    let (dir, common_dir) = located(state, id).await?;
    let _fetching = state.pr_reviews.fetching.lock().await;
    let mut review = state.pr_reviews.get(&common_dir, number).ok_or_else(|| missing(number))?;
    change(&dir, &mut review).await?;
    state.pr_reviews.save(&common_dir, &review)?;
    Ok(Json(review))
}

/// Checks a draft's lines lie within one hunk of its side, giving its `startLine` with a single line's dropped.
async fn placed(dir: &Path, review: &PrReview, path: &str, side: Side, line: u32, start_line: Option<u32>) -> Result<Option<u32>, Failure> {
    let start = start_line.unwrap_or(line);
    if start == 0 || !file_hunks(dir, review, path).await?.contain(side, start, line) {
        return Err((StatusCode::BAD_REQUEST, format!("{path} lines {start}-{line} are not within one hunk")));
    }
    Ok(start_line.filter(|&start| start < line))
}

fn written(body: String) -> Result<String, Failure> {
    if body.trim().is_empty() { Err((StatusCode::BAD_REQUEST, "a Draft comment needs a body".to_owned())) } else { Ok(body) }
}

pub async fn create_draft(
    State(state): State<AppState>,
    Param((id, number)): Param<(String, u64)>,
    Json(draft): Json<NewDraft>,
) -> Result<Json<PrReview>, Failure> {
    change_review(&state, &id, number, async |dir, review| {
        let start_line = placed(dir, review, &draft.path, draft.side, draft.line, draft.start_line).await?;
        review.drafts.push(DraftComment {
            id: Uuid::new_v4().to_string(),
            path: draft.path,
            side: draft.side,
            line: draft.line,
            start_line,
            body: written(draft.body)?,
            commit: review.source.commit.clone(),
            displaced: false,
        });
        Ok(())
    })
    .await
}

pub async fn edit_draft(
    State(state): State<AppState>,
    Param((id, number, draft_id)): Param<(String, u64, String)>,
    Json(edit): Json<DraftEdit>,
) -> Result<Json<PrReview>, Failure> {
    change_review(&state, &id, number, async |dir, review| {
        let at = review.drafts.iter().position(|draft| draft.id == draft_id).ok_or_else(|| (StatusCode::NOT_FOUND, format!("No Draft comment {draft_id}")))?;
        let mut edited = review.drafts[at].clone();
        if let Some(body) = edit.body {
            edited.body = written(body)?;
        }
        if edit.side.is_some() || edit.line.is_some() || edit.start_line.is_some() {
            let start_line = if edit.line.is_some() { edit.start_line } else { edit.start_line.or(edited.start_line) };
            edited.side = edit.side.unwrap_or(edited.side);
            edited.line = edit.line.unwrap_or(edited.line);
            edited.start_line = placed(dir, review, &edited.path, edited.side, edited.line, start_line).await?;
            edited.commit = review.source.commit.clone();
            edited.displaced = false;
        }
        review.drafts[at] = edited;
        Ok(())
    })
    .await
}

pub async fn delete_draft(State(state): State<AppState>, Param((id, number, draft_id)): Param<(String, u64, String)>) -> Result<Json<PrReview>, Failure> {
    change_review(&state, &id, number, async |_, review| {
        let before = review.drafts.len();
        review.drafts.retain(|draft| draft.id != draft_id);
        if review.drafts.len() == before { Err((StatusCode::NOT_FOUND, format!("No Draft comment {draft_id}"))) } else { Ok(()) }
    })
    .await
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Event {
    Comment,
    Approve,
    RequestChanges,
}

#[derive(Deserialize)]
pub struct Sending {
    body: String,
    event: Event,
}

/// The one GitHub review that carries every draft, with `side` as GitHub names it.
fn payload(review: &PrReview, sending: &Sending) -> Value {
    let comments: Vec<Value> = review
        .drafts
        .iter()
        .map(|draft| {
            let side = if draft.side == Side::Source { "RIGHT" } else { "LEFT" };
            let mut comment = json!({ "path": draft.path, "body": draft.body, "side": side, "line": draft.line });
            if let Some(start_line) = draft.start_line {
                comment["start_line"] = json!(start_line);
                comment["start_side"] = json!(side);
            }
            comment
        })
        .collect();
    json!({ "commit_id": review.source.commit, "body": sending.body, "event": sending.event, "comments": comments })
}

pub async fn send(State(state): State<AppState>, Param((id, number)): Param<(String, u64)>, Json(sending): Json<Sending>) -> Result<Json<PrReview>, Failure> {
    change_review(&state, &id, number, async |dir, review| {
        if !matches!(review.state, PrState::Open) {
            return Err((StatusCode::CONFLICT, format!("#{number} is not open")));
        }
        match review.drafts.iter().filter(|draft| draft.displaced).count() {
            0 => {}
            1 => return Err((StatusCode::CONFLICT, "1 Draft comment is Displaced".to_owned())),
            displaced => return Err((StatusCode::CONFLICT, format!("{displaced} Draft comments are Displaced"))),
        }
        if review.drafts.is_empty() && sending.body.trim().is_empty() {
            return Err((StatusCode::BAD_REQUEST, "a review needs Draft comments or a summary".to_owned()));
        }
        if review.mine && sending.event != Event::Comment {
            return Err((StatusCode::BAD_REQUEST, "GitHub refuses approving or requesting changes on your own pull request".to_owned()));
        }
        let repository = repository(dir).await?;
        github::send_review(dir, &repository, number, &payload(review, &sending)).await.map_err(|error| (StatusCode::BAD_GATEWAY, error))?;
        review.drafts.clear();
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_opens_only_a_pull_request_of_the_resolved_repository() {
        let repository = Repository { host: "github.com".to_owned(), owner: "acme".to_owned(), name: "widgets".to_owned(), remote: "origin".to_owned() };
        assert_eq!(named("https://github.com/acme/widgets/pull/7", &repository).unwrap(), 7);
        assert_eq!(named("https://GitHub.com/Acme/Widgets/pull/7/files#diff", &repository).unwrap(), 7);
        for url in ["https://github.com/acme/gadgets/pull/7", "https://ghe.corp/acme/widgets/pull/7", "https://github.com/acme/widgets/issues/7", "nonsense"] {
            assert_eq!(named(url, &repository).unwrap_err().0, StatusCode::BAD_REQUEST, "{url}");
        }
    }

    fn ranges(diff: &str) -> (Vec<(u32, u32)>, Vec<(u32, u32)>) {
        let Hunks { source, target } = hunks(diff);
        (source, target)
    }

    #[test]
    fn each_hunk_header_gives_its_target_and_source_ranges() {
        let diff = "diff --git a/list.txt b/list.txt\n--- a/list.txt\n+++ b/list.txt\n@@ -2,7 +2,8 @@ one\n two\n+@@ -9 +9 @@ looks like a header\n@@ -20,6 +21,6 @@ fn x() {\n";
        assert_eq!(ranges(diff), (vec![(2, 9), (21, 26)], vec![(2, 8), (20, 25)]));
    }

    #[test]
    fn a_pure_addition_or_deletion_has_no_range_on_its_empty_side() {
        assert_eq!(ranges("@@ -3,0 +4,2 @@\n+a\n+b\n"), (vec![(4, 5)], vec![]));
        assert_eq!(ranges("@@ -4,2 +3,0 @@\n-a\n-b\n"), (vec![], vec![(4, 5)]));
        assert_eq!(ranges("@@ -4 +4 @@\n-a\n+b\n"), (vec![(4, 4)], vec![(4, 4)]));
    }

    #[test]
    fn a_new_file_is_commentable_only_on_the_source_and_a_deleted_one_only_on_the_target() {
        let new = "new file mode 100644\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,3 @@\n+a\n+b\n+c\n";
        assert_eq!(ranges(new), (vec![(1, 3)], vec![]));
        let deleted = "deleted file mode 100644\n--- a/old.txt\n+++ /dev/null\n@@ -1,3 +0,0 @@\n-a\n-b\n-c\n";
        assert_eq!(ranges(deleted), (vec![], vec![(1, 3)]));
        assert_eq!(ranges("Binary files a/x.png and b/x.png differ\n"), (vec![], vec![]));
    }

    #[test]
    fn a_range_is_commentable_only_within_one_hunk_of_its_side() {
        let found = hunks("@@ -2,7 +2,8 @@\n@@ -20,6 +21,6 @@\n");
        assert!(found.contain(Side::Source, 2, 9));
        assert!(found.contain(Side::Source, 21, 21));
        assert!(!found.contain(Side::Source, 9, 21));
        assert!(!found.contain(Side::Source, 10, 10));
        assert!(found.contain(Side::Target, 20, 25));
        assert!(!found.contain(Side::Target, 26, 26));
        assert!(!found.contain(Side::Source, 5, 4));
    }

    fn through(diff: &str, start: u32, end: u32) -> Option<(u32, u32)> {
        map_through(&parse(diff), start, end)
    }

    #[test]
    fn lines_above_an_insertion_do_not_move() {
        assert_eq!(through("@@ -10,0 +11,3 @@\n", 4, 10), Some((4, 10)));
    }

    #[test]
    fn lines_below_an_insertion_of_k_lines_shift_by_k() {
        assert_eq!(through("@@ -10,0 +11,3 @@\n", 11, 12), Some((14, 15)));
        assert_eq!(through("@@ -0,0 +1,2 @@\n", 1, 1), Some((3, 3)));
        assert_eq!(through("@@ -3 +3,4 @@\n", 8, 8), Some((11, 11)));
    }

    #[test]
    fn lines_below_a_deletion_shift_up() {
        assert_eq!(through("@@ -4,2 +3,0 @@\n", 9, 9), Some((7, 7)));
        assert_eq!(through("@@ -2,3 +2 @@\n@@ -20,0 +19,1 @@\n", 10, 25), None);
        assert_eq!(through("@@ -2,3 +2 @@\n@@ -20,0 +19,1 @@\n", 21, 25), Some((20, 24)));
    }

    #[test]
    fn a_line_inside_a_changed_hunk_gives_none() {
        assert_eq!(through("@@ -5 +5 @@\n", 5, 5), None);
        assert_eq!(through("@@ -4,2 +3,0 @@\n", 5, 5), None);
        assert_eq!(through("@@ -5 +5 @@\n", 6, 6), Some((6, 6)));
    }

    #[test]
    fn a_range_partly_inside_a_changed_hunk_or_split_by_an_insertion_gives_none() {
        assert_eq!(through("@@ -5,2 +5,2 @@\n", 2, 5), None);
        assert_eq!(through("@@ -5,2 +5,2 @@\n", 6, 9), None);
        assert_eq!(through("@@ -6,0 +7,1 @@\n", 5, 8), None);
        assert_eq!(through("@@ -8,0 +9,1 @@\n", 5, 8), Some((5, 8)));
    }
}
