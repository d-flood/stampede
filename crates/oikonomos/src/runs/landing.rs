use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use tokio::process::Command;

use super::{Runs, epic_branch};
use crate::{
    epics::{Epic, Event, Status},
    git,
};

/// A Landing every check has passed: the Epic branch, the base it merges into, and the checkout it merges in.
struct Landing {
    slug: String,
    title: String,
    checkout: PathBuf,
    base: String,
    branch: String,
}

impl Runs {
    /// Merges the Epic branch into its base in the Project's own checkout, then removes the Integration Worktree and
    /// the Epic branch if they are clean. Refuses, changing nothing and keeping why as a Note, whenever it cannot.
    pub async fn land(&self, slug: &str) -> Result<String, String> {
        let _cutting = self.cutting.lock().await;
        let epic = self.epics.get(slug)?;
        let (landing, head) = match attempt(&epic).await {
            Ok(landed) => landed,
            Err(reason) => {
                self.epics.record(slug, Event::LandingRefused { reason: reason.clone() })?;
                return Err(reason);
            }
        };
        self.epics.record(slug, Event::Landed { base: landing.base.clone(), head: head.clone() })?;
        let landed = format!("Landed {slug} into {} at {}.", landing.base, &head[..head.len().min(12)]);
        Ok(match self.tidy(&landing).await? {
            Some(kept) => format!("{landed} {kept}"),
            None => landed,
        })
    }

    /// Removes the Integration Worktree and the Epic branch after a Landing, or says why it kept them.
    async fn tidy(&self, landing: &Landing) -> Result<Option<String>, String> {
        let (slug, branch) = (&landing.slug, &landing.branch);
        let epic = self.epics.get(slug)?;
        let integration = epic.integration.as_deref().ok_or_else(|| format!("{slug} has no Integration Worktree"))?;
        let path = integration.to_string_lossy();
        let unsettled = epic.tickets.iter().find(|(_, ticket)| matches!(ticket.status, Status::InProgress | Status::Merging));
        let kept = match unsettled {
            Some((number, ticket)) => Some(format!(
                "Oikonomos kept the Integration Worktree at {path} and the Epic branch {branch}, because {slug}/{number} is {} and merges there.",
                ticket.status
            )),
            None => match git::run(integration, &["status", "--porcelain"]).await {
                Err(_) if !integration.exists() => git::run(&landing.checkout, &["worktree", "prune"])
                    .await
                    .err()
                    .map(|error| format!("git could not forget the missing Integration Worktree at {path}: {error}")),
                Err(error) => Some(format!("Oikonomos kept the Integration Worktree at {path}, where git status failed: {error}")),
                Ok(status) if !status.is_empty() => Some(format!(
                    "Oikonomos kept the Integration Worktree at {path} and the Epic branch {branch}, because the worktree has uncommitted changes."
                )),
                Ok(_) => git::run(&landing.checkout, &["worktree", "remove", &path])
                    .await
                    .err()
                    .map(|error| format!("git could not remove the Integration Worktree at {path}: {error}")),
            },
        };
        if kept.is_none()
            && let Err(error) = git::run(&landing.checkout, &["branch", "-d", branch]).await
        {
            eprintln!("cannot delete {branch}: {error}");
        }
        let event = match &kept {
            Some(reason) => Event::IntegrationKept { reason: reason.clone() },
            None => Event::IntegrationRemoved,
        };
        self.epics.record(slug, event)?;
        Ok(kept)
    }
}

/// Lands the Epic once every check passes, or says what stopped it; nothing changes before the merge itself.
async fn attempt(epic: &Epic) -> Result<(Landing, String), String> {
    let landing = Landing::check(epic).await?;
    landing.mergeable().await?;
    landing.merge().await?;
    let head = git::run(&landing.checkout, &["rev-parse", "HEAD"]).await.unwrap_or_default();
    Ok((landing, head))
}

impl Landing {
    /// What must hold before a Landing touches the checkout. Reads only.
    async fn check(epic: &Epic) -> Result<Self, String> {
        let slug = &epic.slug;
        let (Some(base), Some(_)) = (&epic.base, &epic.integration) else {
            return Err(format!("{slug} has no Epic branch yet, so there is nothing to land"));
        };
        let settled = "land once no Run is live and nothing is merging";
        if epic.run.is_some() {
            return Err(format!("{slug} has a live Run; {settled}"));
        }
        if let Some((number, _)) = epic.tickets.iter().find(|(_, ticket)| ticket.status == Status::Merging) {
            return Err(format!("{slug}/{number} is merging; {settled}"));
        }
        let checkout = checkout(&epic.project).await?;
        let shown = checkout.display();
        match git::run(&checkout, &["symbolic-ref", "-q", "--short", "HEAD"]).await {
            Ok(on) if on == *base => {}
            Ok(on) => {
                return Err(format!(
                    "{shown} is on {on} rather than {base}, and Oikonomos never switches branches in your checkout; switch to {base} and land again"
                ));
            }
            Err(_) => return Err(format!("{shown} is not on a branch; check out {base} and land again")),
        }
        if git::run(&checkout, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).await.is_ok() {
            return Err(format!("{shown} is in the middle of a merge, which is yours to finish"));
        }
        let status = git::run(&checkout, &["status", "--porcelain"]).await.map_err(|error| format!("git status failed in {shown}: {error}"))?;
        if !status.is_empty() {
            let paths: Vec<&str> = status.lines().map(|line| line.get(3..).unwrap_or(line)).collect();
            let more = if paths.len() > 5 { ", and more" } else { "" };
            return Err(format!(
                "{shown} has uncommitted changes in {}{more}; commit them or put them aside and land again",
                paths[..paths.len().min(5)].join(", ")
            ));
        }
        let branch = epic_branch(slug);
        if git::run(&checkout, &["rev-parse", "-q", "--verify", &format!("refs/heads/{branch}")]).await.is_err() {
            return Err(format!("{branch} is gone, so there is nothing to land"));
        }
        Ok(Self { slug: slug.clone(), title: epic.title.clone(), checkout, base: base.clone(), branch })
    }

    /// Refuses a merge that would conflict, found by merging the trees alone, without touching any checkout.
    async fn mergeable(&self) -> Result<(), String> {
        let (base, branch) = (&self.base, &self.branch);
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.checkout)
            .args(["merge-tree", "--write-tree", "--name-only", "--no-messages", base, branch])
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|error| format!("cannot run git: {error}"))?;
        match output.status.code() {
            Some(0) => Ok(()),
            Some(1) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let files: Vec<&str> = stdout.lines().skip(1).filter(|line| !line.is_empty()).collect();
                Err(format!(
                    "merging {branch} into {base} would conflict in {}, so Oikonomos left {} untouched; merge {base} into the Integration Worktree, resolve it there, and land again",
                    files.join(", "),
                    self.checkout.display()
                ))
            }
            _ => Err(format!("git could not tell whether {branch} merges cleanly into {base}: {}", String::from_utf8_lossy(&output.stderr).trim())),
        }
    }

    /// Merges the Epic branch into the base by a fast-forward or a merge commit, aborting rather than leaving a conflict.
    async fn merge(&self) -> Result<(), String> {
        let message = format!("Land {}: {}", self.slug, self.title);
        let Err(error) = git::run(&self.checkout, &["merge", "--no-edit", "-m", &message, &self.branch]).await else { return Ok(()) };
        if git::run(&self.checkout, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).await.is_ok() {
            git::run(&self.checkout, &["merge", "--abort"]).await?;
        }
        Err(format!("git refused to merge {} into {}: {error}", self.branch, self.base))
    }
}

/// The Project's own checkout: the main worktree of the repository whose git common directory is `common_dir`.
async fn checkout(common_dir: &Path) -> Result<PathBuf, String> {
    let shown = common_dir.display();
    let list = git::run(common_dir, &["worktree", "list", "--porcelain"]).await.map_err(|error| format!("git could not list the worktrees of {shown}: {error}"))?;
    let mut lines = list.lines();
    match (lines.next().and_then(|line| line.strip_prefix("worktree ")), lines.next()) {
        (Some(path), next) if next != Some("bare") => Ok(PathBuf::from(path)),
        _ => Err(format!("{shown} has no checkout of its own to land into")),
    }
}
