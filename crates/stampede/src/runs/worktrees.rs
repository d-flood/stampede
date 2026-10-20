use std::path::Path;

use super::{Runs, Slot, Starting, discard, epic_branch, submodules, ticket_branch};
use crate::{
    epics::{Event, Status},
    git,
    projects::{Kind, Role, Worktree},
};

pub enum Refusal {
    Unmerged(String),
    Refused(String),
}

impl Runs {
    pub async fn create_worktree(&self, project_id: &str, branch: &str, from: &str) -> Result<String, String> {
        let projects = self.projects.list();
        let project = projects.iter().find(|project| project.id == project_id).ok_or_else(|| format!("Project {project_id} is not open"))?;
        let checkout = Path::new(&project.path);
        match git::run(checkout, &["rev-parse", "--show-toplevel"]).await {
            Ok(toplevel) if Path::new(&toplevel) == checkout => {}
            _ => return Err(format!("{} is not the root of a git repository", project.path)),
        }
        if branch.starts_with('-') || git::run(checkout, &["check-ref-format", "--branch", branch]).await.is_err() {
            return Err(format!("{branch} is not a valid branch name"));
        }
        if from.starts_with('-') || git::run(checkout, &["rev-parse", "-q", "--verify", &format!("{from}^{{commit}}")]).await.is_err() {
            return Err(format!("{from} is not a branch or commit"));
        }
        let parent = project.worktree.as_ref().map_or(&project.id, |worktree| &worktree.parent);
        let path = format!("{}/_branches/{parent}/{}", self.worktrees, branch.replace('/', "-"));
        if Path::new(&path).exists() {
            return Err(format!("{path} already exists"));
        }
        git::run(checkout, &["worktree", "add", "-q", "--no-track", "-b", branch, &path, from]).await?;
        if let Err(error) = submodules::init(Path::new(&path), checkout).await {
            discard(&project.path, &path, branch).await;
            return Err(format!("Stampede could not check out the submodules of the new worktree: {error}"));
        }
        let worktree = Worktree { parent: parent.clone(), kind: Kind::Branch { branch: branch.to_owned() } };
        Ok(self.projects.nest(&path, worktree).id)
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
        let _cutting = self.cutting.lock().await;
        let (event, branch, _starting) = match &worktree.kind {
            Kind::Branch { branch } => (None, branch.clone(), None),
            Kind::Epic { epic, role } => self.hold(epic, *role, path)?,
        };
        if !force && let Some(work) = unmerged(path, &branch).await {
            return Err(Refusal::Unmerged(work));
        }
        git::run(Path::new(&checkout), &["worktree", "remove", "--force", &project.path])
            .await
            .map_err(|error| Refusal::Refused(format!("git could not remove {}: {error}", project.path)))?;
        self.close_project(project_id);
        if let Err(error) = git::run(Path::new(&checkout), &["branch", "-D", &branch]).await {
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

async fn unmerged(path: &Path, branch: &str) -> Option<String> {
    let exclude = format!("--exclude={branch}");
    let ahead = ["rev-list", "--count", "HEAD", branch, "--not", &exclude, "--branches", "--remotes"];
    let (status, ahead) = match (submodules::status(path).await, git::run(path, &ahead).await) {
        (Ok(status), Ok(ahead)) => (status, ahead.parse::<usize>().unwrap_or_default()),
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
    Some(format!("It has {work}. Deleting it and {branch} discards them."))
}
