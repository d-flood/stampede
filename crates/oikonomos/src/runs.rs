use std::{
    collections::HashSet,
    env, fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};

use tokio::{process::Command, sync::mpsc};

use crate::{
    chats::Chats,
    config::Rig,
    epics::{Author, Epic, Epics, Event, Status},
    git,
    projects::Project,
    templates::TICKET,
};

const TAIL: usize = 40;

/// The loop: starts Tickets in their own worktrees, takes their Verdicts, and merges them one at a time.
pub struct Runs {
    data_dir: PathBuf,
    worktrees: String,
    cli: String,
    epics: Arc<Epics>,
    chats: Arc<Chats>,
    starting: Mutex<HashSet<(String, u32)>>,
    cutting: tokio::sync::Mutex<()>,
    merges: mpsc::UnboundedSender<String>,
}

impl Runs {
    /// Starts the merge queue, picking up whatever was left merging or uncleaned when the server last stopped.
    pub fn new(data_dir: &Path, epics: Arc<Epics>, chats: Arc<Chats>) -> io::Result<Arc<Self>> {
        let utf8 = |path: PathBuf| path.into_os_string().into_string().map_err(|_| io::Error::other("a path is not UTF-8"));
        let (merges, queued) = mpsc::unbounded_channel();
        let runs = Arc::new(Self {
            data_dir: data_dir.to_owned(),
            worktrees: utf8(fs::canonicalize(data_dir)?.join("worktrees"))?,
            cli: utf8(env::current_exe()?)?,
            epics,
            chats,
            starting: Mutex::default(),
            cutting: tokio::sync::Mutex::new(()),
            merges,
        });
        tokio::spawn(Arc::clone(&runs).merge_queued(queued));
        for slug in runs.epics.slugs() {
            let _ = runs.merges.send(slug);
        }
        Ok(runs)
    }

    fn worktree(&self, slug: &str, name: &str) -> String {
        format!("{}/{slug}/{name}", self.worktrees)
    }

    /// Starts a Frontier Ticket: cuts its worktree, runs the Setup Command there, and gives it to a new Chat.
    pub async fn start(&self, slug: &str, number: u32, project: &Project, rig: &Rig) -> Result<String, String> {
        let _starting = Starting::new(self, slug, number)?;
        let epic = self.epics.get(slug)?;
        let Some(setup) = epic.setup.clone() else {
            return Err(format!(
                "{slug} has no Setup Command yet; record one, \"\" when a fresh worktree needs nothing, before starting its Tickets"
            ));
        };
        let ticket = epic.ticket(number)?;
        if !epic.frontier().any(|(other, _)| other == number) {
            let unresolved = |blocker: &&u32| epic.tickets.get(blocker).is_none_or(|blocker| !blocker.status.resolved());
            let blockers: Vec<_> = ticket.blockers.iter().filter(unresolved).map(|blocker| format!("{slug}/{blocker}")).collect();
            return Err(match ticket.status {
                Status::NotStarted => format!("{slug}/{number} is not on the Frontier until {} are merged or skipped", blockers.join(", ")),
                status => format!("{slug}/{number} is {status}, so it is not on the Frontier"),
            });
        }
        let integration = {
            let _cutting = self.cutting.lock().await;
            match self.epics.get(slug)?.integration {
                Some(integration) => integration.to_string_lossy().into_owned(),
                None => self.cut(&epic, project, &setup, number).await?,
            }
        };
        let (worktree, branch) = self.fresh(Path::new(&integration), slug, number).await;
        git::run(Path::new(&integration), &["worktree", "add", "-b", &branch, &worktree, &epic_branch(slug)]).await?;
        if let Err(note) = run_setup(&setup, &worktree, "this Ticket's new worktree").await {
            discard(&integration, &worktree, &branch).await;
            return Err(self.setup_failed(slug, number, note));
        }
        let chat = match self.chats.create(project, Path::new(&worktree), rig, None).await {
            Ok(chat) => chat,
            Err(error) => {
                discard(&integration, &worktree, &branch).await;
                return Err(format!("cannot start a Chat for {slug}/{number}: {error}"));
            }
        };
        let claim = Event::Claimed { ticket: number, chat: chat.id.clone(), rig: rig.name.clone(), worktree: PathBuf::from(&worktree) };
        if let Err(error) = self.epics.record(slug, claim) {
            self.chats.close(&chat.id);
            discard(&integration, &worktree, &branch).await;
            return Err(error);
        }
        self.chats.send(&chat.id, &self.prompt(&epic, number, &branch), false, &[]);
        Ok(chat.id)
    }

    /// The worktree and branch `<n>`, or `<n>-2`, `<n>-3`… when an earlier Claim on the Ticket left them behind.
    async fn fresh(&self, repo: &Path, slug: &str, number: u32) -> (String, String) {
        let mut attempt = 1;
        loop {
            let name = if attempt == 1 { number.to_string() } else { format!("{number}-{attempt}") };
            let (worktree, branch) = (self.worktree(slug, &name), ticket_branch(slug, &name));
            let exists = git::run(repo, &["rev-parse", "-q", "--verify", &format!("refs/heads/{branch}")]).await.is_ok();
            if !exists && !Path::new(&worktree).exists() {
                return (worktree, branch);
            }
            attempt += 1;
        }
    }

    /// Cuts the Epic branch from the Project's current branch, checked out in a new Integration Worktree.
    async fn cut(&self, epic: &Epic, project: &Project, setup: &str, number: u32) -> Result<String, String> {
        let slug = &epic.slug;
        let checkout = Path::new(&project.path);
        let base = git::run(checkout, &["symbolic-ref", "--short", "HEAD"])
            .await
            .map_err(|_| format!("{} is not on a branch, so there is nothing to cut {slug}'s branch from", project.path))?;
        let (branch, worktree) = (epic_branch(slug), self.worktree(slug, "integration"));
        git::run(checkout, &["worktree", "add", "-b", &branch, &worktree, &base]).await?;
        if let Err(note) = run_setup(setup, &worktree, "the Epic's new Integration Worktree").await {
            discard(&project.path, &worktree, &branch).await;
            return Err(self.setup_failed(slug, number, note));
        }
        self.epics.record(slug, Event::BranchCut { base, worktree: worktree.clone().into() })?;
        Ok(worktree)
    }

    fn setup_failed(&self, slug: &str, number: u32, note: String) -> String {
        let recorded = self
            .epics
            .record(slug, Event::NoteAdded { ticket: number, author: Author::Oikonomos, text: note })
            .and_then(|()| self.epics.record(slug, Event::SetupFailed { ticket: number }));
        if let Err(error) = recorded {
            eprintln!("cannot record {slug}/{number}'s failed setup: {error}");
        }
        format!("The Setup Command failed for {slug}/{number}; its Note has the end of the output")
    }

    /// The Ticket Template, then the Ticket itself, which `templates::recognize` shows as what followed `/ticket`.
    fn prompt(&self, epic: &Epic, number: u32, branch: &str) -> String {
        let ticket = &epic.tickets[&number];
        let reference = format!("{}/{number}", epic.slug);
        format!(
            "{}\n\nYour Ticket is {reference}, on the branch `{branch}`. On this Host `oikonomos` is `{}`; run it by that path if it is not on your PATH.\n\n## {reference}: {}\n\n{}",
            TICKET.opening(&self.data_dir),
            quote(&self.cli),
            ticket.title,
            ticket.body.trim_end()
        )
    }

    /// Commits what the Ticket's Chat left uncommitted, records the Verdict, and queues the merge.
    pub async fn complete(&self, slug: &str, number: u32, cwd: &Path, message: &str) -> Result<String, String> {
        let worktree = self.held(slug, number, cwd)?;
        if message.trim().is_empty() {
            return Err("the commit message is empty".to_owned());
        }
        if !git::run(&worktree, &["status", "--porcelain"]).await?.is_empty() {
            git::run(&worktree, &["add", "-A"]).await?;
            git::run(&worktree, &["commit", "-q", "-m", message]).await?;
        }
        let commit = git::run(&worktree, &["rev-parse", "HEAD"]).await?;
        self.epics.record(slug, Event::Completed { ticket: number, commit })?;
        let _ = self.merges.send(slug.to_owned());
        Ok(format!("Recorded the Verdict: {slug}/{number} is complete, and Oikonomos merges it into the Integration Worktree next."))
    }

    pub fn skip(&self, slug: &str, number: u32, cwd: &Path, reason: &str) -> Result<String, String> {
        self.held(slug, number, cwd)?;
        if reason.trim().is_empty() {
            return Err("the reason is empty".to_owned());
        }
        self.epics.record(slug, Event::Skipped { ticket: number, reason: reason.to_owned() })?;
        let _ = self.merges.send(slug.to_owned());
        Ok(format!("Recorded the Verdict: {slug}/{number} is skipped."))
    }

    /// The worktree of an in-progress Ticket, provided `cwd` lies inside it.
    fn held(&self, slug: &str, number: u32, cwd: &Path) -> Result<PathBuf, String> {
        let epic = self.epics.get(slug)?;
        let ticket = epic.ticket(number)?;
        if ticket.status != Status::InProgress {
            return Err(format!("{slug}/{number} is {}, not in progress, so it takes no Verdict", ticket.status));
        }
        let worktree = ticket.worktree.clone().ok_or_else(|| format!("{slug}/{number} has no Ticket Worktree"))?;
        let cwd = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_owned());
        if !cwd.starts_with(&worktree) {
            return Err(format!(
                "a Verdict on {slug}/{number} is taken only from inside its Ticket Worktree, {}, and this ran in {}",
                worktree.display(),
                cwd.display()
            ));
        }
        Ok(worktree)
    }

    async fn merge_queued(self: Arc<Self>, mut queued: mpsc::UnboundedReceiver<String>) {
        while let Some(slug) = queued.recv().await {
            while let Ok(epic) = self.epics.get(&slug) {
                let settled = if let Some(&number) = epic.queue.first() {
                    self.merge(&epic, number).await
                } else if let Some((&number, _)) = epic.tickets.iter().find(|(_, ticket)| ticket.awaits_cleanup()) {
                    self.clean(&epic, number).await
                } else {
                    break;
                };
                if let Err(error) = settled {
                    eprintln!("cannot settle {slug}: {error}");
                    break;
                }
            }
        }
    }

    /// Merges a completed Ticket into the Integration Worktree, or aborts the merge and flags the Ticket.
    async fn merge(&self, epic: &Epic, number: u32) -> Result<(), String> {
        let slug = &epic.slug;
        let ticket = epic.ticket(number)?;
        let integration = epic.integration.as_deref().ok_or_else(|| format!("{slug} has no Integration Worktree"))?;
        let commit = ticket.commit.as_deref().ok_or_else(|| format!("{slug}/{number} has no completed commit"))?;
        let message = format!("Merge {slug}/{number}: {}", ticket.title);
        let event = match git::run(integration, &["merge", "--no-ff", "--no-edit", "-m", &message, commit]).await {
            Ok(_) => Event::Merged { ticket: number },
            Err(error) => {
                let conflicts = git::run(integration, &["diff", "--name-only", "--diff-filter=U"]).await.unwrap_or_default();
                if git::run(integration, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).await.is_ok() {
                    git::run(integration, &["merge", "--abort"]).await?;
                }
                let reason = if conflicts.is_empty() {
                    format!("Its merge into the Integration Worktree failed: {error}")
                } else {
                    let files: Vec<_> = conflicts.lines().collect();
                    format!("Its merge into the Integration Worktree conflicted in {}, so Oikonomos aborted it.", files.join(", "))
                };
                Event::MergeFailed { ticket: number, reason }
            }
        };
        self.epics.record(slug, event)
    }

    /// Removes a resolved Ticket's worktree and branch, unless that would lose work.
    async fn clean(&self, epic: &Epic, number: u32) -> Result<(), String> {
        let slug = &epic.slug;
        let ticket = epic.ticket(number)?;
        let integration = epic.integration.as_deref().ok_or_else(|| format!("{slug} has no Integration Worktree"))?;
        let worktree = ticket.worktree.as_deref().ok_or_else(|| format!("{slug}/{number} has no Ticket Worktree"))?;
        let name = worktree.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
        let (path, branch) = (worktree.to_string_lossy(), ticket_branch(slug, &name));
        let kept = |reason: String| Event::WorktreeKept { ticket: number, reason };
        let event = match git::run(worktree, &["status", "--porcelain"]).await {
            Err(_) if !worktree.exists() => {
                git::run(integration, &["worktree", "prune"]).await?;
                Event::WorktreeRemoved { ticket: number }
            }
            Err(error) => kept(format!("Oikonomos kept its Ticket Worktree at {path}, where git status failed: {error}")),
            Ok(status) if !status.is_empty() => {
                kept(format!("Its Ticket Worktree at {path} has uncommitted changes, so Oikonomos kept it."))
            }
            Ok(_) if git::run(integration, &["merge-base", "--is-ancestor", &branch, &epic_branch(slug)]).await.is_err() => kept(format!(
                "Oikonomos kept its Ticket Worktree at {path} and its branch {branch}, which has commits the Epic branch lacks."
            )),
            Ok(_) => match git::run(integration, &["worktree", "remove", &path]).await {
                Err(error) => kept(format!("git could not remove its Ticket Worktree at {path}: {error}")),
                Ok(_) => {
                    if let Err(error) = git::run(integration, &["branch", "-D", &branch]).await {
                        eprintln!("cannot delete {branch}: {error}");
                    }
                    Event::WorktreeRemoved { ticket: number }
                }
            },
        };
        self.epics.record(slug, event)
    }
}

/// Marks a Ticket as starting, so a second Start cannot race the first through its Setup Command.
struct Starting<'a> {
    runs: &'a Runs,
    key: (String, u32),
}

impl<'a> Starting<'a> {
    fn new(runs: &'a Runs, slug: &str, number: u32) -> Result<Self, String> {
        let key = (slug.to_owned(), number);
        if !runs.starting.lock().unwrap().insert(key.clone()) {
            return Err(format!("{slug}/{number} is already starting"));
        }
        Ok(Self { runs, key })
    }
}

impl Drop for Starting<'_> {
    fn drop(&mut self) {
        self.runs.starting.lock().unwrap().remove(&self.key);
    }
}

fn epic_branch(slug: &str) -> String {
    format!("oikonomos/{slug}/epic")
}

fn ticket_branch(slug: &str, name: &str) -> String {
    format!("oikonomos/{slug}/{name}")
}

/// Runs the Setup Command in a fresh worktree, giving a Note with the end of its output when it fails.
async fn run_setup(command: &str, worktree: &str, place: &str) -> Result<(), String> {
    if command.trim().is_empty() {
        return Ok(());
    }
    let output = Command::new("sh")
        .args(["-c", command])
        .current_dir(worktree)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|error| format!("The Setup Command `{command}` could not run in {place}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let tail = lines[lines.len().saturating_sub(TAIL)..].join("\n");
    Err(format!("The Setup Command `{command}` failed in {place} ({}). The end of its output:\n\n```text\n{tail}\n```", output.status))
}

/// Removes a worktree and branch made moments ago, which hold nothing yet.
async fn discard(repo: &str, worktree: &str, branch: &str) {
    let repo = Path::new(repo);
    let removed = git::run(repo, &["worktree", "remove", "--force", worktree]).await;
    if let Err(error) = removed.and(git::run(repo, &["branch", "-D", branch]).await) {
        eprintln!("cannot discard {worktree}: {error}");
    }
}

fn quote(text: &str) -> String {
    if text.chars().all(|char| char.is_ascii_alphanumeric() || "/._-+=:@,%".contains(char)) {
        text.to_owned()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}
