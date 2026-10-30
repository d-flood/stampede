use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use serde::Serialize;
use tokio::{
    process::Command,
    sync::{broadcast::error::RecvError, watch},
    time::{Instant, sleep_until, timeout},
};

use crate::{
    config::Config,
    git::{self, Git},
    projects::{List, Project},
};

const EVERY: Duration = Duration::from_secs(30 * 60);
const LIMIT: Duration = Duration::from_secs(60);
const SETTLE: Duration = Duration::from_secs(1);
const LISTED: usize = 20;
const BUILT: &str = env!("STAMPEDE_COMMIT");
const SOURCE: &str = env!("CARGO_MANIFEST_DIR");

/// What origin has that is not here yet: each Project's default branch when a pull would fast-forward it, and what
/// Stampede's own repository has beyond the commit this server was built from.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(tag = "type", rename = "origin.changed")]
pub struct Origin {
    trunks: BTreeMap<String, Trunk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    update: Option<Update>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Trunk {
    branch: String,
    behind: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Update {
    behind: u32,
    subjects: Vec<String>,
    pulled: bool,
    head: String,
}

/// Fetches every open repository once a Device connects and every EVERY while one stays connected, when settings
/// allow, and reads origin again as git changes each Project.
pub fn spawn(
    mut projects: watch::Receiver<List>,
    config: watch::Receiver<Arc<Config>>,
    mut devices: watch::Receiver<usize>,
    git: Git,
) -> watch::Receiver<Origin> {
    let (sender, receiver) = watch::channel(Origin::default());
    tokio::spawn(async move {
        let mut changes = git.subscribe();
        let (mut due, mut settled) = (Instant::now(), None);
        let (mut all, mut stale) = (true, HashSet::new());
        projects.mark_changed();
        loop {
            let connected = *devices.borrow_and_update() > 0;
            tokio::select! {
                () = sleep_until(due), if connected => {
                    if config.borrow().auto_fetch {
                        let list = projects.borrow().clone();
                        fetch(&list, &git).await;
                    }
                    due = Instant::now() + EVERY;
                    all = true;
                }
                () = sleep_until(settled.unwrap_or(due)), if settled.is_some() => {}
                changed = changes.recv() => {
                    match changed {
                        Ok(changed) => {
                            stale.insert(changed.project_id);
                        }
                        Err(RecvError::Lagged(_)) => all = true,
                        Err(RecvError::Closed) => break,
                    }
                    settled.get_or_insert(Instant::now() + SETTLE);
                    continue;
                }
                Ok(()) = projects.changed() => all = true,
                Ok(()) = devices.changed() => continue,
            }
            let list = projects.borrow_and_update().clone();
            let mut origin = sender.borrow().clone();
            origin.trunks.retain(|id, _| list.iter().any(|project| project.id == *id));
            let mut source = all;
            for project in list.iter().filter(|project| all || stale.contains(&project.id)) {
                match trunk(Path::new(&project.path)).await {
                    Some(trunk) => origin.trunks.insert(project.id.clone(), trunk),
                    None => origin.trunks.remove(&project.id),
                };
                source |= Path::new(SOURCE).starts_with(&project.path);
            }
            if source {
                origin.update = update(Path::new(SOURCE), BUILT).await;
            }
            (all, settled) = (false, None);
            stale.clear();
            sender.send_if_modified(|current| {
                let differs = *current != origin;
                *current = origin;
                differs
            });
        }
    });
    receiver
}

/// Fetches each repository once, however many open Projects share it, then has those Projects read again.
async fn fetch(list: &[Project], git: &Git) {
    let mut repositories = BTreeMap::<PathBuf, (PathBuf, Vec<&str>)>::new();
    for project in list {
        if let Some(common) = git::common_dir(Path::new(&project.path)).await {
            let (_, ids) = repositories.entry(common).or_insert_with(|| (PathBuf::from(&project.path), Vec::new()));
            ids.push(&project.id);
        }
    }
    if !BUILT.is_empty()
        && let Some(common) = git::common_dir(Path::new(SOURCE)).await
    {
        repositories.entry(common).or_insert_with(|| (PathBuf::from(SOURCE), Vec::new()));
    }
    for (dir, ids) in repositories.into_values() {
        if git::run(&dir, &["config", "--get", "remote.origin.url"]).await.is_err() {
            continue;
        }
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&dir)
            .args(["fetch", "--quiet", "origin"])
            // An empty GIT_ASKPASS also stops git falling back to SSH_ASKPASS.
            .envs([("GIT_TERMINAL_PROMPT", "0"), ("GIT_ASKPASS", ""), ("SSH_ASKPASS_REQUIRE", "never"), ("GCM_INTERACTIVE", "never")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if !matches!(timeout(LIMIT, command.status()).await, Ok(Ok(status)) if status.success()) {
            eprintln!("cannot fetch origin in {}", dir.display());
        }
        for id in ids {
            git.reread(id);
        }
    }
}

/// The default branch, when a pull would fast-forward it, with how many commits it trails origin.
async fn trunk(dir: &Path) -> Option<Trunk> {
    let branch = default_branch(dir).await?;
    let (ahead, behind) = counts(dir, &format!("refs/heads/{branch}"), &format!("refs/remotes/origin/{branch}")).await?;
    (ahead == 0 && behind > 0).then_some(Trunk { branch, behind })
}

/// What origin's default branch has beyond `built`, when the build trails it rather than diverging from it.
async fn update(dir: &Path, built: &str) -> Option<Update> {
    if built.is_empty() {
        return None;
    }
    let origin = format!("refs/remotes/origin/{}", default_branch(dir).await?);
    let (ahead, behind) = counts(dir, built, &origin).await?;
    if ahead > 0 || behind == 0 {
        return None;
    }
    let (listed, range) = (format!("--max-count={LISTED}"), format!("{built}..{origin}"));
    let subjects = git::run(dir, &["log", "--format=%s", &listed, &range, "--"]).await.ok()?;
    let pulled = git::run(dir, &["merge-base", "--is-ancestor", &origin, "HEAD"]).await.is_ok();
    let head = git::run(dir, &["rev-parse", &origin]).await.ok()?;
    Some(Update { behind, subjects: subjects.lines().map(str::to_owned).collect(), pulled, head })
}

/// The branch origin's HEAD names, else main or master, whichever origin has.
async fn default_branch(dir: &Path) -> Option<String> {
    if let Ok(head) = git::run(dir, &["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"]).await {
        return head.strip_prefix("origin/").map(str::to_owned);
    }
    let args = ["for-each-ref", "--format=%(refname:lstrip=3)", "refs/remotes/origin/main", "refs/remotes/origin/master"];
    git::run(dir, &args).await.ok()?.lines().next().map(str::to_owned)
}

/// How many commits each side has that the other lacks.
async fn counts(dir: &Path, left: &str, right: &str) -> Option<(u32, u32)> {
    let counts = git::run(dir, &["rev-list", "--left-right", "--count", &format!("{left}...{right}"), "--"]).await.ok()?;
    let (left, right) = counts.split_once('\t')?;
    Some((left.parse().ok()?, right.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(["-c", "user.name=stampede", "-c", "user.email=test@stampede.invalid", "-c", "commit.gpgsign=false", "-C"])
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
    }

    #[tokio::test]
    async fn origin_counts_as_ahead_only_where_a_pull_would_fast_forward() {
        let root = tempfile::tempdir().unwrap();
        let (origin, source, other) = (root.path().join("origin.git"), root.path().join("source"), root.path().join("other"));
        git(root.path(), &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
        git(root.path(), &["clone", "-q", origin.to_str().unwrap(), other.to_str().unwrap()]);
        git(&other, &["commit", "-q", "--allow-empty", "-m", "First"]);
        git(&other, &["push", "-q", "origin", "main"]);
        git(root.path(), &["clone", "-q", origin.to_str().unwrap(), source.to_str().unwrap()]);
        let built = git(&source, &["rev-parse", "HEAD"]);
        assert_eq!(update(&source, &built).await, None);

        git(&other, &["commit", "-q", "--allow-empty", "-m", "Second"]);
        git(&other, &["commit", "-q", "--allow-empty", "-m", "Third"]);
        git(&other, &["push", "-q", "origin", "main"]);
        git(&source, &["fetch", "-q"]);
        let (subjects, head) = (vec!["Third".to_owned(), "Second".to_owned()], git(&source, &["rev-parse", "origin/main"]));
        assert_eq!(update(&source, &built).await, Some(Update { behind: 2, subjects: subjects.clone(), pulled: false, head: head.clone() }));
        git(&source, &["pull", "-q", "--ff-only"]);
        assert_eq!(update(&source, &built).await, Some(Update { behind: 2, subjects, pulled: true, head }));

        git(&source, &["commit", "-q", "--allow-empty", "-m", "Mine"]);
        let diverged = git(&source, &["rev-parse", "HEAD"]);
        git(&other, &["commit", "-q", "--allow-empty", "-m", "Fourth"]);
        git(&other, &["push", "-q", "origin", "main"]);
        git(&source, &["fetch", "-q"]);
        assert_eq!(update(&source, &diverged).await, None);
        assert_eq!(update(&source, "").await, None);
        assert_eq!(trunk(&source).await, None);
        git(&source, &["reset", "-q", "--hard", "HEAD~"]);
        assert_eq!(trunk(&source).await, Some(Trunk { branch: "main".to_owned(), behind: 1 }));
    }
}
