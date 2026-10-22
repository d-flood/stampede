use std::{path::Path, time::Duration};

use tokio::time::timeout;

use super::{Runs, Slot, Starting, discard, epic_branch, submodules, ticket_branch};
use crate::{
    chats::Chat,
    epics::{Event, Status},
    git,
    projects::{Kind, Role, Worktree},
};

pub enum Refusal {
    Unmerged(String),
    Refused(String),
}

impl Runs {
    pub async fn create_worktree(&self, project_id: &str, branch: Option<&str>, from: &str) -> Result<String, String> {
        let projects = self.projects.list();
        let project = projects.iter().find(|project| project.id == project_id).ok_or_else(|| format!("Project {project_id} is not open"))?;
        let checkout = Path::new(&project.path);
        match git::run(checkout, &["rev-parse", "--show-toplevel"]).await {
            Ok(toplevel) if Path::new(&toplevel) == checkout => {}
            _ => return Err(format!("{} is not the root of a git repository", project.path)),
        }
        if from.starts_with('-') || git::run(checkout, &["rev-parse", "-q", "--verify", &format!("{from}^{{commit}}")]).await.is_err() {
            return Err(format!("{from} is not a branch or commit"));
        }
        let exists = async |reference: String| git::run(checkout, &["show-ref", "-q", "--verify", &reference]).await.is_ok();
        let (branch, existing, track) = match branch {
            Some(branch) => (branch, false, "--no-track"),
            None if exists(format!("refs/heads/{from}")).await => (from, true, ""),
            None if exists(format!("refs/remotes/{from}")).await => (from.split_once('/').map_or(from, |(_, name)| name), false, "--track"),
            None => return Err(format!("{from} is not a branch")),
        };
        if branch.starts_with('-') || git::run(checkout, &["check-ref-format", "--branch", branch]).await.is_err() {
            return Err(format!("{branch} is not a valid branch name"));
        }
        let parent = project.worktree.as_ref().map_or(&project.id, |worktree| &worktree.parent);
        let path = format!("{}/_branches/{parent}/{}", self.worktrees, branch.replace('/', "-"));
        if Path::new(&path).exists() {
            return Err(format!("{path} already exists"));
        }
        if existing {
            git::run(checkout, &["worktree", "add", "-q", &path, branch]).await?;
        } else {
            git::run(checkout, &["worktree", "add", "-q", track, "-b", branch, &path, from]).await?;
        }
        if let Err(error) = submodules::init(Path::new(&path), checkout).await {
            if !existing {
                discard(&project.path, &path, branch).await;
            } else if let Err(removal) = git::run(checkout, &["worktree", "remove", "--force", &path]).await {
                eprintln!("cannot discard {path}: {removal}");
            }
            return Err(format!("Stampede could not check out the submodules of the new worktree: {error}"));
        }
        let worktree = Worktree { parent: parent.clone(), kind: Kind::Branch { branch: branch.to_owned(), existing } };
        Ok(self.projects.nest(&path, worktree).id)
    }

    pub async fn move_chat(&self, chat_id: &str, branch: &str) -> Result<Chat, String> {
        let source = self.chats.movable(chat_id).map_err(|error| error.to_string())?;
        if self.epics.holding(chat_id).or_else(|| self.epics.resolving(chat_id)).or_else(|| self.epics.reviewing(chat_id)).is_some() {
            return Err("An Epic's Run owns this Chat.".to_owned());
        }
        let projects = self.projects.list();
        let project = projects.iter().find(|project| project.id == source.project_id).ok_or("The Chat's Project is no longer open.")?;
        let checkout = Path::new(&project.path);
        if source.cwd != checkout {
            return Err("Only a Chat working in its Project's own checkout can move.".to_owned());
        }
        let status = git::read(checkout).await.ok_or_else(|| format!("{} is not a git repository", project.path))?;
        git::actions::settled(&status).map_err(|(_, reason)| reason)?;
        let base = git::run(checkout, &["rev-parse", "--abbrev-ref", "HEAD"]).await?;
        let commit = git::run(checkout, &["rev-parse", "HEAD"]).await?;
        let short = git::run(checkout, &["rev-parse", "--short", "HEAD"]).await?;
        let id = self.create_worktree(&project.id, Some(branch), &commit).await?;
        let path = self.projects.list().iter().find(|project| project.id == id).map(|project| project.path.clone()).ok_or("The new worktree is not open.")?;
        let abandon = async |error: String| {
            if let Err(Refusal::Unmerged(reason) | Refusal::Refused(reason)) = self.delete_worktree(&id, true).await {
                eprintln!("cannot discard {path}: {reason}");
            }
            Err(error)
        };
        match timeout(Duration::from_secs(30), self.chats.stop(chat_id)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return abandon(error.to_string()).await,
            Err(_) => return abandon("The Chat did not stop.".to_owned()).await,
        }
        let rigs = self.config.borrow().rigs.clone();
        let carried = match carry(checkout, Path::new(&path)).await {
            Ok(carried) => carried,
            Err(error) => {
                if let Err(reopened) = self.chats.reopen(source, &rigs).await {
                    eprintln!("cannot reopen chat {chat_id}: {reopened}");
                }
                return abandon(format!("The uncommitted changes could not move: {error}")).await;
            }
        };
        let from = if base == "HEAD" { format!("`{short}`") } else { format!("`{base}` at `{short}`") };
        let changes = if carried { ", and the uncommitted changes moved with it" } else { "" };
        let old = checkout.display();
        let note = format!(
            "FYI: Stampede moved this Chat to a new git worktree. The working directory is now `{path}`, on the new branch `{branch}`, cut from {from}{changes}. \
             Paths earlier in this conversation under `{old}` are now under `{path}`. Read and edit files only here, not in the original checkout."
        );
        self.chats.relocate(&source, &id, Path::new(&path), &note, &rigs).await.map_err(|error| format!("The worktree {path} is ready, but the Chat could not resume there: {error}"))
    }

    pub async fn delete_worktree(&self, project_id: &str, force: bool) -> Result<(), Refusal> {
        let projects = self.projects.list();
        let nested = projects.iter().find(|project| project.id == project_id).and_then(|project| Some((project, project.worktree.as_ref()?)));
        let Some((project, worktree)) = nested else {
            return Err(Refusal::Refused(format!("Project {project_id} is not a worktree")));
        };
        let checkout = projects.iter().find(|other| other.id == worktree.parent).map(|parent| parent.path.clone());
        let checkout = checkout.ok_or_else(|| Refusal::Refused(format!("The Project {} belongs to is not open.", project.path)))?;
        let path = Path::new(&project.path);
        let existing = matches!(worktree.kind, Kind::Branch { existing: true, .. });
        let _cutting = self.cutting.lock().await;
        let (event, branch, _starting) = match &worktree.kind {
            Kind::Branch { branch, .. } => (None, branch.clone(), None),
            Kind::Epic { epic, role } => self.hold(epic, *role, path)?,
        };
        if !force && let Some(work) = unmerged(path, &branch, existing).await {
            return Err(Refusal::Unmerged(work));
        }
        git::run(Path::new(&checkout), &["worktree", "remove", "--force", &project.path])
            .await
            .map_err(|error| Refusal::Refused(format!("git could not remove {}: {error}", project.path)))?;
        self.close_project(project_id);
        if !existing && let Err(error) = git::run(Path::new(&checkout), &["branch", "-D", &branch]).await {
            eprintln!("cannot delete {branch}: {error}");
        }
        if let (Some(event), Kind::Epic { epic, .. }) = (event, &worktree.kind)
            && let Err(error) = self.epics.record(epic, event)
        {
            eprintln!("cannot record the removal of {}: {error}", project.path);
        }
        Ok(())
    }

    fn hold(&self, slug: &str, role: Role, path: &Path) -> Result<(Option<Event>, String, Option<Starting>), Refusal> {
        let starting = match role {
            Role::Ticket(number) => Some(Starting::new(self, slug, Slot::Ticket(number)).map_err(Refusal::Refused)?),
            Role::Integration => None,
        };
        let epic = self.epics.get(slug).map_err(Refusal::Refused)?;
        let busy = |status: Status| matches!(status, Status::InProgress | Status::Merging);
        match role {
            Role::Integration => {
                if epic.run.is_some() {
                    return Err(Refusal::Refused(format!("{slug} has a live Run, which merges into it.")));
                }
                if let Some(number) = self.starting(slug).into_iter().find_map(|slot| match slot {
                    Slot::Ticket(number) => Some(number),
                    _ => None,
                }) {
                    return Err(Refusal::Refused(format!("{slug}/{number} is starting from it.")));
                }
                if let Some((number, ticket)) = epic.tickets.iter().find(|(_, ticket)| busy(ticket.status)) {
                    return Err(Refusal::Refused(format!("{slug}/{number} is {} and merges into it.", ticket.status)));
                }
                let event = (epic.integration.as_deref() == Some(path)).then_some(Event::IntegrationRemoved);
                Ok((event, epic_branch(slug), starting))
            }
            Role::Ticket(number) => {
                let event = match epic.tickets.get(&number).filter(|ticket| ticket.worktree.as_deref() == Some(path)) {
                    Some(ticket) if busy(ticket.status) => {
                        return Err(Refusal::Refused(format!("{slug}/{number} is {} in it.", ticket.status)));
                    }
                    Some(_) => Some(Event::WorktreeRemoved { ticket: number }),
                    None => None,
                };
                let name = path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
                Ok((event, ticket_branch(slug, &name), starting))
            }
        }
    }
}

async fn unmerged(path: &Path, branch: &str, existing: bool) -> Option<String> {
    let exclude = format!("--exclude={branch}");
    let args = ["rev-list", "--count", "HEAD", branch, "--not", &exclude, "--branches", "--remotes"];
    let ahead = async || match existing {
        true => Ok(0),
        false => git::run(path, &args).await.map(|ahead| ahead.parse::<usize>().unwrap_or_default()),
    };
    let (status, ahead) = match (submodules::status(path).await, ahead().await) {
        (Ok(status), Ok(ahead)) => (status, ahead),
        (Err(error), _) | (_, Err(error)) => {
            return Some(format!("Stampede could not tell whether it holds work no other branch has: {error}"));
        }
    };
    let commits = match ahead {
        0 => None,
        1 => Some("1 commit no other branch has".to_owned()),
        count => Some(format!("{count} commits no other branch has")),
    };
    let work = match (status.is_empty(), commits) {
        (true, None) => return None,
        (false, None) => "uncommitted changes".to_owned(),
        (true, Some(commits)) => commits,
        (false, Some(commits)) => format!("uncommitted changes and {commits}"),
    };
    let doomed = if existing { String::new() } else { format!(" and {branch}") };
    Some(format!("It has {work}. Deleting it{doomed} discards them."))
}

async fn carry(from: &Path, to: &Path) -> Result<bool, String> {
    let stashed = async |dir: &Path| git::run(dir, &["rev-parse", "-q", "--verify", "refs/stash"]).await.ok();
    let before = stashed(from).await;
    git::run(from, &["stash", "push", "--include-untracked", "-q", "-m", "Stampede: moving a Chat to a new worktree"]).await?;
    // A push with nothing to stash still succeeds; popping then would take an older, unrelated stash.
    if stashed(from).await == before {
        return Ok(false);
    }
    if let Err(error) = git::run(to, &["stash", "pop", "--index", "-q"]).await {
        git::run(from, &["stash", "pop", "--index", "-q"]).await.map_err(|restored| format!("{error}; they are kept in git stash: {restored}"))?;
        return Err(error);
    }
    Ok(true)
}
