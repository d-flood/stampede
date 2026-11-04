use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use axum::{
    Json,
    extract::{Path as Param, State},
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
    viewed: BTreeMap<String, Value>,
    drafts: Vec<Value>,
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
    let (viewed, drafts) = existing.map(|review| (review.viewed, review.drafts)).unwrap_or_default();
    let review = PrReview {
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
    state.pr_reviews.save(common_dir, &review)?;
    Ok(review)
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
}
