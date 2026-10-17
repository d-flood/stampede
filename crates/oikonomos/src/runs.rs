use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env, fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};

use tokio::{
    process::Command,
    sync::{
        broadcast::{self, error::RecvError},
        mpsc, watch,
    },
};
use uuid::Uuid;

use crate::{
    attention::{Attention, Feed, Source, Tab},
    chats::{self, Chat, Chats, Lifecycle},
    config::{Config, Rig},
    epics::{Author, Epic, Epics, Event, Keep, Status},
    git,
    projects::{Project, Projects},
    templates::{self, RESOLVER, TICKET},
};

mod landing;
mod submodules;

const TAIL: usize = 40;

/// The loop: Runs that start Tickets in their own worktrees, take their Verdicts, and merge them one at a time.
pub struct Runs {
    data_dir: PathBuf,
    worktrees: String,
    cli: String,
    epics: Arc<Epics>,
    chats: Arc<Chats>,
    projects: Arc<Projects>,
    config: watch::Receiver<Arc<Config>>,
    attention: Arc<Feed>,
    starting: Arc<Mutex<HashSet<(String, u32)>>>,
    cutting: tokio::sync::Mutex<()>,
    merges: mpsc::UnboundedSender<String>,
    schedule: mpsc::UnboundedSender<String>,
    views: Mutex<HashMap<Uuid, (Option<String>, Vec<String>)>>,
}

impl Runs {
    /// Matches Claims to the Chats restored before it, then starts the merge queue and the Runs' scheduling,
    /// picking up whatever was left merging, uncleaned or unscheduled when the server last stopped.
    pub fn new(
        data_dir: &Path,
        epics: Arc<Epics>,
        chats: Arc<Chats>,
        projects: Arc<Projects>,
        config: watch::Receiver<Arc<Config>>,
        attention: Arc<Feed>,
    ) -> io::Result<Arc<Self>> {
        let utf8 = |path: PathBuf| path.into_os_string().into_string().map_err(|_| io::Error::other("a path is not UTF-8"));
        let (merges, queued) = mpsc::unbounded_channel();
        let (schedule, scheduled) = mpsc::unbounded_channel();
        let lifecycle = chats.lifecycle();
        let runs = Arc::new(Self {
            data_dir: data_dir.to_owned(),
            worktrees: utf8(fs::canonicalize(data_dir)?.join("worktrees"))?,
            cli: utf8(env::current_exe()?)?,
            epics,
            chats,
            projects,
            config,
            attention,
            starting: Arc::default(),
            cutting: tokio::sync::Mutex::new(()),
            merges,
            schedule,
            views: Mutex::default(),
        });
        runs.reconcile();
        tokio::spawn(Arc::clone(&runs).merge_queued(queued));
        tokio::spawn(Arc::clone(&runs).schedule(scheduled, lifecycle));
        for slug in runs.epics.slugs() {
            let _ = runs.merges.send(slug);
        }
        Ok(runs)
    }

    /// Flags every in-progress Ticket whose Chat did not come back with the server, keeping its Claim and worktree,
    /// and every merging one whose Resolver did not, keeping its merge in progress and the merges after it waiting.
    fn reconcile(&self) {
        let live: HashSet<String> = self.chats.list().into_iter().map(|chat| chat.id).collect();
        for slug in self.epics.slugs() {
            let Ok(epic) = self.epics.get(&slug) else { continue };
            for (&number, ticket) in &epic.tickets {
                let chat = match ticket.status {
                    Status::InProgress if !ticket.releasing => ticket.claim.as_ref().map(|claim| &claim.chat),
                    Status::Merging => ticket.resolver.as_ref(),
                    _ => None,
                };
                let Some(chat) = chat.filter(|chat| !live.contains(*chat)) else { continue };
                if ticket.attention.is_none() {
                    let lost = Event::RestoreFailed { ticket: number, chat: chat.clone() };
                    if let Err(error) = self.epics.record(&slug, lost) {
                        eprintln!("cannot flag {slug}/{number}'s missing Chat: {error}");
                    }
                }
            }
        }
    }

    /// Steps an Epic's Run whenever the Epic changes, a Chat holding one of its Tickets changes, or a start ends,
    /// and rechecks a conflicted merge whenever its Resolver's turn ends or its Chat closes.
    async fn schedule(self: Arc<Self>, mut scheduled: mpsc::UnboundedReceiver<String>, mut lifecycle: broadcast::Receiver<Lifecycle>) {
        let mut changes = self.epics.subscribe();
        let mut called = BTreeMap::new();
        let mut closing = HashSet::new();
        let mut slugs = self.epics.slugs();
        loop {
            for slug in slugs.drain(..) {
                self.release(&slug, &mut closing);
                if self.fill(&slug) {
                    self.finish(&slug).await;
                }
                self.call(&slug, &mut called).await;
            }
            tokio::select! {
                Some(slug) = scheduled.recv() => slugs.push(slug),
                changed = changes.recv() => match changed {
                    Ok(slug) => slugs.push(slug),
                    Err(RecvError::Lagged(_)) => slugs = self.epics.slugs(),
                    Err(RecvError::Closed) => break,
                },
                event = lifecycle.recv() => match event {
                    Ok(event) => {
                        if let Lifecycle::Ended(chat) = &event {
                            closing.remove(&chat.id);
                            self.released(chat);
                        }
                        slugs.extend(self.epics.holding(event.chat()));
                        if matches!(event, Lifecycle::Status(_, chats::Status::Done) | Lifecycle::Ended(_))
                            && let Some(slug) = self.epics.resolving(event.chat())
                        {
                            let _ = self.merges.send(slug);
                        }
                    }
                    Err(RecvError::Lagged(_)) => {
                        closing.clear();
                        slugs = self.epics.slugs();
                        for slug in &slugs {
                            let _ = self.merges.send(slug.clone());
                        }
                    }
                    Err(RecvError::Closed) => break,
                },
            }
        }
    }

    /// Starts Frontier Tickets in number order until the live Run's slots are full, and says whether it has finished.
    /// Every in-progress or merging Ticket holds a slot, whatever its Chat's status.
    fn fill(self: &Arc<Self>, slug: &str) -> bool {
        let Ok(epic) = self.epics.get(slug) else { return false };
        let Some(run) = epic.run.as_ref().filter(|_| !epic.archived) else { return false };
        let mut busy: HashSet<u32> = epic
            .tickets
            .iter()
            .filter(|(_, ticket)| matches!(ticket.status, Status::InProgress | Status::Merging))
            .map(|(&number, _)| number)
            .collect();
        busy.extend(self.starting.lock().unwrap().iter().filter(|(other, _)| other == slug).map(|&(_, number)| number));
        let takeable = epic.frontier().filter(|(number, ticket)| ticket.attention.is_none() && !busy.contains(number));
        let takeable: Vec<u32> = takeable.map(|(number, _)| number).collect();
        if takeable.is_empty() && busy.is_empty() {
            return true;
        }
        if run.paused.is_some() {
            return false;
        }
        for number in takeable.into_iter().take((run.parallel as usize).saturating_sub(busy.len())) {
            let Ok(starting) = Starting::new(self, slug, number) else { continue };
            let (runs, slug, rig) = (Arc::clone(self), slug.to_owned(), run.rig.clone());
            tokio::spawn(async move {
                if let Err(reason) = runs.take(&slug, number, &rig).await {
                    runs.start_failed(&slug, number, reason);
                }
                drop(starting);
            });
        }
        false
    }

    /// Stops the Chat of each Ticket going back to not started, once, or releases the Ticket now when it has none.
    fn release(&self, slug: &str, closing: &mut HashSet<String>) {
        let Ok(epic) = self.epics.get(slug) else { return };
        for (&number, ticket) in epic.tickets.iter().filter(|(_, ticket)| ticket.releasing) {
            let (Some(claim), Some(worktree)) = (&ticket.claim, &ticket.worktree) else { continue };
            if closing.insert(claim.chat.clone()) && !self.chats.end(&claim.chat) {
                closing.remove(&claim.chat);
                self.give_back(slug, number, Keep { worktree: worktree.clone(), agent: None, session: None });
            }
        }
    }

    /// Returns the in-progress Ticket a Chat held once that Chat has stopped, keeping its worktree and Agent session.
    fn released(&self, chat: &Chat) {
        let Some(slug) = self.epics.holding(&chat.id) else { return };
        let Ok(epic) = self.epics.get(&slug) else { return };
        let held = epic.tickets.iter().find(|(_, ticket)| ticket.claim.as_ref().is_some_and(|claim| claim.chat == chat.id));
        let Some((&number, ticket)) = held.filter(|(_, ticket)| ticket.status == Status::InProgress) else { return };
        let Some(worktree) = ticket.worktree.clone() else { return };
        self.give_back(&slug, number, Keep { worktree, agent: Some(chat.agent), session: chat.agent_session.clone() });
    }

    fn give_back(&self, slug: &str, number: u32, keep: Keep) {
        if let Err(error) = self.epics.record(slug, Event::Released { ticket: number, keep }) {
            eprintln!("cannot release {slug}/{number}: {error}");
        }
    }

    /// The Run's step: starts a Ticket with the Run's Rig, in the open Project that checks out the Epic's repository.
    async fn take(&self, slug: &str, number: u32, rig: &str) -> Result<String, String> {
        let project = self.project(&self.epics.get(slug)?).await?;
        let found = self.config.borrow().rigs.iter().find(|other| other.name == rig).cloned();
        let rig = found.ok_or_else(|| format!("The Run's Rig {rig} is no longer in the config, so Oikonomos could not start a Chat."))?;
        self.begin(slug, number, &project, &rig, true).await
    }

    /// Flags a Ticket the live Run could not start, unless its failed Setup Command already has, so the Run moves on.
    fn start_failed(&self, slug: &str, number: u32, reason: String) {
        let unflagged = self.epics.get(slug).is_ok_and(|epic| {
            epic.run.is_some() && epic.tickets.get(&number).is_some_and(|ticket| ticket.attention.is_none())
        });
        if unflagged && let Err(error) = self.epics.record(slug, Event::StartFailed { ticket: number, reason }) {
            eprintln!("cannot flag {slug}/{number}: {error}");
        }
    }

    /// Records that the Run has finished, and pushes to every browser not showing its Epic.
    async fn finish(&self, slug: &str) {
        if let Err(error) = self.epics.record(slug, Event::RunFinished) {
            return eprintln!("cannot finish {slug}'s Run: {error}");
        }
        if let Ok(item) = self.item(slug).await {
            self.attention.call(item, self.showing(slug));
        }
        if self.shown(slug) {
            let _ = self.epics.record(slug, Event::Viewed);
        }
    }

    /// Keeps the attention feed's Epic items in step with the finished Runs no Device has shown yet.
    async fn call(&self, slug: &str, called: &mut BTreeMap<String, Attention>) {
        let finished = self.epics.get(slug).is_ok_and(|epic| epic.finished && !epic.archived);
        if finished == called.contains_key(slug) {
            return;
        }
        if !finished {
            called.remove(slug);
        } else if let Ok(item) = self.item(slug).await {
            called.insert(slug.to_owned(), item);
        } else {
            return;
        }
        self.attention.publish(Source::Epics, called.values().cloned().collect());
    }

    async fn item(&self, slug: &str) -> Result<Attention, String> {
        let project = self.project(&self.epics.get(slug)?).await?;
        let (tab, label) = (Tab::Epic { epic: slug.to_owned() }, format!("Epic {slug}"));
        Ok(Attention { project_id: project.id, tab, label, status: chats::Status::Done })
    }

    /// The open Project that checks out the Epic's repository, its main checkout before any other worktree.
    async fn project(&self, epic: &Epic) -> Result<Project, String> {
        let mut found = None;
        for project in self.projects.list().iter() {
            let path = Path::new(&project.path);
            if git::common_dir(path).await.as_deref() == Some(epic.project.as_path()) {
                if epic.project.parent() == Some(path) {
                    return Ok(project.clone());
                }
                found.get_or_insert_with(|| project.clone());
            }
        }
        found.ok_or_else(|| format!("No open Project checks out {}'s repository, so Oikonomos could not start a Chat.", epic.slug))
    }

    /// Records which Epics a Device shows, and that David has seen any finished Run among them.
    pub fn view(&self, connection: Uuid, browser: Option<String>, slugs: Vec<String>) {
        for slug in &slugs {
            let _ = self.epics.record(slug, Event::Viewed);
        }
        let mut views = self.views.lock().unwrap();
        if slugs.is_empty() {
            views.remove(&connection);
        } else {
            views.insert(connection, (browser, slugs));
        }
    }

    fn shown(&self, slug: &str) -> bool {
        self.views.lock().unwrap().values().any(|(_, shown)| shown.iter().any(|shown| shown == slug))
    }

    fn showing(&self, slug: &str) -> HashSet<String> {
        let views = self.views.lock().unwrap();
        let showing = views.values().filter(|(_, shown)| shown.iter().any(|shown| shown == slug));
        showing.filter_map(|(browser, _)| browser.clone()).collect()
    }

    fn worktree(&self, slug: &str, name: &str) -> String {
        format!("{}/{slug}/{name}", self.worktrees)
    }

    /// Starts a Frontier Ticket by hand, outside any Run.
    pub async fn start(&self, slug: &str, number: u32, project: &Project, rig: &Rig) -> Result<String, String> {
        let _starting = Starting::new(self, slug, number)?;
        self.begin(slug, number, project, rig, false).await
    }

    /// Starts a Frontier Ticket: cuts its worktree and runs the Setup Command there, or takes the worktree its released
    /// Claim kept, then gives it to a new Chat, which resumes the kept session when there is one. A Run's start gives
    /// up if the Run stops before the Claim is recorded.
    async fn begin(&self, slug: &str, number: u32, project: &Project, rig: &Rig, run: bool) -> Result<String, String> {
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
        let live = |epic: &Epic| match &epic.run {
            None if run => Err(format!("{slug}'s Run stopped before {slug}/{number} had its Chat")),
            _ => Ok(()),
        };
        let integration = {
            let _cutting = self.cutting.lock().await;
            match self.epics.get(slug)?.integration {
                Some(integration) => integration.to_string_lossy().into_owned(),
                None => self.cut(&epic, project, &setup, number).await?,
            }
        };
        let kept = ticket.worktree.as_deref().filter(|worktree| worktree.exists());
        let (worktree, branch) = match kept {
            Some(kept) => {
                let name = kept.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
                (kept.to_string_lossy().into_owned(), ticket_branch(slug, &name))
            }
            None => {
                let (worktree, branch) = self.fresh(Path::new(&integration), slug, number).await;
                git::run(Path::new(&integration), &["worktree", "add", "-b", &branch, &worktree, &epic_branch(slug)]).await?;
                if let Err(error) = submodules::init(Path::new(&worktree), Path::new(&integration)).await {
                    discard(&integration, &worktree, &branch).await;
                    return Err(format!("Oikonomos could not check out the submodules of {slug}/{number}'s new worktree: {error}"));
                }
                if let Err(note) = run_setup(&setup, &worktree, "this Ticket's new worktree").await {
                    discard(&integration, &worktree, &branch).await;
                    return Err(self.setup_failed(slug, number, note));
                }
                (worktree, branch)
            }
        };
        let abandon = async |error: String| {
            if kept.is_none() {
                discard(&integration, &worktree, &branch).await;
            }
            Err(error)
        };
        if let Err(error) = live(&self.epics.get(slug)?) {
            return abandon(error).await;
        }
        let resume = ticket.kept.as_ref().filter(|_| kept.is_some());
        let rig = match resume {
            Some(resume) if resume.agent != rig.agent => self.config.borrow().rig_for(resume.agent),
            _ => rig.clone(),
        };
        let chat = match self.chats.create(project, Path::new(&worktree), &rig, resume.map(|resume| resume.session.as_str())).await {
            Ok(chat) => chat,
            Err(error) => return abandon(format!("cannot start a Chat for {slug}/{number}: {error}")).await,
        };
        let claim = Event::Claimed { ticket: number, chat: chat.id.clone(), rig: rig.name.clone(), worktree: PathBuf::from(&worktree) };
        if let Err(error) = self.epics.record_if(slug, claim, live) {
            self.chats.close(&chat.id);
            return abandon(error).await;
        }
        let text = match resume {
            Some(_) => self.carry_on(&epic, number, &branch),
            None => self.prompt(&epic, number, &branch),
        };
        self.chats.send(&chat.id, &text, false, &[]);
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
        if let Err(error) = submodules::init(Path::new(&worktree), checkout).await {
            discard(&project.path, &worktree, &branch).await;
            return Err(format!("Oikonomos could not check out the submodules of {slug}'s new Integration Worktree: {error}"));
        }
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
            "{}\n\nYour Ticket is {reference}, on the branch `{branch}`. {}\n\n## {reference}: {}\n\n{}",
            TICKET.opening(&self.data_dir),
            self.binary(),
            ticket.title,
            ticket.body.trim_end()
        )
    }

    /// What a resumed session is sent in place of the Ticket Template.
    fn carry_on(&self, epic: &Epic, number: u32, branch: &str) -> String {
        let reference = format!("{}/{number}", epic.slug);
        let mut text = format!(
            "Oikonomos stopped this Chat before {reference} had its Verdict, and has resumed it in the same worktree. Carry on where you stopped, and finish with exactly one Verdict.\n\nYour Ticket is {reference}, on the branch `{branch}`. {}",
            self.binary()
        );
        if let Some(sentence) = templates::data_dir_sentence(&self.data_dir) {
            text = format!("{text} {sentence}");
        }
        text
    }

    /// The Resolver Template, then the merge it resolves, which `templates::recognize` shows as what followed `/resolver`.
    fn resolver_prompt(&self, epic: &Epic, number: u32, commit: &str, files: &str) -> String {
        let slug = &epic.slug;
        format!(
            "{}\n\nYour merge is {slug}/{number}, {}: its commit `{commit}` conflicted in {files} when merged into `{}`. {}",
            RESOLVER.opening(&self.data_dir),
            epic.tickets[&number].title,
            epic_branch(slug),
            self.binary()
        )
    }

    fn binary(&self) -> String {
        format!("On this Host `oikonomos` is `{}`; run it by that path if it is not on your PATH.", quote(&self.cli))
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
                let next = epic.queue.first().map(|&number| (number, epic.tickets[&number].resolver.as_deref()));
                let settled = match next {
                    Some((number, None)) => self.merge(&epic, number).await,
                    Some((number, Some(resolver))) if self.resolved(&epic, number, resolver).await => {
                        self.epics.record(&slug, Event::MergeResolved { ticket: number })
                    }
                    _ => match epic.tickets.iter().find(|(_, ticket)| ticket.awaits_cleanup()) {
                        Some((&number, _)) => self.clean(&epic, number).await,
                        None => break,
                    },
                };
                if let Err(error) = settled {
                    eprintln!("cannot settle {slug}: {error}");
                    break;
                }
            }
        }
    }

    /// Merges a completed Ticket into the Integration Worktree, leaving a conflicted merge to a Resolver Chat. The
    /// submodule commits it records are fetched from its Ticket Worktree first, and it is not merged without them.
    async fn merge(&self, epic: &Epic, number: u32) -> Result<(), String> {
        let slug = &epic.slug;
        let ticket = epic.ticket(number)?;
        let integration = epic.integration.as_deref().ok_or_else(|| format!("{slug} has no Integration Worktree"))?;
        let commit = ticket.commit.as_deref().ok_or_else(|| format!("{slug}/{number} has no completed commit"))?;
        let worktree = ticket.worktree.as_deref().ok_or_else(|| format!("{slug}/{number} has no Ticket Worktree"))?;
        let moved = match submodules::carry(worktree, integration, "HEAD", commit).await {
            Ok(moved) => moved,
            Err(why) => {
                let text = format!("Oikonomos did not merge {slug}/{number}, because {why}.");
                self.epics.record(slug, Event::NoteAdded { ticket: number, author: Author::Oikonomos, text })?;
                let reason = "Its submodule commit could not be fetched into the Integration Worktree, so Oikonomos did not merge it; its Note names the submodule and commit.";
                return self.epics.record(slug, Event::MergeFailed { ticket: number, reason: reason.to_owned() });
            }
        };
        let message = format!("Merge {slug}/{number}: {}", ticket.title);
        let error = match git::run(integration, &["merge", "--no-ff", "--no-edit", "-m", &message, commit]).await {
            Ok(_) => {
                if let Err(error) = submodules::sync(integration, &moved).await {
                    let text = format!(
                        "Oikonomos merged {slug}/{number} but could not check out its submodule commits in the Integration Worktree, which shows them as changed until they are: {error}"
                    );
                    self.epics.record(slug, Event::NoteAdded { ticket: number, author: Author::Oikonomos, text })?;
                }
                return self.epics.record(slug, Event::Merged { ticket: number });
            }
            Err(error) => error,
        };
        if git::run(integration, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).await.is_err() {
            let reason = format!("Its merge into the Integration Worktree failed: {error}");
            return self.epics.record(slug, Event::MergeFailed { ticket: number, reason });
        }
        let conflicts = git::run(integration, &["diff", "--name-only", "--diff-filter=U"]).await.unwrap_or_default();
        let files = conflicts.lines().collect::<Vec<_>>().join(", ");
        let Err(error) = self.start_resolver(epic, number, integration, commit, &files).await else { return Ok(()) };
        git::run(integration, &["merge", "--abort"]).await?;
        let reason = format!("Its merge into the Integration Worktree conflicted in {files}, so Oikonomos aborted it. {error}");
        self.epics.record(slug, Event::MergeFailed { ticket: number, reason })
    }

    /// Starts a Resolver Chat in the Integration Worktree with the live Run's Rig, or the Rig its Ticket was started with.
    async fn start_resolver(&self, epic: &Epic, number: u32, integration: &Path, commit: &str, files: &str) -> Result<(), String> {
        let slug = &epic.slug;
        let claim = epic.ticket(number)?.claim.as_ref();
        let name = epic.run.as_ref().map(|run| &run.rig).or(claim.map(|claim| &claim.rig));
        let name = name.ok_or_else(|| format!("{slug}/{number} names no Rig for a Resolver."))?;
        let rig = self.config.borrow().rigs.iter().find(|rig| rig.name == *name).cloned();
        let rig = rig.ok_or_else(|| format!("Its Rig {name} is no longer in the config, so Oikonomos could not start a Resolver."))?;
        let project = self.project(epic).await?;
        let chat = self
            .chats
            .create(&project, integration, &rig, None)
            .await
            .map_err(|error| format!("Its Resolver Chat could not start: {error}."))?;
        if let Err(error) = self.epics.record(slug, Event::MergeConflicted { ticket: number, resolver: chat.id.clone() }) {
            self.chats.close(&chat.id);
            return Err(error);
        }
        self.chats.send(&chat.id, &self.resolver_prompt(epic, number, commit, files), false, &[]);
        Ok(())
    }

    /// Whether the Resolver's turn has ended with the merge committed, HEAD containing the Ticket and nothing left
    /// uncommitted once the submodules are checked out at the commits the merge records.
    async fn resolved(&self, epic: &Epic, number: u32, resolver: &str) -> bool {
        if self.chats.list().iter().any(|chat| chat.id == resolver && chat.status != chats::Status::Done) {
            return false;
        }
        let (Some(integration), Some(commit)) = (epic.integration.as_deref(), epic.tickets[&number].commit.as_deref()) else {
            return false;
        };
        if git::run(integration, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).await.is_ok()
            || git::run(integration, &["merge-base", "--is-ancestor", commit, "HEAD"]).await.is_err()
        {
            return false;
        }
        if let Ok(paths) = submodules::checked_out(integration).await {
            let _ = submodules::sync(integration, &paths).await;
        }
        git::run(integration, &["status", "--porcelain"]).await.is_ok_and(|status| status.is_empty())
    }

    /// Removes a resolved Ticket's worktree and branch, unless that would lose work. The worktree is clean only when
    /// its submodules are too, and is then removed with `--force`, which git requires of a worktree with submodules.
    async fn clean(&self, epic: &Epic, number: u32) -> Result<(), String> {
        let slug = &epic.slug;
        let ticket = epic.ticket(number)?;
        let integration = epic.integration.as_deref().ok_or_else(|| format!("{slug} has no Integration Worktree"))?;
        let worktree = ticket.worktree.as_deref().ok_or_else(|| format!("{slug}/{number} has no Ticket Worktree"))?;
        let name = worktree.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
        let (path, branch) = (worktree.to_string_lossy(), ticket_branch(slug, &name));
        let kept = |reason: String| Event::WorktreeKept { ticket: number, reason };
        let event = match submodules::status(worktree).await {
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
            Ok(_) => match git::run(integration, &["worktree", "remove", "--force", &path]).await {
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

/// Marks a Ticket as starting, so a second start cannot race the first through its Setup Command.
/// The start holds a slot of a live Run until it ends, when the Run is stepped again.
struct Starting {
    starting: Arc<Mutex<HashSet<(String, u32)>>>,
    schedule: mpsc::UnboundedSender<String>,
    key: (String, u32),
}

impl Starting {
    fn new(runs: &Runs, slug: &str, number: u32) -> Result<Self, String> {
        let key = (slug.to_owned(), number);
        if !runs.starting.lock().unwrap().insert(key.clone()) {
            return Err(format!("{slug}/{number} is already starting"));
        }
        Ok(Self { starting: Arc::clone(&runs.starting), schedule: runs.schedule.clone(), key })
    }
}

impl Drop for Starting {
    fn drop(&mut self) {
        self.starting.lock().unwrap().remove(&self.key);
        let _ = self.schedule.send(self.key.0.clone());
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
