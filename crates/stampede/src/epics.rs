use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::broadcast;

use crate::{config::Agent, usage::Window};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Event {
    #[serde(rename = "epic.added")]
    EpicAdded { slug: String, title: String, brief: String, project: PathBuf },
    #[serde(rename = "brief.set")]
    BriefSet { brief: String },
    #[serde(rename = "setup.set")]
    SetupSet { command: String },
    #[serde(rename = "ticket.added")]
    TicketAdded {
        ticket: u32, title: String, body: String, blockers: Vec<u32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<Tag>,
    },
    #[serde(rename = "ticket.tags.set")]
    TagsSet { ticket: u32, tags: Vec<Tag> },
    #[serde(rename = "ticket.rig.set")]
    RigSet { ticket: u32, rig: Option<String> },
    #[serde(rename = "rig.renamed")]
    RigRenamed { original: String, name: String },
    #[serde(rename = "ticket.body.set")]
    BodySet { ticket: u32, body: String },
    #[serde(rename = "blockers.set")]
    BlockersSet { ticket: u32, blockers: Vec<u32> },
    #[serde(rename = "note.added")]
    NoteAdded { ticket: u32, author: Author, text: String },
    #[serde(rename = "epic.archived")]
    Archived,
    #[serde(rename = "branch.cut")]
    BranchCut { base: String, worktree: PathBuf },
    #[serde(rename = "ticket.claimed")]
    Claimed { ticket: u32, chat: String, rig: String, worktree: PathBuf },
    #[serde(rename = "setup.failed")]
    SetupFailed { ticket: u32 },
    #[serde(rename = "ticket.completed")]
    Completed { ticket: u32, commit: String },
    #[serde(rename = "ticket.skipped")]
    Skipped { ticket: u32, reason: String },
    #[serde(rename = "ticket.merged")]
    Merged { ticket: u32 },
    #[serde(rename = "merge.failed")]
    MergeFailed { ticket: u32, reason: String },
    #[serde(rename = "merge.conflicted")]
    MergeConflicted { ticket: u32, resolver: String },
    #[serde(rename = "merge.resolved")]
    MergeResolved { ticket: u32 },
    #[serde(rename = "worktree.removed")]
    WorktreeRemoved { ticket: u32 },
    #[serde(rename = "worktree.kept")]
    WorktreeKept { ticket: u32, reason: String },
    #[serde(rename = "ticket.reopened")]
    Reopened { ticket: u32 },
    #[serde(rename = "start.failed")]
    StartFailed { ticket: u32, reason: String },
    #[serde(rename = "restore.failed")]
    RestoreFailed { ticket: u32, chat: String },
    #[serde(rename = "turn.retried")]
    TurnRetried { ticket: u32, chat: String },
    #[serde(rename = "run.started")]
    RunStarted {
        rig: String,
        parallel: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        reviewers: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remediator: Option<String>,
    },
    #[serde(rename = "run.changed")]
    RunChanged {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rig: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parallel: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reviewers: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remediator: Option<String>,
    },
    #[serde(rename = "run.finished")]
    RunFinished,
    #[serde(rename = "run.paused")]
    RunPaused {
        #[serde(flatten)]
        pause: Pause,
    },
    #[serde(rename = "run.resumed")]
    RunResumed,
    #[serde(rename = "run.stopped")]
    RunStopped,
    #[serde(rename = "claim.released")]
    Released { ticket: u32, keep: Keep },
    #[serde(rename = "turn.cut")]
    TurnCut { ticket: u32, turn: String },
    #[serde(rename = "turn.continued")]
    TurnContinued { ticket: u32 },
    #[serde(rename = "epic.viewed")]
    Viewed,
    #[serde(rename = "epic.landed")]
    Landed { base: String, head: String },
    #[serde(rename = "landing.refused")]
    LandingRefused { reason: String },
    #[serde(rename = "integration.removed")]
    IntegrationRemoved,
    #[serde(rename = "integration.kept")]
    IntegrationKept { reason: String },
    #[serde(rename = "review.started")]
    ReviewStarted,
    #[serde(rename = "reviewer.started")]
    ReviewerStarted { reviewer: u32, rig: String, chat: String, worktree: PathBuf },
    #[serde(rename = "reviewer.failed")]
    ReviewerFailed { reviewer: u32, rig: String, reason: String },
    #[serde(rename = "reviewer.reported")]
    ReviewerReported { reviewer: u32 },
    #[serde(rename = "finding.added")]
    FindingAdded { finding: u32, rig: String, title: String, body: String },
    #[serde(rename = "remediation.started")]
    RemediationStarted { rig: String, chat: String },
    #[serde(rename = "remediation.failed")]
    RemediationFailed { rig: String, reason: String },
    #[serde(rename = "finding.judged")]
    FindingJudged { finding: u32, accepted: bool, reason: String },
    #[serde(rename = "review.remediated")]
    Remediated { commit: String },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "lowercase")]
pub enum Pause {
    David,
    Storage { message: String },
    Usage {
        agent: Agent,
        window: Window,
        #[serde(rename = "resetsAt", default, skip_serializing_if = "Option::is_none")]
        resets_at: Option<u64>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keep {
    pub worktree: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<Agent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub at: u64,
    #[serde(flatten)]
    pub event: Event,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Author {
    Agent,
    David,
    Stampede,
}

impl fmt::Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Author::Agent => "agent",
            Author::David => "david",
            Author::Stampede => "stampede",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    NotStarted,
    InProgress,
    Merging,
    Merged,
    Skipped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Tag {
    Routine,
    Judgment,
    FrontendDesign,
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::Routine => "routine",
            Self::Judgment => "judgment",
            Self::FrontendDesign => "frontend-design",
        })
    }
}

impl Status {
    pub fn resolved(self) -> bool {
        matches!(self, Status::Merged | Status::Skipped)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Status::NotStarted => "not started",
            Status::InProgress => "in progress",
            Status::Merging => "merging",
            Status::Merged => "merged",
            Status::Skipped => "skipped",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Epic {
    pub slug: String,
    pub title: String,
    #[serde(skip)]
    pub project: PathBuf,
    pub brief: String,
    pub setup: Option<String>,
    pub archived: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integration: Option<PathBuf>,
    #[serde(serialize_with = "numbered")]
    pub tickets: BTreeMap<u32, Ticket>,
    /// The merging Tickets still to merge, in the order of their Verdicts.
    #[serde(skip)]
    pub queue: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<Run>,
    #[serde(skip)]
    pub call: Option<Call>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landed: Option<Landed>,
    /// Why the last Landing left the Integration Worktree standing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<Review>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    Finished,
    Paused,
}

/// The base the Epic branch last landed on, and the commit the base reached.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Landed {
    pub base: String,
    pub head: String,
}

/// The live Run: the Rig its next Chats start with, how many Chats it runs at once, the Rigs that review the Epic
/// once its Tickets are done and remediate what they find, and why it starts no Chats.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Run {
    #[serde(skip)]
    pub recovered: BTreeSet<String>,
    pub rig: String,
    pub parallel: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reviewers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused: Option<Pause>,
}

/// The last Run's Review of the Epic branch: its Reviewers, numbered in the order the Run names their Rigs, then
/// the Remediator that judges their Findings.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Review {
    pub reviewers: Vec<Reviewer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediator: Option<Remediator>,
    /// Its Run has finished or stopped.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ended: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Reviewer {
    pub number: u32,
    pub rig: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<PathBuf>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Remediator {
    pub rig: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
    /// The Epic branch's head once it remediated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Finding {
    pub number: u32,
    pub rig: String,
    pub title: String,
    pub body: String,
    pub at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judgment: Option<Judgment>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Judgment {
    pub accepted: bool,
    pub reason: String,
    pub at: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Ticket {
    pub title: String,
    pub body: String,
    pub blockers: Vec<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rig: Option<String>,
    pub status: Status,
    pub notes: Vec<Note>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolver: Option<String>,
    #[serde(skip)]
    pub commit: Option<String>,
    /// Its Chat is being stopped so that its Claim can be released.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub releasing: bool,
    /// The session its released Claim left to resume in its kept worktree.
    #[serde(skip)]
    pub kept: Option<Kept>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    pub agent: Agent,
    pub session: String,
    /// The body changed after the session last read it.
    pub edited: bool,
}

/// The Chat that holds a Ticket, or held it when it gave its Verdict.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Claim {
    pub chat: String,
    pub rig: String,
    /// The body changed after this Claim was made, and nothing told its Chat.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub edited: bool,
    #[serde(skip)]
    pub continuation: Option<Continuation>,
    #[serde(skip)]
    pub cut_turn: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Continuation {
    Pending,
    Due,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Note {
    pub author: Author,
    pub text: String,
    pub at: u64,
}

fn numbered<S: Serializer>(tickets: &BTreeMap<u32, Ticket>, serializer: S) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Numbered<'a> {
        number: u32,
        #[serde(flatten)]
        ticket: &'a Ticket,
    }
    serializer.collect_seq(tickets.iter().map(|(&number, ticket)| Numbered { number, ticket }))
}

/// Folds an Epic's log into its current state, or nothing when the log does not start with `epic.added`.
pub fn fold<'a>(entries: impl IntoIterator<Item = &'a Entry>) -> Option<Epic> {
    let mut entries = entries.into_iter();
    let mut epic = Epic::added(entries.next()?)?;
    for entry in entries {
        epic.apply(entry);
    }
    Some(epic)
}

impl Epic {
    fn added(entry: &Entry) -> Option<Self> {
        let Event::EpicAdded { slug, title, brief, project } = &entry.event else { return None };
        Some(Self {
            slug: slug.clone(),
            title: title.clone(),
            project: project.clone(),
            brief: brief.clone(),
            setup: None,
            archived: false,
            base: None,
            integration: None,
            tickets: BTreeMap::new(),
            queue: Vec::new(),
            run: None,
            call: None,
            landed: None,
            kept: None,
            notes: Vec::new(),
            review: None,
            findings: Vec::new(),
        })
    }

    fn apply(&mut self, entry: &Entry) {
        match &entry.event {
            Event::BriefSet { brief } => self.brief = brief.clone(),
            Event::SetupSet { command } => self.setup = Some(command.clone()),
            Event::TicketAdded { ticket, title, body, blockers, tags } => {
                self.tickets.entry(*ticket).or_insert_with(|| Ticket {
                    title: title.clone(),
                    body: body.clone(),
                    blockers: blockers.clone(),
                    tags: tags.clone(),
                    rig: None,
                    status: Status::NotStarted,
                    notes: Vec::new(),
                    claim: None,
                    worktree: None,
                    attention: None,
                    resolver: None,
                    commit: None,
                    releasing: false,
                    kept: None,
                });
            }
            Event::TagsSet { ticket, tags } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.tags = tags.clone();
                }
            }
            Event::RigSet { ticket, rig } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.rig = rig.clone();
                    ticket.attention = None;
                }
            }
            Event::BodySet { ticket, body } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.body = body.clone();
                    if let Some(claim) = &mut ticket.claim {
                        claim.edited = true;
                    } else if let Some(kept) = &mut ticket.kept {
                        kept.edited = true;
                    }
                }
            }
            Event::BlockersSet { ticket, blockers } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.blockers = blockers.clone();
                }
            }
            Event::NoteAdded { ticket, author, text } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.notes.push(Note { author: *author, text: text.clone(), at: entry.at });
                }
            }
            Event::Archived => self.archived = true,
            Event::BranchCut { base, worktree } => {
                self.base = Some(base.clone());
                self.integration = Some(worktree.clone());
                self.landed = None;
                self.kept = None;
            }
            Event::Claimed { ticket, chat, rig, worktree } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    let resumed = ticket.kept.take().filter(|_| ticket.worktree.as_ref() == Some(worktree));
                    let edited = resumed.is_some_and(|kept| kept.edited);
                    ticket.status = Status::InProgress;
                    ticket.claim = Some(Claim { chat: chat.clone(), rig: rig.clone(), edited, continuation: None, cut_turn: None });
                    ticket.worktree = Some(worktree.clone());
                    ticket.attention = None;
                }
            }
            Event::SetupFailed { ticket } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.attention = Some("The Setup Command failed; its Note has the end of the output.".to_owned());
                }
            }
            Event::Completed { ticket: number, commit } => {
                if let Some(ticket) = self.tickets.get_mut(number) {
                    ticket.status = Status::Merging;
                    ticket.commit = Some(commit.clone());
                    ticket.releasing = false;
                    self.queue.push(*number);
                }
            }
            Event::Skipped { ticket, reason } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.status = Status::Skipped;
                    ticket.releasing = false;
                    ticket.notes.push(Note { author: Author::Agent, text: reason.clone(), at: entry.at });
                }
            }
            Event::Merged { ticket: number } | Event::MergeResolved { ticket: number } => {
                self.queue.retain(|queued| queued != number);
                if let Some(ticket) = self.tickets.get_mut(number) {
                    ticket.status = Status::Merged;
                    ticket.attention = None;
                    ticket.resolver = None;
                }
            }
            Event::MergeFailed { ticket: number, reason } => {
                self.queue.retain(|queued| queued != number);
                if let Some(ticket) = self.tickets.get_mut(number) {
                    ticket.attention = Some(reason.clone());
                }
            }
            Event::MergeConflicted { ticket, resolver } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.resolver = Some(resolver.clone());
                }
            }
            Event::WorktreeRemoved { ticket } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.worktree = None;
                    if ticket.status.resolved() {
                        ticket.attention = None;
                    }
                }
            }
            Event::WorktreeKept { ticket, reason } | Event::StartFailed { ticket, reason } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.attention = Some(reason.clone());
                }
            }
            Event::Reopened { ticket } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    if ticket.status == Status::InProgress {
                        ticket.releasing = true;
                    } else {
                        if let Some(kept) = ticket.attention.take() {
                            ticket.notes.push(Note { author: Author::Stampede, text: kept, at: entry.at });
                        }
                        ticket.status = Status::NotStarted;
                        ticket.claim = None;
                        ticket.worktree = None;
                        ticket.commit = None;
                        ticket.kept = None;
                    }
                }
            }
            Event::Released { ticket, keep } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    let edited = ticket.claim.take().is_some_and(|claim| claim.edited);
                    let session = keep.agent.zip(keep.session.clone());
                    ticket.kept = session.map(|(agent, session)| Kept { agent, session, edited });
                    ticket.status = Status::NotStarted;
                    ticket.worktree = Some(keep.worktree.clone());
                    ticket.attention = None;
                    ticket.releasing = false;
                }
            }
            Event::TurnCut { ticket, turn } => {
                if let Some(claim) = self.tickets.get_mut(ticket).and_then(|ticket| ticket.claim.as_mut()) {
                    claim.continuation = self.run.as_ref().filter(|run| run.paused != Some(Pause::David)).map(|_| Continuation::Pending);
                    claim.cut_turn = Some(turn.clone());
                }
            }
            Event::TurnContinued { ticket } => {
                if let Some(claim) = self.tickets.get_mut(ticket).and_then(|ticket| ticket.claim.as_mut()) {
                    claim.continuation = None;
                }
            }
            Event::RestoreFailed { ticket, chat } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.attention = Some(if ticket.resolver.as_ref() == Some(chat) {
                        format!(
                            "Its Resolver Chat {chat} did not come back when Stampede restarted, so its merge waits in the Integration Worktree, and later merges wait for it."
                        )
                    } else {
                        format!(
                            "Its Chat {chat} did not come back when Stampede restarted, so its Claim and worktree are kept and no other Chat takes it."
                        )
                    });
                }
            }
            Event::TurnRetried { chat, .. } => {
                if let Some(run) = &mut self.run { run.recovered.insert(chat.clone()); }
            }
            Event::RunStarted { rig, parallel, reviewers, remediator } => {
                self.run = Some(Run {
                    recovered: BTreeSet::new(),
                    rig: rig.clone(),
                    parallel: *parallel,
                    reviewers: reviewers.clone(),
                    remediator: remediator.clone(),
                    paused: None,
                });
                self.call = None;
                for ticket in self.tickets.values_mut().filter(|ticket| ticket.status == Status::NotStarted) {
                    ticket.attention = None;
                }
            }
            Event::RunChanged { rig, parallel, reviewers, remediator } => {
                if let Some(run) = &mut self.run {
                    run.rig = rig.clone().unwrap_or_else(|| run.rig.clone());
                    run.parallel = parallel.unwrap_or(run.parallel);
                    run.reviewers = reviewers.clone().unwrap_or_else(|| run.reviewers.clone());
                    run.remediator = remediator.clone().or_else(|| run.remediator.clone());
                }
            }
            Event::RigRenamed { original, name } => {
                if let Some(run) = &mut self.run {
                    for rig in [&mut run.rig].into_iter().chain(&mut run.reviewers).chain(&mut run.remediator) {
                        if *rig == *original {
                            *rig = name.clone();
                        }
                    }
                }
                for ticket in self.tickets.values_mut() {
                    if ticket.rig.as_ref() == Some(original) {
                        ticket.rig = Some(name.clone());
                    }
                    if let Some(claim) = &mut ticket.claim
                        && claim.rig == *original
                    {
                        claim.rig = name.clone();
                    }
                }
            }
            Event::RunFinished => {
                self.run = None;
                self.call = Some(Call::Finished);
                self.end_review();
            }
            Event::RunPaused { pause } => {
                let Some(run) = &mut self.run else { return };
                let paused = run.paused.replace(pause.clone());
                if *pause == Pause::David {
                    self.unpause(None);
                } else if paused.is_none() {
                    self.call = Some(Call::Paused);
                }
            }
            Event::RunResumed => {
                let Some(run) = &mut self.run else { return };
                if let Some(Pause::Usage { .. } | Pause::Storage { .. }) = run.paused.take() {
                    self.unpause(Some(Continuation::Due));
                }
            }
            Event::RunStopped => {
                self.run = None;
                self.unpause(None);
                self.end_review();
                for ticket in self.tickets.values_mut().filter(|ticket| ticket.status == Status::InProgress) {
                    ticket.releasing = true;
                }
            }
            Event::Viewed => self.call = None,
            Event::Landed { base, head } => self.landed = Some(Landed { base: base.clone(), head: head.clone() }),
            Event::LandingRefused { reason } => {
                self.notes.push(Note { author: Author::Stampede, text: format!("Landing was refused: {reason}."), at: entry.at });
            }
            Event::IntegrationRemoved => {
                self.integration = None;
                self.kept = None;
            }
            Event::IntegrationKept { reason } => self.kept = Some(reason.clone()),
            Event::ReviewStarted => self.review = Some(Review { reviewers: Vec::new(), remediator: None, ended: false }),
            Event::ReviewerStarted { reviewer, rig, chat, worktree } => {
                if let Some(review) = &mut self.review {
                    review.reviewers.push(Reviewer {
                        number: *reviewer,
                        rig: rig.clone(),
                        chat: Some(chat.clone()),
                        worktree: Some(worktree.clone()),
                        reported: false,
                        failed: None,
                    });
                }
            }
            Event::ReviewerFailed { reviewer: number, rig, reason } => {
                let Some(review) = &mut self.review else { return };
                match review.reviewers.iter_mut().find(|reviewer| reviewer.number == *number) {
                    Some(reviewer) => reviewer.failed = Some(reason.clone()),
                    None => review.reviewers.push(Reviewer {
                        number: *number,
                        rig: rig.clone(),
                        chat: None,
                        worktree: None,
                        reported: false,
                        failed: Some(reason.clone()),
                    }),
                }
            }
            Event::ReviewerReported { reviewer: number } => {
                let reviewers = self.review.iter_mut().flat_map(|review| &mut review.reviewers);
                if let Some(reviewer) = reviewers.into_iter().find(|reviewer| reviewer.number == *number) {
                    reviewer.reported = true;
                }
            }
            Event::FindingAdded { finding, rig, title, body } => self.findings.push(Finding {
                number: *finding,
                rig: rig.clone(),
                title: title.clone(),
                body: body.clone(),
                at: entry.at,
                judgment: None,
            }),
            Event::RemediationStarted { rig, chat } => {
                if let Some(review) = &mut self.review {
                    review.remediator = Some(Remediator { rig: rig.clone(), chat: Some(chat.clone()), commit: None, failed: None });
                }
            }
            Event::RemediationFailed { rig, reason } => {
                if let Some(review) = &mut self.review {
                    let remediator = review.remediator.get_or_insert_with(|| Remediator { rig: rig.clone(), chat: None, commit: None, failed: None });
                    remediator.failed = Some(reason.clone());
                }
            }
            Event::FindingJudged { finding, accepted, reason } => {
                if let Some(finding) = self.findings.iter_mut().find(|other| other.number == *finding) {
                    finding.judgment = Some(Judgment { accepted: *accepted, reason: reason.clone(), at: entry.at });
                }
            }
            Event::Remediated { commit } => {
                if let Some(remediator) = self.review.as_mut().and_then(|review| review.remediator.as_mut()) {
                    remediator.commit = Some(commit.clone());
                }
            }
            Event::EpicAdded { .. } | Event::Unknown => {}
        }
    }

    fn end_review(&mut self) {
        if let Some(review) = &mut self.review {
            review.ended = true;
        }
    }

    fn unpause(&mut self, owed: Option<Continuation>) {
        if self.call == Some(Call::Paused) {
            self.call = None;
        }
        for claim in self.tickets.values_mut().filter_map(|ticket| ticket.claim.as_mut()) {
            if claim.continuation.is_some() {
                claim.continuation = owed;
            }
        }
    }

    pub fn check(&self, event: &Event) -> Result<(), String> {
        let slug = &self.slug;
        match event {
            Event::BriefSet { brief } => filled(brief, "Brief"),
            Event::TicketAdded { ticket, title, body, blockers, tags } => {
                if self.tickets.contains_key(ticket) {
                    return Err(format!("{slug}/{ticket} already exists"));
                }
                one_line(title)?;
                filled(body, "body")?;
                check_tags(tags)?;
                if blockers.contains(ticket) {
                    return Err(format!("{slug}/{ticket} cannot block itself"));
                }
                match blockers.iter().find(|blocker| !self.tickets.contains_key(blocker)) {
                    Some(missing) => Err(format!("{slug} has no Ticket {missing} to block {slug}/{ticket}; add Tickets in dependency order")),
                    None => Ok(()),
                }
            }
            Event::TagsSet { ticket, tags } => {
                self.ticket(*ticket)?;
                check_tags(tags)
            }
            Event::RigSet { ticket, rig } => {
                let status = self.ticket(*ticket)?.status;
                if status != Status::NotStarted {
                    return Err(format!("{slug}/{ticket} is {status}; its Rig can change only while not started"));
                }
                rig.as_ref().map_or(Ok(()), |rig| filled(rig, "Rig"))
            }
            Event::BodySet { ticket, body } => {
                self.ticket(*ticket)?;
                filled(body, "body")
            }
            Event::BlockersSet { ticket, blockers } => {
                self.ticket(*ticket)?;
                if blockers.contains(ticket) {
                    return Err(format!("{slug}/{ticket} cannot block itself"));
                }
                if let Some(missing) = blockers.iter().find(|blocker| !self.tickets.contains_key(blocker)) {
                    return Err(format!("{slug} has no Ticket {missing} to block {slug}/{ticket}"));
                }
                let mut seen = BTreeSet::new();
                match blockers.iter().find_map(|&blocker| self.chain(blocker, *ticket, &mut seen)) {
                    Some(chain) => {
                        let chain: Vec<_> = chain.iter().map(|number| format!("{slug}/{number}")).collect();
                        Err(format!(
                            "those Blockers would make a cycle: {slug}/{ticket} is blocked by {}",
                            chain.join(", which is blocked by ")
                        ))
                    }
                    None => Ok(()),
                }
            }
            Event::SetupFailed { ticket } => self.ticket(*ticket).map(drop),
            Event::WorktreeRemoved { ticket } => match self.ticket(*ticket)?.status {
                status @ (Status::InProgress | Status::Merging) => Err(format!("{slug}/{ticket} is {status} in its worktree")),
                _ => Ok(()),
            },
            Event::WorktreeKept { ticket, .. } => {
                let status = self.ticket(*ticket)?.status;
                if status.resolved() { Ok(()) } else { Err(format!("{slug}/{ticket} is {status}, not merged or skipped")) }
            }
            Event::NoteAdded { ticket, text, .. } => {
                self.ticket(*ticket)?;
                filled(text, "Note")
            }
            Event::BranchCut { .. } if self.integration.is_some() => Err(format!("{slug} already has its Epic branch")),
            Event::Claimed { ticket, .. } => {
                if self.frontier().any(|(number, _)| number == *ticket) {
                    Ok(())
                } else {
                    Err(format!("{slug}/{ticket} is not on the Frontier: it is {}", self.ticket(*ticket)?.status))
                }
            }
            Event::Completed { ticket, .. } | Event::Skipped { ticket, .. } => {
                let status = self.ticket(*ticket)?.status;
                if status == Status::InProgress {
                    Ok(())
                } else {
                    Err(format!("{slug}/{ticket} is {status}, not in progress, so it takes no Verdict"))
                }
            }
            Event::Merged { ticket } | Event::MergeFailed { ticket, .. } => {
                let status = self.ticket(*ticket)?.status;
                if status == Status::Merging { Ok(()) } else { Err(format!("{slug}/{ticket} is {status}, not merging")) }
            }
            Event::Reopened { ticket: number } => {
                let ticket = self.ticket(*number)?;
                match ticket.status {
                    Status::InProgress if ticket.releasing => Err(format!("{slug}/{number}'s Chat is already stopping")),
                    Status::InProgress | Status::Merged | Status::Skipped => Ok(()),
                    status => Err(format!(
                        "{slug}/{number} is {status}; only an in-progress, merged or skipped Ticket can be moved back to not started"
                    )),
                }
            }
            Event::Released { ticket, .. } => {
                let status = self.ticket(*ticket)?.status;
                if status == Status::InProgress {
                    Ok(())
                } else {
                    Err(format!("{slug}/{ticket} is {status}, so it has no Claim to release"))
                }
            }
            Event::RestoreFailed { ticket, chat } if self.ticket(*ticket)?.resolver.as_ref() == Some(chat) => Ok(()),
            Event::StartFailed { ticket, .. } | Event::RestoreFailed { ticket, .. } => {
                let status = self.ticket(*ticket)?.status;
                let expected = if matches!(event, Event::StartFailed { .. }) { Status::NotStarted } else { Status::InProgress };
                if status == expected { Ok(()) } else { Err(format!("{slug}/{ticket} is {status}")) }
            }
            Event::TurnRetried { ticket, chat } => {
                let held = self.ticket(*ticket)?;
                if !self.archived && self.run.as_ref().is_some_and(|run| run.paused.is_none() && !run.recovered.contains(chat))
                    && held.status == Status::InProgress && !held.releasing
                    && held.claim.as_ref().is_some_and(|claim| &claim.chat == chat)
                { Ok(()) } else { Err(format!("{slug}/{ticket} cannot retry its interrupted turn")) }
            }
            Event::RunStarted { .. } if self.run.is_some() => Err(format!("{slug} already has a live Run")),
            Event::RunStarted { .. } if self.setup.is_none() => Err(format!(
                "{slug} has no Setup Command yet; record one, \"\" when a fresh worktree needs nothing, before starting a Run"
            )),
            Event::RunStarted { parallel: 0, .. } | Event::RunChanged { parallel: Some(0), .. } => {
                Err("a Run works on at least one Ticket at a time".to_owned())
            }
            Event::RunStarted { reviewers, remediator: None, .. } if !reviewers.is_empty() => Err(UNREMEDIATED.to_owned()),
            Event::RunChanged { .. } | Event::RunFinished | Event::RunPaused { .. } | Event::RunResumed | Event::RunStopped
                if self.run.is_none() =>
            {
                Err(format!("{slug} has no live Run"))
            }
            Event::RunChanged { reviewers: Some(_), .. } if self.reviewing().is_some() => {
                Err(format!("{slug}'s Review has begun, so its Reviewers are settled"))
            }
            Event::RunChanged { remediator: Some(_), .. } if self.reviewing().is_some_and(|review| review.remediator.is_some()) => {
                Err(format!("{slug}'s Remediator has started"))
            }
            Event::RunChanged { reviewers: Some(reviewers), remediator, .. }
                if !reviewers.is_empty() && remediator.is_none() && self.run.as_ref().is_some_and(|run| run.remediator.is_none()) =>
            {
                Err(UNREMEDIATED.to_owned())
            }
            Event::RunPaused { pause } if self.run.as_ref().is_some_and(|run| run.paused.as_ref() == Some(pause)) => {
                Err(format!("{slug}'s Run is already paused"))
            }
            Event::RunPaused { pause: Pause::Usage { .. } | Pause::Storage { .. } }
                if self.run.as_ref().is_some_and(|run| run.paused == Some(Pause::David)) =>
            {
                Err(format!("David paused {slug}'s Run"))
            }
            Event::RunResumed if self.run.as_ref().is_some_and(|run| run.paused.is_none()) => {
                Err(format!("{slug}'s Run is not paused"))
            }
            Event::TurnCut { .. } if self.run.is_none() => Err(format!("{slug} has no live Run to continue its Chat")),
            Event::TurnCut { ticket, .. } | Event::TurnContinued { ticket } => {
                let held = self.ticket(*ticket)?;
                let owed = held.claim.as_ref().is_some_and(|claim| claim.continuation.is_some());
                if let Event::TurnCut { turn, .. } = event
                    && held.claim.as_ref().is_some_and(|claim| claim.cut_turn.as_ref() == Some(turn))
                {
                    return Err(format!("{slug}/{ticket}'s cut-off turn was recorded already"));
                }
                match (held.status, matches!(event, Event::TurnCut { .. }), owed) {
                    (Status::InProgress, true, _) | (Status::InProgress, false, true) => Ok(()),
                    (Status::InProgress, false, false) => Err(format!("{slug}/{ticket}'s Chat is owed no continue")),
                    (status, ..) => Err(format!("{slug}/{ticket} is {status}, so it has no Chat to continue")),
                }
            }
            Event::Viewed if self.call.is_none() => Err(format!("{slug} has no finished or paused Run to show")),
            Event::MergeConflicted { ticket, .. } => {
                if self.ticket(*ticket)?.resolver.is_some() {
                    Err(format!("{slug}/{ticket} already has a Resolver"))
                } else if self.queue.first() != Some(ticket) {
                    Err(format!("{slug}/{ticket} is not the next Ticket to merge"))
                } else {
                    Ok(())
                }
            }
            Event::MergeResolved { ticket } => match self.ticket(*ticket)?.resolver {
                Some(_) => Ok(()),
                None => Err(format!("{slug}/{ticket} has no Resolver")),
            },
            Event::Landed { .. } | Event::IntegrationRemoved | Event::IntegrationKept { .. } if self.integration.is_none() => {
                Err(format!("{slug} has no Integration Worktree"))
            }
            Event::LandingRefused { reason } => filled(reason, "reason"),
            Event::ReviewStarted => {
                let run = self.run.as_ref().ok_or_else(|| format!("{slug} has no live Run"))?;
                if self.reviewing().is_some() {
                    Err(format!("{slug}'s Run is already reviewing"))
                } else if run.reviewers.is_empty() {
                    Err(format!("{slug}'s Run names no Reviewers"))
                } else if self.integration.is_none() {
                    Err(format!("{slug} has no Integration Worktree to review"))
                } else if let Some((number, ticket)) = self.tickets.iter().find(|(_, ticket)| !ticket.status.resolved()) {
                    Err(format!("{slug}/{number} is {}, and a Review waits for every Ticket", ticket.status))
                } else {
                    Ok(())
                }
            }
            Event::ReviewerStarted { reviewer, .. } | Event::ReviewerFailed { reviewer, .. } => {
                let review = self.reviewing().ok_or_else(|| format!("{slug} is not reviewing"))?;
                let named = self.run.as_ref().map_or(0, |run| run.reviewers.len());
                match review.reviewers.iter().find(|other| other.number == *reviewer) {
                    _ if *reviewer == 0 || *reviewer as usize > named => Err(format!("{slug}'s Run names no Reviewer {reviewer}")),
                    Some(_) if matches!(event, Event::ReviewerStarted { .. }) => Err(format!("{slug}'s Reviewer {reviewer} has started")),
                    Some(other) if other.done() => Err(format!("{slug}'s Reviewer {reviewer} is done")),
                    _ => Ok(()),
                }
            }
            Event::ReviewerReported { reviewer } => {
                let review = self.reviewing().ok_or_else(|| format!("{slug} is not reviewing"))?;
                match review.reviewers.iter().find(|other| other.number == *reviewer) {
                    Some(other) if !other.done() => Ok(()),
                    Some(_) => Err(format!("{slug}'s Reviewer {reviewer} is done")),
                    None => Err(format!("{slug}'s Reviewer {reviewer} has not started")),
                }
            }
            Event::FindingAdded { finding, title, body, .. } => {
                self.reviewing().ok_or_else(|| format!("{slug} is not reviewing"))?;
                if self.findings.iter().any(|other| other.number == *finding) {
                    return Err(format!("{slug} already has Finding {finding}"));
                }
                one_line(title)?;
                filled(body, "body")
            }
            Event::RemediationStarted { .. } => {
                let review = self.reviewing().ok_or_else(|| format!("{slug} is not reviewing"))?;
                let named = self.run.as_ref().map_or(0, |run| run.reviewers.len());
                if review.remediator.is_some() {
                    Err(format!("{slug}'s Remediator has started"))
                } else if review.reviewers.len() < named || review.reviewers.iter().any(|reviewer| !reviewer.done()) {
                    Err(format!("{slug}'s Reviewers have not all reported"))
                } else {
                    Ok(())
                }
            }
            Event::RemediationFailed { .. } => match self.reviewing().ok_or_else(|| format!("{slug} is not reviewing"))?.remediator {
                Some(ref remediator) if remediator.done() => Err(format!("{slug}'s Remediator is done")),
                _ => Ok(()),
            },
            Event::FindingJudged { finding, reason, .. } => {
                self.remediating()?;
                if !self.findings.iter().any(|other| other.number == *finding) {
                    return Err(format!("{slug} has no Finding {finding}"));
                }
                filled(reason, "reason")
            }
            Event::Remediated { .. } => {
                self.remediating()?;
                match self.findings.iter().find(|finding| finding.judgment.is_none()) {
                    Some(finding) => Err(format!("{slug}'s Finding {} is not judged yet; accept or reject every Finding first", finding.number)),
                    None => Ok(()),
                }
            }
            _ => Ok(()),
        }
    }

    /// The live Run's Review, once it has begun.
    pub fn reviewing(&self) -> Option<&Review> {
        self.review.as_ref().filter(|review| !review.ended)
    }

    /// The live Remediator, which judges Findings until it has remediated.
    fn remediating(&self) -> Result<&Remediator, String> {
        let remediator = self.reviewing().and_then(|review| review.remediator.as_ref());
        remediator.filter(|remediator| remediator.chat.is_some() && !remediator.done()).ok_or_else(|| format!("{} is not remediating", self.slug))
    }

    fn chain(&self, from: u32, to: u32, seen: &mut BTreeSet<u32>) -> Option<Vec<u32>> {
        if from == to {
            return Some(vec![to]);
        }
        if !seen.insert(from) {
            return None;
        }
        self.tickets.get(&from)?.blockers.iter().find_map(|&next| {
            let mut chain = self.chain(next, to, seen)?;
            chain.insert(0, from);
            Some(chain)
        })
    }

    pub fn ticket(&self, number: u32) -> Result<&Ticket, String> {
        self.tickets.get(&number).ok_or_else(|| format!("{} has no Ticket {number}", self.slug))
    }

    pub fn frontier(&self) -> impl Iterator<Item = (u32, &Ticket)> {
        let resolved = |blocker: &u32| self.tickets.get(blocker).is_some_and(|ticket| ticket.status.resolved());
        self.tickets
            .iter()
            .filter(move |(_, ticket)| {
                ticket.status == Status::NotStarted && ticket.claim.is_none() && ticket.blockers.iter().all(resolved)
            })
            .map(|(number, ticket)| (*number, ticket))
    }
}

impl Reviewer {
    pub fn done(&self) -> bool {
        self.reported || self.failed.is_some()
    }
}

impl Remediator {
    pub fn done(&self) -> bool {
        self.commit.is_some() || self.failed.is_some()
    }
}

const UNREMEDIATED: &str = "a Run with Reviewers needs a Remediator to judge their Findings";

impl Ticket {
    /// Resolved, with a worktree that cleanup has neither removed nor kept.
    pub fn awaits_cleanup(&self) -> bool {
        self.status.resolved() && self.worktree.is_some() && self.attention.is_none()
    }
}

fn one_line(title: &str) -> Result<(), String> {
    if title.trim().is_empty() {
        Err("the title is empty".to_owned())
    } else if title.contains('\n') {
        Err("the title must be one line".to_owned())
    } else {
        Ok(())
    }
}

fn check_tags(tags: &[Tag]) -> Result<(), String> {
    if tags.iter().collect::<BTreeSet<_>>().len() != tags.len() {
        return Err("Ticket tags must be unique".to_owned());
    }
    Ok(())
}

fn filled(text: &str, what: &str) -> Result<(), String> {
    if text.trim().is_empty() { Err(format!("the {what} is empty")) } else { Ok(()) }
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && slug.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Every Epic, each kept as an append-only log at `epics/<slug>.jsonl` in the data directory.
pub struct Epics {
    dir: PathBuf,
    epics: Mutex<BTreeMap<String, Epic>>,
    /// Held from an event's check until it applies, so readers of `epics` never wait on the write.
    appending: Mutex<()>,
    changes: broadcast::Sender<String>,
}

impl Epics {
    pub fn load(data_dir: &Path) -> io::Result<Self> {
        let dir = data_dir.join("epics");
        fs::create_dir_all(&dir)?;
        let mut epics = BTreeMap::new();
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension() != Some("jsonl".as_ref()) {
                continue;
            }
            match read(&path).map(|entries| fold(&entries)) {
                Ok(Some(epic)) => {
                    epics.insert(epic.slug.clone(), epic);
                }
                Ok(None) => eprintln!("{} records no Epic", path.display()),
                Err(error) => eprintln!("cannot read {}: {error}", path.display()),
            }
        }
        Ok(Self { dir, epics: Mutex::new(epics), appending: Mutex::default(), changes: broadcast::channel(64).0 })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.changes.subscribe()
    }

    fn path(&self, slug: &str) -> PathBuf {
        self.dir.join(format!("{slug}.jsonl"))
    }

    pub fn get(&self, slug: &str) -> Result<Epic, String> {
        self.epics.lock().unwrap().get(slug).cloned().ok_or_else(|| format!("there is no Epic {slug}"))
    }

    pub fn slugs(&self) -> Vec<String> {
        self.epics.lock().unwrap().keys().cloned().collect()
    }

    /// Whether any Epic has a Run, paused or not, that is not archived.
    pub fn live(&self) -> bool {
        self.epics.lock().unwrap().values().any(|epic| epic.run.is_some() && !epic.archived)
    }

    pub fn rename_rig(&self, original: &str, name: &str) -> Result<(), String> {
        for slug in self.slugs() {
            let epic = self.get(&slug)?;
            let names = |run: &Run| [&run.rig].into_iter().chain(&run.reviewers).chain(&run.remediator).any(|rig| rig == original);
            if epic.run.as_ref().is_some_and(names)
                || epic.tickets.values().any(|ticket| ticket.rig.as_deref() == Some(original)
                    || ticket.claim.as_ref().is_some_and(|claim| claim.rig == original))
            {
                self.record(&slug, Event::RigRenamed { original: original.to_owned(), name: name.to_owned() })?;
            }
        }
        Ok(())
    }

    /// The Epic with a Ticket claimed by `chat`.
    pub fn holding(&self, chat: &str) -> Option<String> {
        let epics = self.epics.lock().unwrap();
        let mut claims = epics.values().flat_map(|epic| epic.tickets.values().map(move |ticket| (epic, ticket)));
        claims.find(|(_, ticket)| ticket.claim.as_ref().is_some_and(|claim| claim.chat == chat)).map(|(epic, _)| epic.slug.clone())
    }

    /// The Epic whose conflicted merge `chat` is resolving.
    pub fn resolving(&self, chat: &str) -> Option<String> {
        let epics = self.epics.lock().unwrap();
        let mut resolvers = epics.values().flat_map(|epic| epic.tickets.values().map(move |ticket| (epic, ticket)));
        resolvers.find(|(_, ticket)| ticket.resolver.as_deref() == Some(chat)).map(|(epic, _)| epic.slug.clone())
    }

    /// The Epic whose live Review has `chat` as a Reviewer or its Remediator.
    pub fn reviewing(&self, chat: &str) -> Option<String> {
        let epics = self.epics.lock().unwrap();
        epics.values().find(|epic| {
            epic.reviewing().is_some_and(|review| {
                let reviewers = review.reviewers.iter().filter_map(|reviewer| reviewer.chat.as_deref());
                reviewers.chain(review.remediator.iter().filter_map(|remediator| remediator.chat.as_deref())).any(|other| other == chat)
            })
        }).map(|epic| epic.slug.clone())
    }

    pub fn list(&self, project: &Path) -> Vec<Epic> {
        self.epics.lock().unwrap().values().filter(|epic| epic.project == project).cloned().collect()
    }

    pub fn add(&self, slug: &str, title: &str, brief: &str, project: &Path) -> Result<(), String> {
        if !valid_slug(slug) {
            return Err(format!("{slug:?} is not a valid slug; use lowercase letters, digits and hyphens, such as chat-ui"));
        }
        one_line(title)?;
        tokio::task::block_in_place(|| {
            let _appending = self.appending.lock().unwrap();
            if let Some(existing) = self.epics.lock().unwrap().get(slug) {
                let archived = if existing.archived { ", archived" } else { "" };
                return Err(format!(
                    "Epic {slug} already exists in {}{archived}; slugs are unique across every Project",
                    existing.project.display()
                ));
            }
            let event = Event::EpicAdded {
                slug: slug.to_owned(),
                title: title.to_owned(),
                brief: brief.to_owned(),
                project: project.to_owned(),
            };
            let entry = stamp(event);
            let path = self.path(slug);
            OpenOptions::new()
                .append(true)
                .create_new(true)
                .open(&path)
                .and_then(|file| append(file, &entry))
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
            self.epics.lock().unwrap().insert(slug.to_owned(), Epic::added(&entry).expect("an epic.added entry"));
            let _ = self.changes.send(slug.to_owned());
            Ok(())
        })
    }

    pub fn record(&self, slug: &str, event: Event) -> Result<(), String> {
        self.record_if(slug, event, |_| Ok(()))
    }

    /// Records the event only while `condition` holds for the Epic, checked in the same step as the event itself.
    pub fn record_if(&self, slug: &str, event: Event, condition: impl FnOnce(&Epic) -> Result<(), String>) -> Result<(), String> {
        self.record_from(slug, |epic| condition(epic).map(|()| event)).map(drop)
    }

    /// Records the event `make` builds from the Epic, in the same step as the event itself, and gives it back.
    pub fn record_from(&self, slug: &str, make: impl FnOnce(&Epic) -> Result<Event, String>) -> Result<Event, String> {
        tokio::task::block_in_place(|| {
            let _appending = self.appending.lock().unwrap();
            let event = {
                let epics = self.epics.lock().unwrap();
                let epic = epics.get(slug).ok_or_else(|| format!("there is no Epic {slug}"))?;
                let event = make(epic)?;
                epic.check(&event)?;
                event
            };
            let entry = stamp(event);
            let path = self.path(slug);
            OpenOptions::new()
                .append(true)
                .open(&path)
                .and_then(|file| append(file, &entry))
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
            if let Some(epic) = self.epics.lock().unwrap().get_mut(slug) {
                epic.apply(&entry);
            }
            let _ = self.changes.send(slug.to_owned());
            Ok(entry.event)
        })
    }
}

fn stamp(event: Event) -> Entry {
    let at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64);
    Entry { at, event }
}

fn append(mut file: File, entry: &Entry) -> io::Result<()> {
    let mut line = serde_json::to_vec(entry)?;
    line.push(b'\n');
    file.write_all(&line)?;
    file.sync_data()
}

fn read(path: &Path) -> io::Result<Vec<Entry>> {
    let text = fs::read_to_string(path)?;
    let mut entries = Vec::new();
    for (index, line) in text.lines().enumerate().filter(|(_, line)| !line.trim().is_empty()) {
        match serde_json::from_str(line) {
            Ok(entry) => entries.push(entry),
            Err(error) => eprintln!("{}:{}: {error}", path.display(), index + 1),
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDED: &str = r#"{"at":1,"kind":"epic.added","slug":"loop","title":"The loop","brief":"Brief","project":"/repo/.git"}"#;

    fn ticket(number: u32, blockers: &[u32]) -> String {
        format!(r#"{{"at":2,"kind":"ticket.added","ticket":{number},"title":"T{number}","body":"Body {number}","blockers":{blockers:?}}}"#)
    }

    fn blockers(number: u32, blockers: &[u32]) -> String {
        format!(r#"{{"at":3,"kind":"blockers.set","ticket":{number},"blockers":{blockers:?}}}"#)
    }

    fn folded(lines: &[String]) -> Option<Epic> {
        let entries: Vec<Entry> = lines.iter().map(|line| serde_json::from_str(line).unwrap()).collect();
        fold(&entries)
    }

    fn frontier(epic: &Epic) -> Vec<u32> {
        epic.frontier().map(|(number, _)| number).collect()
    }

    #[test]
    fn renaming_a_rig_preserves_run_ticket_and_claim_choices_after_reload() {
        let dir = tempfile::tempdir().unwrap();
        let epics = Epics::load(dir.path()).unwrap();
        epics.add("loop", "The loop", "Brief", Path::new("/repo")).unwrap();
        epics.record("loop", Event::SetupSet { command: String::new() }).unwrap();
        for ticket in 1..=3 {
            epics.record("loop", Event::TicketAdded {
                ticket, title: format!("T{ticket}"), body: "Body".to_owned(), blockers: vec![], tags: vec![],
            }).unwrap();
            epics.record("loop", Event::RigSet { ticket, rig: Some(if ticket == 3 { "Other" } else { "Original" }.to_owned()) }).unwrap();
        }
        epics.record("loop", Event::Claimed { ticket: 1, chat: "chat".to_owned(), rig: "Original".to_owned(), worktree: PathBuf::from("/repo/1") }).unwrap();
        epics.record("loop", Event::RunStarted { rig: "Original".to_owned(), parallel: 2, reviewers: vec![], remediator: None }).unwrap();
        epics.rename_rig("Original", "Reviewer").unwrap();
        let reloaded = Epics::load(dir.path()).unwrap();
        for epics in [&epics, &reloaded] {
            let epic = epics.get("loop").unwrap();
            assert_eq!(epic.run.unwrap().rig, "Reviewer");
            assert_eq!(epic.tickets[&1].claim.as_ref().unwrap().rig, "Reviewer");
            assert_eq!(epic.tickets[&1].rig.as_deref(), Some("Reviewer"));
            assert_eq!(epic.tickets[&2].rig.as_deref(), Some("Reviewer"));
            assert_eq!(epic.tickets[&3].rig.as_deref(), Some("Other"));
        }
    }

    #[test]
    fn tickets_fold_into_statuses_and_the_frontier() {
        let unknown = [
            r#"{"at":4,"kind":"run.rotated","rig":"Codex"}"#.to_owned(),
            r#"{"at":5,"kind":"claim.transferred","ticket":1,"terminal":"t"}"#.to_owned(),
        ];
        let missing = [
            blockers(9, &[]),
            r#"{"at":3,"kind":"ticket.body.set","ticket":9,"body":"x"}"#.to_owned(),
            r#"{"at":3,"kind":"note.added","ticket":9,"author":"agent","text":"x"}"#.to_owned(),
        ];
        let cases: Vec<(&str, Vec<String>, Vec<u32>, Vec<u32>)> = vec![
            ("no tickets", vec![], vec![], vec![]),
            ("unblocked tickets in number order", vec![ticket(2, &[]), ticket(1, &[])], vec![1, 2], vec![1, 2]),
            ("a not started blocker holds back", vec![ticket(1, &[]), ticket(2, &[1]), ticket(3, &[1, 2])], vec![1, 2, 3], vec![1]),
            ("blockers.set removes a blocker", vec![ticket(1, &[]), ticket(2, &[1]), blockers(2, &[])], vec![1, 2], vec![1, 2]),
            ("blockers.set adds a blocker", vec![ticket(1, &[]), ticket(2, &[]), blockers(2, &[1])], vec![1, 2], vec![1]),
            ("a blocker naming no ticket never resolves", vec![ticket(1, &[]), blockers(1, &[9])], vec![1], vec![]),
            ("events for a missing ticket change nothing", missing.to_vec(), vec![], vec![]),
            ("a used number keeps its first ticket", vec![ticket(1, &[]), ticket(2, &[]), ticket(1, &[2])], vec![1, 2], vec![1, 2]),
            ("unknown kinds are ignored", [vec![ticket(1, &[])], unknown.to_vec(), vec![ticket(2, &[1])]].concat(), vec![1, 2], vec![1]),
        ];
        for (name, events, tickets, expected) in cases {
            let epic = folded(&[vec![ADDED.to_owned()], events].concat()).unwrap();
            assert_eq!(epic.tickets.keys().copied().collect::<Vec<_>>(), tickets, "{name}");
            assert!(epic.tickets.values().all(|ticket| ticket.status == Status::NotStarted), "{name}");
            assert_eq!(frontier(&epic), expected, "{name}");
        }
    }

    #[test]
    fn only_merged_or_skipped_blockers_are_resolved() {
        use Status::*;
        for (status, resolved) in [(NotStarted, false), (InProgress, false), (Merging, false), (Merged, true), (Skipped, true)] {
            let mut epic = folded(&[ADDED.to_owned(), ticket(1, &[]), ticket(2, &[1]), ticket(3, &[])]).unwrap();
            epic.tickets.get_mut(&1).unwrap().status = status;
            let expected: Vec<u32> = [(status == NotStarted, 1), (resolved, 2), (true, 3)]
                .into_iter()
                .filter_map(|(on, number)| on.then_some(number))
                .collect();
            assert_eq!(frontier(&epic), expected, "{status}");
            epic.tickets.get_mut(&3).unwrap().status = status;
            assert_eq!(frontier(&epic).contains(&3), status == NotStarted, "{status}");
        }
    }

    #[test]
    fn blocker_changes_that_would_make_a_cycle_are_refused_naming_it() {
        let mut epic = folded(&[ADDED.to_owned(), ticket(1, &[]), ticket(2, &[1]), ticket(3, &[2]), ticket(4, &[])]).unwrap();
        let cycle = |path: &str| Err(format!("those Blockers would make a cycle: {path}"));
        let steps: Vec<(u32, &[u32], Result<(), String>)> = vec![
            (1, &[3], cycle("loop/1 is blocked by loop/3, which is blocked by loop/2, which is blocked by loop/1")),
            (2, &[1, 3], cycle("loop/2 is blocked by loop/3, which is blocked by loop/2")),
            (3, &[3], Err("loop/3 cannot block itself".to_owned())),
            (3, &[9], Err("loop has no Ticket 9 to block loop/3".to_owned())),
            (9, &[1], Err("loop has no Ticket 9".to_owned())),
            (4, &[2, 3], Ok(())),
            (2, &[4], cycle("loop/2 is blocked by loop/4, which is blocked by loop/2")),
            (2, &[], Ok(())),
            (1, &[3], Ok(())),
            (2, &[1], cycle("loop/2 is blocked by loop/1, which is blocked by loop/3, which is blocked by loop/2")),
            (3, &[], Ok(())),
            (2, &[1], Ok(())),
        ];
        for (number, blockers, expected) in steps {
            let event = Event::BlockersSet { ticket: number, blockers: blockers.to_vec() };
            let checked = epic.check(&event);
            assert_eq!(checked, expected, "{number} after {blockers:?}");
            if checked.is_ok() {
                epic.apply(&Entry { at: 9, event });
            }
        }
        let blockers: Vec<_> = epic.tickets.iter().map(|(number, ticket)| (*number, ticket.blockers.clone())).collect();
        assert_eq!(blockers, [(1, vec![3]), (2, vec![1]), (3, vec![]), (4, vec![2, 3])]);
        assert_eq!(frontier(&epic), [3]);
    }

    #[test]
    fn every_kind_folds_into_the_epic() {
        let epic = folded(&[
            ADDED.to_owned(),
            r#"{"at":2,"kind":"brief.set","brief":"Second brief"}"#.to_owned(),
            r#"{"at":3,"kind":"setup.set","command":""}"#.to_owned(),
            ticket(1, &[]),
            r#"{"at":5,"kind":"ticket.body.set","ticket":1,"body":"Edited"}"#.to_owned(),
            r#"{"at":6,"kind":"note.added","ticket":1,"author":"agent","text":"First"}"#.to_owned(),
            r#"{"at":7,"kind":"note.added","ticket":1,"author":"david","text":"Second"}"#.to_owned(),
            r#"{"at":8,"kind":"epic.archived"}"#.to_owned(),
        ])
        .unwrap();
        let ticket = Ticket {
            title: "T1".to_owned(),
            body: "Edited".to_owned(),
            blockers: vec![],
            tags: vec![],
            rig: None,
            status: Status::NotStarted,
            notes: vec![
                Note { author: Author::Agent, text: "First".to_owned(), at: 6 },
                Note { author: Author::David, text: "Second".to_owned(), at: 7 },
            ],
            claim: None,
            worktree: None,
            attention: None,
            resolver: None,
            commit: None,
            releasing: false,
            kept: None,
        };
        let expected = Epic {
            slug: "loop".to_owned(),
            title: "The loop".to_owned(),
            project: PathBuf::from("/repo/.git"),
            brief: "Second brief".to_owned(),
            setup: Some(String::new()),
            archived: true,
            base: None,
            integration: None,
            tickets: BTreeMap::from([(1, ticket)]),
            queue: Vec::new(),
            run: None,
            call: None,
            landed: None,
            kept: None,
            notes: Vec::new(),
            review: None,
            findings: Vec::new(),
        };
        assert_eq!(epic, expected);
        let fresh = folded(&[ADDED.to_owned()]).unwrap();
        assert_eq!((fresh.setup, fresh.archived), (None, false));
    }

    #[test]
    fn a_log_that_does_not_start_with_epic_added_projects_to_nothing() {
        assert_eq!(folded(&[]), None);
        assert_eq!(folded(&[ticket(1, &[]), ADDED.to_owned()]), None);
    }

    #[test]
    fn entries_are_one_flat_object_per_line() {
        let cases = [
            (Event::BriefSet { brief: "B".to_owned() }, r#"{"at":9,"kind":"brief.set","brief":"B"}"#),
            (Event::SetupSet { command: "pnpm install".to_owned() }, r#"{"at":9,"kind":"setup.set","command":"pnpm install"}"#),
            (
                Event::TicketAdded { ticket: 3, title: "T".to_owned(), body: "B".to_owned(), blockers: vec![1, 2], tags: vec![] },
                r#"{"at":9,"kind":"ticket.added","ticket":3,"title":"T","body":"B","blockers":[1,2]}"#,
            ),
            (Event::BodySet { ticket: 3, body: "B".to_owned() }, r#"{"at":9,"kind":"ticket.body.set","ticket":3,"body":"B"}"#),
            (Event::BlockersSet { ticket: 3, blockers: vec![] }, r#"{"at":9,"kind":"blockers.set","ticket":3,"blockers":[]}"#),
            (
                Event::NoteAdded { ticket: 3, author: Author::Agent, text: "N".to_owned() },
                r#"{"at":9,"kind":"note.added","ticket":3,"author":"agent","text":"N"}"#,
            ),
            (Event::Archived, r#"{"at":9,"kind":"epic.archived"}"#),
            (
                Event::BranchCut { base: "main".to_owned(), worktree: PathBuf::from("/w/i") },
                r#"{"at":9,"kind":"branch.cut","base":"main","worktree":"/w/i"}"#,
            ),
            (
                Event::Claimed { ticket: 3, chat: "c".to_owned(), rig: "R".to_owned(), worktree: PathBuf::from("/w/3") },
                r#"{"at":9,"kind":"ticket.claimed","ticket":3,"chat":"c","rig":"R","worktree":"/w/3"}"#,
            ),
            (Event::SetupFailed { ticket: 3 }, r#"{"at":9,"kind":"setup.failed","ticket":3}"#),
            (Event::Completed { ticket: 3, commit: "abc".to_owned() }, r#"{"at":9,"kind":"ticket.completed","ticket":3,"commit":"abc"}"#),
            (Event::Skipped { ticket: 3, reason: "R".to_owned() }, r#"{"at":9,"kind":"ticket.skipped","ticket":3,"reason":"R"}"#),
            (Event::Merged { ticket: 3 }, r#"{"at":9,"kind":"ticket.merged","ticket":3}"#),
            (Event::MergeFailed { ticket: 3, reason: "R".to_owned() }, r#"{"at":9,"kind":"merge.failed","ticket":3,"reason":"R"}"#),
            (
                Event::MergeConflicted { ticket: 3, resolver: "c".to_owned() },
                r#"{"at":9,"kind":"merge.conflicted","ticket":3,"resolver":"c"}"#,
            ),
            (Event::MergeResolved { ticket: 3 }, r#"{"at":9,"kind":"merge.resolved","ticket":3}"#),
            (Event::WorktreeRemoved { ticket: 3 }, r#"{"at":9,"kind":"worktree.removed","ticket":3}"#),
            (Event::WorktreeKept { ticket: 3, reason: "R".to_owned() }, r#"{"at":9,"kind":"worktree.kept","ticket":3,"reason":"R"}"#),
            (Event::Reopened { ticket: 3 }, r#"{"at":9,"kind":"ticket.reopened","ticket":3}"#),
            (Event::StartFailed { ticket: 3, reason: "R".to_owned() }, r#"{"at":9,"kind":"start.failed","ticket":3,"reason":"R"}"#),
            (Event::RestoreFailed { ticket: 3, chat: "c".to_owned() }, r#"{"at":9,"kind":"restore.failed","ticket":3,"chat":"c"}"#),
            (
                Event::RunStarted { rig: "R".to_owned(), parallel: 2, reviewers: vec![], remediator: None },
                r#"{"at":9,"kind":"run.started","rig":"R","parallel":2}"#,
            ),
            (
                Event::RunStarted { rig: "R".to_owned(), parallel: 2, reviewers: vec!["A".to_owned(), "B".to_owned()], remediator: Some("C".to_owned()) },
                r#"{"at":9,"kind":"run.started","rig":"R","parallel":2,"reviewers":["A","B"],"remediator":"C"}"#,
            ),
            (
                Event::RunChanged { rig: None, parallel: Some(3), reviewers: None, remediator: None },
                r#"{"at":9,"kind":"run.changed","parallel":3}"#,
            ),
            (
                Event::RunChanged { rig: Some("R".to_owned()), parallel: None, reviewers: Some(vec![]), remediator: Some("C".to_owned()) },
                r#"{"at":9,"kind":"run.changed","rig":"R","reviewers":[],"remediator":"C"}"#,
            ),
            (Event::RunFinished, r#"{"at":9,"kind":"run.finished"}"#),
            (Event::RunPaused { pause: Pause::David }, r#"{"at":9,"kind":"run.paused","reason":"david"}"#),
            (Event::RunResumed, r#"{"at":9,"kind":"run.resumed"}"#),
            (Event::RunStopped, r#"{"at":9,"kind":"run.stopped"}"#),
            (
                Event::Released {
                    ticket: 3,
                    keep: Keep { worktree: PathBuf::from("/w/3"), agent: Some(Agent::ClaudeCode), session: Some("s".to_owned()) },
                },
                r#"{"at":9,"kind":"claim.released","ticket":3,"keep":{"worktree":"/w/3","agent":"claude-code","session":"s"}}"#,
            ),
            (
                Event::Released { ticket: 3, keep: Keep { worktree: PathBuf::from("/w/3"), agent: Some(Agent::Codex), session: None } },
                r#"{"at":9,"kind":"claim.released","ticket":3,"keep":{"worktree":"/w/3","agent":"codex"}}"#,
            ),
            (Event::Viewed, r#"{"at":9,"kind":"epic.viewed"}"#),
            (
                Event::Landed { base: "main".to_owned(), head: "abc".to_owned() },
                r#"{"at":9,"kind":"epic.landed","base":"main","head":"abc"}"#,
            ),
            (Event::LandingRefused { reason: "R".to_owned() }, r#"{"at":9,"kind":"landing.refused","reason":"R"}"#),
            (Event::IntegrationRemoved, r#"{"at":9,"kind":"integration.removed"}"#),
            (Event::IntegrationKept { reason: "R".to_owned() }, r#"{"at":9,"kind":"integration.kept","reason":"R"}"#),
            (Event::ReviewStarted, r#"{"at":9,"kind":"review.started"}"#),
            (
                Event::ReviewerStarted { reviewer: 2, rig: "R".to_owned(), chat: "c".to_owned(), worktree: PathBuf::from("/w/review-2") },
                r#"{"at":9,"kind":"reviewer.started","reviewer":2,"rig":"R","chat":"c","worktree":"/w/review-2"}"#,
            ),
            (
                Event::ReviewerFailed { reviewer: 2, rig: "R".to_owned(), reason: "W".to_owned() },
                r#"{"at":9,"kind":"reviewer.failed","reviewer":2,"rig":"R","reason":"W"}"#,
            ),
            (Event::ReviewerReported { reviewer: 2 }, r#"{"at":9,"kind":"reviewer.reported","reviewer":2}"#),
            (
                Event::FindingAdded { finding: 1, rig: "R".to_owned(), title: "T".to_owned(), body: "B".to_owned() },
                r#"{"at":9,"kind":"finding.added","finding":1,"rig":"R","title":"T","body":"B"}"#,
            ),
            (
                Event::RemediationStarted { rig: "R".to_owned(), chat: "c".to_owned() },
                r#"{"at":9,"kind":"remediation.started","rig":"R","chat":"c"}"#,
            ),
            (
                Event::RemediationFailed { rig: "R".to_owned(), reason: "W".to_owned() },
                r#"{"at":9,"kind":"remediation.failed","rig":"R","reason":"W"}"#,
            ),
            (
                Event::FindingJudged { finding: 1, accepted: false, reason: "W".to_owned() },
                r#"{"at":9,"kind":"finding.judged","finding":1,"accepted":false,"reason":"W"}"#,
            ),
            (Event::Remediated { commit: "abc".to_owned() }, r#"{"at":9,"kind":"review.remediated","commit":"abc"}"#),
        ];
        for (event, line) in cases {
            let entry = Entry { at: 9, event };
            assert_eq!(serde_json::to_string(&entry).unwrap(), line);
            assert_eq!(serde_json::from_str::<Entry>(line).unwrap(), entry);
        }
        let unknown: Entry = serde_json::from_str(r#"{"at":9,"kind":"run.rotated"}"#).unwrap();
        assert_eq!(unknown.event, Event::Unknown);
    }

    fn claimed(number: u32) -> String {
        format!(r#"{{"at":4,"kind":"ticket.claimed","ticket":{number},"chat":"chat{number}","rig":"Claude","worktree":"/w/{number}"}}"#)
    }

    fn completed(number: u32) -> String {
        format!(r#"{{"at":5,"kind":"ticket.completed","ticket":{number},"commit":"c{number}"}}"#)
    }

    fn event(kind: &str, number: u32) -> String {
        format!(r#"{{"at":6,"kind":"{kind}","ticket":{number},"reason":"Why {number}"}}"#)
    }

    #[test]
    fn claims_verdicts_and_merges_move_tickets_through_their_statuses() {
        use Status::*;
        let base = [ticket(1, &[]), ticket(2, &[1]), ticket(3, &[])];
        let cases: Vec<(&str, Vec<String>, [Status; 3], Vec<u32>, Vec<u32>)> = vec![
            ("a Claim takes a Ticket off the Frontier", vec![claimed(1)], [InProgress, NotStarted, NotStarted], vec![3], vec![]),
            ("complete makes it merging and queues it", vec![claimed(1), completed(1)], [Merging, NotStarted, NotStarted], vec![3], vec![1]),
            (
                "a merge resolves it as a Blocker",
                vec![claimed(1), completed(1), event("ticket.merged", 1)],
                [Merged, NotStarted, NotStarted],
                vec![2, 3],
                vec![],
            ),
            ("skip resolves it as a Blocker", vec![claimed(1), event("ticket.skipped", 1)], [Skipped, NotStarted, NotStarted], vec![2, 3], vec![]),
            (
                "a failed merge leaves it merging, out of the queue",
                vec![claimed(1), completed(1), event("merge.failed", 1)],
                [Merging, NotStarted, NotStarted],
                vec![3],
                vec![],
            ),
            (
                "merges queue in Verdict order",
                vec![claimed(3), claimed(1), completed(3), completed(1)],
                [Merging, NotStarted, Merging],
                vec![],
                vec![3, 1],
            ),
            (
                "a merge leaves the rest of the queue",
                vec![claimed(3), claimed(1), completed(3), completed(1), event("ticket.merged", 3)],
                [Merging, NotStarted, Merged],
                vec![],
                vec![1],
            ),
            ("a failed setup leaves it on the Frontier", vec![event("setup.failed", 1)], [NotStarted, NotStarted, NotStarted], vec![1, 3], vec![]),
        ];
        for (name, events, statuses, expected, queue) in cases {
            let epic = folded(&[vec![ADDED.to_owned()], base.to_vec(), events].concat()).unwrap();
            let folded: Vec<Status> = epic.tickets.values().map(|ticket| ticket.status).collect();
            assert_eq!(folded, statuses, "{name}");
            assert_eq!(frontier(&epic), expected, "{name}");
            assert_eq!(epic.queue, queue, "{name}");
        }
    }

    #[test]
    fn a_ticket_keeps_its_claim_worktree_and_attention() {
        let epic = folded(&[
            ADDED.to_owned(),
            ticket(1, &[]),
            r#"{"at":2,"kind":"branch.cut","base":"main","worktree":"/w/integration"}"#.to_owned(),
            event("setup.failed", 1),
        ])
        .unwrap();
        assert_eq!((epic.base.as_deref(), epic.integration.as_deref()), (Some("main"), Some(Path::new("/w/integration"))));
        let failed = &epic.tickets[&1];
        assert!(failed.attention.as_ref().unwrap().contains("Setup Command"));
        assert_eq!((&failed.claim, &failed.worktree), (&None, &None));

        let lines = [ADDED.to_owned(), ticket(1, &[]), event("setup.failed", 1), claimed(1)];
        let epic = folded(&lines).unwrap();
        let held = &epic.tickets[&1];
        assert_eq!(held.claim, Some(Claim { chat: "chat1".to_owned(), rig: "Claude".to_owned(), edited: false, continuation: None, cut_turn: None }));
        assert_eq!(held.worktree.as_deref(), Some(Path::new("/w/1")));
        assert_eq!(held.attention, None, "a Claim clears what the failed setup flagged");

        let merged = folded(&[lines.to_vec(), vec![completed(1), event("ticket.merged", 1)]].concat()).unwrap();
        let ticket1 = &merged.tickets[&1];
        assert_eq!((ticket1.commit.as_deref(), ticket1.claim.is_some()), (Some("c1"), true), "the Claim records who gave the Verdict");
        assert!(ticket1.awaits_cleanup());
        let removed = folded(&[lines.to_vec(), vec![completed(1), event("ticket.merged", 1), event("worktree.removed", 1)]].concat()).unwrap();
        assert_eq!(removed.tickets[&1].worktree, None);
        assert!(!removed.tickets[&1].awaits_cleanup());
        let kept = folded(&[lines.to_vec(), vec![event("ticket.skipped", 1), event("worktree.kept", 1)]].concat()).unwrap();
        let kept = &kept.tickets[&1];
        assert_eq!((kept.attention.as_deref(), kept.worktree.is_some()), (Some("Why 1"), true));
        assert_eq!(kept.notes, vec![Note { author: Author::Agent, text: "Why 1".to_owned(), at: 6 }], "a skip's reason is its Note");
        assert!(!kept.awaits_cleanup());
    }

    #[test]
    fn check_refuses_events_the_state_machine_forbids() {
        let epic = folded(&[
            ADDED.to_owned(),
            r#"{"at":2,"kind":"branch.cut","base":"main","worktree":"/w/integration"}"#.to_owned(),
            ticket(1, &[]),
            ticket(2, &[1]),
            ticket(3, &[]),
            ticket(4, &[]),
            claimed(3),
            completed(3),
            claimed(4),
        ])
        .unwrap();
        let claim = |ticket| Event::Claimed { ticket, chat: "c".to_owned(), rig: "R".to_owned(), worktree: PathBuf::from("/w") };
        let refused = [
            (claim(2), "loop/2 is not on the Frontier: it is not started"),
            (claim(3), "loop/3 is not on the Frontier: it is merging"),
            (Event::Completed { ticket: 1, commit: "c".to_owned() }, "loop/1 is not started, not in progress, so it takes no Verdict"),
            (Event::Completed { ticket: 3, commit: "c".to_owned() }, "loop/3 is merging, not in progress, so it takes no Verdict"),
            (Event::Skipped { ticket: 1, reason: "r".to_owned() }, "loop/1 is not started, not in progress, so it takes no Verdict"),
            (Event::Merged { ticket: 1 }, "loop/1 is not started, not merging"),
            (Event::BranchCut { base: "main".to_owned(), worktree: PathBuf::from("/w") }, "loop already has its Epic branch"),
            (claim(9), "loop has no Ticket 9"),
            (Event::Reopened { ticket: 1 }, "loop/1 is not started; only an in-progress, merged or skipped Ticket can be moved back to not started"),
            (Event::Reopened { ticket: 3 }, "loop/3 is merging; only an in-progress, merged or skipped Ticket can be moved back to not started"),
            (Event::Reopened { ticket: 9 }, "loop has no Ticket 9"),
            (Event::WorktreeRemoved { ticket: 4 }, "loop/4 is in progress in its worktree"),
            (Event::WorktreeKept { ticket: 3, reason: "r".to_owned() }, "loop/3 is merging, not merged or skipped"),
        ];
        for (event, error) in refused {
            assert_eq!(epic.check(&event), Err(error.to_owned()));
        }
        assert_eq!(epic.check(&claim(1)), Ok(()));
        assert_eq!(epic.check(&Event::Merged { ticket: 3 }), Ok(()));
        assert_eq!(epic.check(&Event::Reopened { ticket: 4 }), Ok(()));
    }

    fn reopened(number: u32) -> String {
        format!(r#"{{"at":7,"kind":"ticket.reopened","ticket":{number}}}"#)
    }

    #[test]
    fn reopening_a_merged_or_skipped_ticket_returns_it_to_the_frontier_without_its_claim() {
        use Status::*;
        let base = [ADDED.to_owned(), ticket(1, &[]), ticket(2, &[1])];
        let merged = vec![claimed(1), completed(1), event("ticket.merged", 1), event("worktree.removed", 1)];
        let kept = vec![claimed(1), event("ticket.skipped", 1), event("worktree.kept", 1)];
        let cases: Vec<(&str, Vec<String>)> = vec![
            ("a merged Ticket", merged.clone()),
            ("a skipped Ticket whose worktree was kept", kept.clone()),
            ("a merged Ticket before its cleanup", vec![claimed(1), completed(1), event("ticket.merged", 1)]),
            ("a Ticket merged a second time", [merged.clone(), vec![reopened(1)], merged.clone()].concat()),
        ];
        for (name, events) in cases {
            let resolved = folded(&[base.to_vec(), events.clone()].concat()).unwrap();
            assert!(resolved.tickets[&1].status.resolved(), "{name}");
            assert_eq!(resolved.check(&Event::Reopened { ticket: 1 }), Ok(()), "{name}");
            let epic = folded(&[base.to_vec(), events, vec![reopened(1)]].concat()).unwrap();
            assert_eq!(epic.tickets.values().map(|ticket| ticket.status).collect::<Vec<_>>(), [NotStarted, NotStarted], "{name}");
            assert_eq!(frontier(&epic), [1], "{name}: its dependent waits again");
            let ticket = &epic.tickets[&1];
            assert_eq!((&ticket.claim, &ticket.worktree, &ticket.attention, &ticket.commit), (&None, &None, &None, &None), "{name}");
        }

        let epic = folded(&[base.to_vec(), kept, vec![reopened(1)]].concat()).unwrap();
        assert_eq!(
            epic.tickets[&1].notes,
            [
                Note { author: Author::Agent, text: "Why 1".to_owned(), at: 6 },
                Note { author: Author::Stampede, text: "Why 1".to_owned(), at: 7 }
            ],
            "a kept worktree's flag becomes a Note"
        );

        let again = r#"{"at":8,"kind":"ticket.claimed","ticket":1,"chat":"again","rig":"Codex","worktree":"/w/1-2"}"#.to_owned();
        let epic = folded(&[base.to_vec(), merged, vec![reopened(1), again]].concat()).unwrap();
        let ticket = &epic.tickets[&1];
        assert_eq!((ticket.status, frontier(&epic)), (InProgress, vec![]));
        assert_eq!(ticket.claim, Some(Claim { chat: "again".to_owned(), rig: "Codex".to_owned(), edited: false, continuation: None, cut_turn: None }));
        assert_eq!(ticket.worktree.as_deref(), Some(Path::new("/w/1-2")));
    }

    #[test]
    fn a_body_set_after_the_current_claim_marks_it_edited() {
        let edit = |number: u32| format!(r#"{{"at":5,"kind":"ticket.body.set","ticket":{number},"body":"Edited"}}"#);
        let skipped = event("ticket.skipped", 1);
        let cases: Vec<(&str, Vec<String>, Option<bool>)> = vec![
            ("an edit with no Claim", vec![edit(1)], None),
            ("an edit before the Claim", vec![edit(1), claimed(1)], Some(false)),
            ("an edit after the Claim", vec![claimed(1), edit(1)], Some(true)),
            ("another Ticket's edit", vec![claimed(1), edit(2)], Some(false)),
            ("an edit before the Verdict", vec![claimed(1), edit(1), completed(1), event("ticket.merged", 1)], Some(true)),
            ("an edit after the Verdict", vec![claimed(1), completed(1), edit(1)], Some(true)),
            ("reopening, which drops the Claim", vec![claimed(1), edit(1), skipped.clone(), reopened(1)], None),
            ("the next Claim", vec![claimed(1), edit(1), skipped, reopened(1), claimed(1)], Some(false)),
        ];
        for (name, events, expected) in cases {
            let epic = folded(&[vec![ADDED.to_owned(), ticket(1, &[]), ticket(2, &[])], events].concat()).unwrap();
            assert_eq!(epic.tickets[&1].claim.as_ref().map(|claim| claim.edited), expected, "{name}");
        }

        let claim = |edited| serde_json::to_value(Claim { chat: "c".to_owned(), rig: "R".to_owned(), edited, continuation: None, cut_turn: None }).unwrap();
        assert_eq!(claim(false), serde_json::json!({ "chat": "c", "rig": "R" }));
        assert_eq!(claim(true), serde_json::json!({ "chat": "c", "rig": "R", "edited": true }));
    }

    const SETUP: &str = r#"{"at":2,"kind":"setup.set","command":""}"#;

    fn started(rig: &str, parallel: u32) -> String {
        format!(r#"{{"at":7,"kind":"run.started","rig":"{rig}","parallel":{parallel}}}"#)
    }

    fn run_event(kind: &str) -> String {
        format!(r#"{{"at":8,"kind":"{kind}"}}"#)
    }

    #[test]
    fn a_run_starts_changes_finishes_and_calls_until_viewed() {
        let run = |rig: &str, parallel| Some(Run { recovered: BTreeSet::new(), rig: rig.to_owned(), parallel, reviewers: vec![], remediator: None, paused: None });
        let changed = |payload: &str| format!(r#"{{"at":8,"kind":"run.changed",{payload}}}"#);
        let cases: Vec<(&str, Vec<String>, Option<Run>, bool)> = vec![
            ("no Run", vec![], None, false),
            ("started", vec![started("Claude", 2)], run("Claude", 2), false),
            ("the limit changes alone", vec![started("Claude", 2), changed(r#""parallel":3"#)], run("Claude", 3), false),
            ("the Rig changes alone", vec![started("Claude", 2), changed(r#""rig":"Codex""#)], run("Codex", 2), false),
            ("finished", vec![started("Claude", 2), run_event("run.finished")], None, true),
            ("viewed", vec![started("Claude", 2), run_event("run.finished"), run_event("epic.viewed")], None, false),
            ("a new Run", vec![started("Claude", 2), run_event("run.finished"), started("Codex", 1)], run("Codex", 1), false),
            ("a change without a Run", vec![changed(r#""parallel":3"#)], None, false),
        ];
        for (name, events, run, finished) in cases {
            let epic = folded(&[vec![ADDED.to_owned(), SETUP.to_owned(), ticket(1, &[])], events].concat()).unwrap();
            assert_eq!((epic.run, epic.call == Some(Call::Finished)), (run, finished), "{name}");
        }
    }

    #[test]
    fn a_new_run_retries_tickets_it_could_not_start_and_a_lost_chat_keeps_its_claim() {
        let start_failed = r#"{"at":6,"kind":"start.failed","ticket":3,"reason":"No Chat"}"#.to_owned();
        let restore_failed = r#"{"at":6,"kind":"restore.failed","ticket":2,"chat":"chat2"}"#.to_owned();
        let lines = [
            ADDED.to_owned(),
            SETUP.to_owned(),
            ticket(1, &[]),
            ticket(2, &[]),
            ticket(3, &[]),
            ticket(4, &[]),
            claimed(1),
            completed(1),
            event("merge.failed", 1),
            claimed(2),
            restore_failed,
            start_failed,
            event("setup.failed", 4),
        ];
        let epic = folded(&lines).unwrap();
        let attention = |epic: &Epic| epic.tickets.values().map(|ticket| ticket.attention.is_some()).collect::<Vec<_>>();
        assert_eq!(attention(&epic), [true, true, true, true]);
        assert_eq!(epic.tickets[&3].attention.as_deref(), Some("No Chat"));
        let lost = &epic.tickets[&2];
        assert_eq!((lost.status, lost.claim.as_ref().map(|claim| claim.chat.as_str())), (Status::InProgress, Some("chat2")));
        assert_eq!(lost.worktree.as_deref(), Some(Path::new("/w/2")));
        assert!(lost.attention.as_ref().unwrap().contains("Its Chat chat2 did not come back"), "{lost:?}");
        assert_eq!(frontier(&epic), [3, 4], "flagged Tickets stay on the Frontier");

        let rerun = folded(&[lines.to_vec(), vec![started("Claude", 1)]].concat()).unwrap();
        assert_eq!(attention(&rerun), [true, true, false, false], "only not-started Tickets are retried");
    }

    #[test]
    fn run_events_out_of_turn_are_refused() {
        let unset = folded(&[ADDED.to_owned(), ticket(1, &[])]).unwrap();
        let start = |parallel| Event::RunStarted { rig: "Claude".to_owned(), parallel, reviewers: vec![], remediator: None };
        let change = |parallel| Event::RunChanged { rig: None, parallel: Some(parallel), reviewers: None, remediator: None };
        assert_eq!(
            unset.check(&start(1)),
            Err(r#"loop has no Setup Command yet; record one, "" when a fresh worktree needs nothing, before starting a Run"#.to_owned())
        );
        let idle = folded(&[ADDED.to_owned(), SETUP.to_owned(), ticket(1, &[]), ticket(2, &[])]).unwrap();
        let live = folded(&[ADDED.to_owned(), SETUP.to_owned(), ticket(1, &[]), ticket(2, &[]), started("Claude", 1), claimed(1)]).unwrap();
        let refused = [
            (&idle, start(0), "a Run works on at least one Ticket at a time"),
            (&idle, change(2), "loop has no live Run"),
            (&idle, Event::RunFinished, "loop has no live Run"),
            (&idle, Event::Viewed, "loop has no finished or paused Run to show"),
            (&live, start(1), "loop already has a live Run"),
            (&live, change(0), "a Run works on at least one Ticket at a time"),
            (&live, Event::StartFailed { ticket: 1, reason: "R".to_owned() }, "loop/1 is in progress"),
            (&live, Event::RestoreFailed { ticket: 2, chat: "c".to_owned() }, "loop/2 is not started"),
        ];
        for (epic, event, error) in refused {
            assert_eq!(epic.check(&event), Err(error.to_owned()), "{event:?}");
        }
        assert_eq!(idle.check(&start(1)), Ok(()));
        assert_eq!(live.check(&change(3)), Ok(()));
        assert_eq!(live.check(&Event::RunFinished), Ok(()));
        assert_eq!(live.check(&Event::StartFailed { ticket: 2, reason: "R".to_owned() }), Ok(()));
        assert_eq!(live.check(&Event::RestoreFailed { ticket: 1, chat: "chat1".to_owned() }), Ok(()));
    }

    #[test]
    fn a_conflicted_merge_holds_the_queue_until_its_resolver_resolves_it() {
        let conflicted = |number: u32| format!(r#"{{"at":7,"kind":"merge.conflicted","ticket":{number},"resolver":"resolver{number}"}}"#);
        let lines = [ADDED.to_owned(), ticket(1, &[]), ticket(2, &[1]), ticket(3, &[]), claimed(1), claimed(3), completed(1), completed(3)];
        let mut epic = folded(&[lines.to_vec(), vec![conflicted(1)]].concat()).unwrap();
        assert_eq!((epic.tickets[&1].status, epic.tickets[&1].resolver.as_deref()), (Status::Merging, Some("resolver1")));
        assert_eq!(epic.queue, [1, 3], "later merges wait behind the Resolver");
        assert_eq!(frontier(&epic), Vec::<u32>::new());
        assert!(!epic.tickets[&1].awaits_cleanup());
        let refused = [
            (Event::MergeConflicted { ticket: 1, resolver: "again".to_owned() }, "loop/1 already has a Resolver"),
            (Event::MergeConflicted { ticket: 3, resolver: "early".to_owned() }, "loop/3 is not the next Ticket to merge"),
            (Event::MergeConflicted { ticket: 2, resolver: "idle".to_owned() }, "loop/2 is not the next Ticket to merge"),
            (Event::MergeResolved { ticket: 3 }, "loop/3 has no Resolver"),
            (Event::RestoreFailed { ticket: 1, chat: "chat1".to_owned() }, "loop/1 is merging"),
        ];
        for (event, error) in refused {
            assert_eq!(epic.check(&event), Err(error.to_owned()));
        }

        let lost = Event::RestoreFailed { ticket: 1, chat: "resolver1".to_owned() };
        assert_eq!(epic.check(&lost), Ok(()), "a Resolver that did not come back is flagged on its Ticket");
        epic.apply(&Entry { at: 8, event: lost });
        let flagged = epic.tickets[&1].attention.as_deref().unwrap();
        assert!(flagged.starts_with("Its Resolver Chat resolver1 did not come back"), "{flagged}");
        assert_eq!(epic.queue, [1, 3]);

        let resolved = Event::MergeResolved { ticket: 1 };
        assert_eq!(epic.check(&resolved), Ok(()));
        epic.apply(&Entry { at: 8, event: resolved });
        let ticket1 = &epic.tickets[&1];
        assert_eq!((ticket1.status, ticket1.resolver.as_deref(), ticket1.attention.as_deref()), (Status::Merged, None, None));
        assert!(ticket1.awaits_cleanup());
        assert_eq!(epic.queue, [3]);
        assert_eq!(frontier(&epic), [2], "a resolved merge resolves its Blocker");
        assert_eq!(epic.check(&Event::MergeConflicted { ticket: 3, resolver: "next".to_owned() }), Ok(()));
    }

    #[test]
    fn a_landing_is_recorded_its_refusals_become_notes_and_a_new_branch_starts_over() {
        let cut = |worktree: &str| format!(r#"{{"at":2,"kind":"branch.cut","base":"main","worktree":"{worktree}"}}"#);
        let refused = r#"{"at":3,"kind":"landing.refused","reason":"/repo is on topic rather than main"}"#.to_owned();
        let landed = r#"{"at":4,"kind":"epic.landed","base":"main","head":"abc"}"#.to_owned();
        let kept = r#"{"at":5,"kind":"integration.kept","reason":"It has uncommitted changes."}"#.to_owned();
        let removed = run_event("integration.removed");
        let state = |lines: &[String]| {
            let epic = folded(&[vec![ADDED.to_owned(), cut("/w/integration")], lines.to_vec()].concat()).unwrap();
            let integration = epic.integration.as_ref().map(|path| path.to_string_lossy().into_owned());
            (integration, epic.landed.map(|landed| (landed.base, landed.head)), epic.kept, epic.notes.len())
        };
        let landing = || Some(("main".to_owned(), "abc".to_owned()));
        let integration = || Some("/w/integration".to_owned());
        let cases: Vec<(&str, Vec<String>, (Option<String>, Option<(String, String)>, Option<String>, usize))> = vec![
            ("a refusal", vec![refused.clone()], (integration(), None, None, 1)),
            ("a clean Landing", vec![refused.clone(), landed.clone(), removed.clone()], (None, landing(), None, 1)),
            (
                "a Landing that keeps the worktree",
                vec![landed.clone(), kept.clone()],
                (integration(), landing(), Some("It has uncommitted changes.".to_owned()), 0),
            ),
            ("a later Landing that removes it", vec![landed.clone(), kept, landed.clone(), removed.clone()], (None, landing(), None, 0)),
            ("a new Epic branch", vec![refused, landed, removed, cut("/w/again")], (Some("/w/again".to_owned()), None, None, 1)),
        ];
        for (name, lines, expected) in cases {
            assert_eq!(state(&lines), expected, "{name}");
        }

        let epic = folded(&[ADDED.to_owned(), cut("/w/integration"), r#"{"at":3,"kind":"landing.refused","reason":"why"}"#.to_owned()]).unwrap();
        assert_eq!(epic.notes, [Note { author: Author::Stampede, text: "Landing was refused: why.".to_owned(), at: 3 }]);
        let json = serde_json::to_value(&epic).unwrap();
        assert_eq!(json["notes"], serde_json::json!([{ "author": "stampede", "text": "Landing was refused: why.", "at": 3 }]));
        let uncut = folded(&[ADDED.to_owned()]).unwrap();
        assert_eq!(serde_json::to_value(&uncut).unwrap().get("notes"), None);
        for event in [Event::Landed { base: "main".to_owned(), head: "abc".to_owned() }, Event::IntegrationRemoved] {
            assert_eq!(uncut.check(&event), Err("loop has no Integration Worktree".to_owned()));
            assert_eq!(epic.check(&event), Ok(()));
        }
        assert_eq!(epic.check(&Event::LandingRefused { reason: " ".to_owned() }), Err("the reason is empty".to_owned()));
    }

    fn paused() -> String {
        r#"{"at":8,"kind":"run.paused","reason":"david"}"#.to_owned()
    }

    #[test]
    fn usage_pauses_persist_one_continuation_and_manual_actions_cancel_it() {
        let base = vec![ADDED.to_owned(), SETUP.to_owned(), ticket(1, &[]), started("Claude", 1), claimed(1)];
        let usage = r#"{"at":8,"kind":"run.paused","reason":"usage","agent":"claude-code","window":"five-hour","resetsAt":1000}"#.to_owned();
        let cut = r#"{"at":9,"kind":"turn.cut","ticket":1,"turn":"first"}"#.to_owned();
        let continued = r#"{"at":11,"kind":"turn.continued","ticket":1}"#.to_owned();
        let cases = [
            (vec![usage.clone(), cut.clone()], Some(Continuation::Pending), Some(Call::Paused)),
            (vec![usage.clone(), cut.clone(), run_event("run.resumed")], Some(Continuation::Due), None),
            (vec![usage.clone(), cut.clone(), run_event("run.resumed"), continued.clone()], None, None),
            (vec![usage.clone(), cut.clone(), paused()], None, None),
            (vec![usage.clone(), cut.clone(), run_event("run.stopped")], None, None),
            (vec![usage.clone(), cut.clone(), run_event("epic.viewed")], Some(Continuation::Pending), None),
        ];
        for (events, continuation, call) in cases {
            let epic = folded(&[base.clone(), events].concat()).unwrap();
            let claim = epic.tickets[&1].claim.as_ref().unwrap();
            assert_eq!(claim.continuation, continuation);
            assert_eq!(claim.cut_turn.as_deref(), Some("first"));
            assert_eq!(epic.call, call);
            assert!(epic.check(&Event::TurnCut { ticket: 1, turn: "first".to_owned() }).is_err());
        }
        let epic = folded(&[base, vec![usage, cut, run_event("run.resumed"), continued]].concat()).unwrap();
        assert_eq!(epic.check(&Event::TurnCut { ticket: 1, turn: "second".to_owned() }), Ok(()));
        let entry: Entry = serde_json::from_str(r#"{"at":8,"kind":"run.paused","reason":"usage","agent":"claude-code","window":"weekly","resetsAt":1000}"#).unwrap();
        assert_eq!(serde_json::to_value(entry).unwrap()["resetsAt"], 1000);
    }

    fn released(number: u32, session: Option<&str>) -> String {
        let session = session.map(|session| format!(r#","session":"{session}""#)).unwrap_or_default();
        format!(r#"{{"at":9,"kind":"claim.released","ticket":{number},"keep":{{"worktree":"/w/{number}","agent":"claude-code"{session}}}}}"#)
    }

    #[test]
    fn a_run_pauses_resumes_and_stops_giving_back_its_tickets() {
        use Status::*;
        let base = [ADDED.to_owned(), SETUP.to_owned(), ticket(1, &[]), ticket(2, &[]), ticket(3, &[])];
        let live = [started("Claude", 2), claimed(1), claimed(2), completed(2)];
        let run = |paused| Some(Run { recovered: BTreeSet::new(), rig: "Claude".to_owned(), parallel: 2, reviewers: vec![], remediator: None, paused });
        let cases: Vec<(&str, Vec<String>, Option<Run>, [Status; 3], [bool; 3])> = vec![
            ("live", vec![], run(None), [InProgress, Merging, NotStarted], [false; 3]),
            ("paused", vec![paused()], run(Some(Pause::David)), [InProgress, Merging, NotStarted], [false; 3]),
            ("resumed", vec![paused(), run_event("run.resumed")], run(None), [InProgress, Merging, NotStarted], [false; 3]),
            (
                "changed while paused",
                vec![paused(), r#"{"at":8,"kind":"run.changed","parallel":3}"#.to_owned()],
                Some(Run { recovered: BTreeSet::new(), rig: "Claude".to_owned(), parallel: 3, reviewers: vec![], remediator: None, paused: Some(Pause::David) }),
                [InProgress, Merging, NotStarted],
                [false; 3],
            ),
            ("stopped", vec![run_event("run.stopped")], None, [InProgress, Merging, NotStarted], [true, false, false]),
            ("stopped while paused", vec![paused(), run_event("run.stopped")], None, [InProgress, Merging, NotStarted], [true, false, false]),
            ("stopped, then released", vec![run_event("run.stopped"), released(1, Some("s1"))], None, [NotStarted, Merging, NotStarted], [false; 3]),
            ("a Verdict while stopping", vec![run_event("run.stopped"), completed(1)], None, [Merging, Merging, NotStarted], [false; 3]),
            ("a skip while stopping", vec![run_event("run.stopped"), event("ticket.skipped", 1)], None, [Skipped, Merging, NotStarted], [false; 3]),
        ];
        for (name, events, expected, statuses, releasing) in cases {
            let epic = folded(&[base.to_vec(), live.to_vec(), events].concat()).unwrap();
            assert_eq!(epic.run, expected, "{name}");
            assert!(epic.call.is_none(), "{name}: stopping is not finishing");
            assert_eq!(epic.tickets.values().map(|ticket| ticket.status).collect::<Vec<_>>(), statuses, "{name}");
            assert_eq!(epic.tickets.values().map(|ticket| ticket.releasing).collect::<Vec<_>>(), releasing, "{name}");
        }

        let epic = folded(&[base.to_vec(), live.to_vec(), vec![paused()]].concat()).unwrap();
        let idle = folded(&[base.to_vec(), live.to_vec(), vec![run_event("run.stopped")]].concat()).unwrap();
        let refused = [
            (&epic, Event::RunPaused { pause: Pause::David }, "loop's Run is already paused"),
            (&folded(&[base.to_vec(), live.to_vec()].concat()).unwrap(), Event::RunResumed, "loop's Run is not paused"),
            (&idle, Event::RunPaused { pause: Pause::David }, "loop has no live Run"),
            (&idle, Event::RunResumed, "loop has no live Run"),
            (&idle, Event::RunStopped, "loop has no live Run"),
            (&idle, Event::Reopened { ticket: 1 }, "loop/1's Chat is already stopping"),
        ];
        for (epic, event, error) in refused {
            assert_eq!(epic.check(&event), Err(error.to_owned()), "{event:?}");
        }
        assert_eq!(epic.check(&Event::RunResumed), Ok(()));
        assert_eq!(epic.check(&Event::RunStopped), Ok(()));
    }

    #[test]
    fn a_released_claim_returns_its_ticket_to_the_frontier_with_its_worktree_and_session() {
        use Status::*;
        let base = [ADDED.to_owned(), ticket(1, &[]), ticket(2, &[1])];
        let lost = r#"{"at":6,"kind":"restore.failed","ticket":1,"chat":"chat1"}"#.to_owned();
        let cases: Vec<(&str, Vec<String>, Option<&str>)> = vec![
            ("its Chat closed", vec![claimed(1), released(1, Some("s1"))], Some("s1")),
            ("moved back", vec![claimed(1), reopened(1), released(1, Some("s1"))], Some("s1")),
            ("closed before its session began", vec![claimed(1), released(1, None)], None),
            ("its Chat lost in a restart", vec![claimed(1), lost, reopened(1), released(1, None)], None),
        ];
        for (name, events, session) in cases {
            let epic = folded(&[base.to_vec(), events].concat()).unwrap();
            let ticket = &epic.tickets[&1];
            assert_eq!((ticket.status, &ticket.claim, ticket.releasing), (NotStarted, &None, false), "{name}");
            assert_eq!((ticket.worktree.as_deref(), &ticket.attention), (Some(Path::new("/w/1")), &None), "{name}");
            assert_eq!(ticket.kept.as_ref().map(|kept| (kept.agent, kept.session.as_str())), session.map(|session| (Agent::ClaudeCode, session)), "{name}");
            assert_eq!(frontier(&epic), [1], "{name}");
        }

        let moving = folded(&[base.to_vec(), vec![claimed(1), reopened(1)]].concat()).unwrap();
        let ticket = &moving.tickets[&1];
        assert_eq!((ticket.status, ticket.releasing, ticket.claim.is_some()), (InProgress, true, true), "held until its Chat stops");
        assert_eq!(frontier(&moving), Vec::<u32>::new());

        let edit = r#"{"at":9,"kind":"ticket.body.set","ticket":1,"body":"Edited"}"#.to_owned();
        let elsewhere = r#"{"at":10,"kind":"ticket.claimed","ticket":1,"chat":"again","rig":"Claude","worktree":"/w/1-2"}"#.to_owned();
        let cases: Vec<(&str, Vec<String>, bool)> = vec![
            ("resumed unedited", vec![claimed(1), released(1, Some("s1")), claimed(1)], false),
            ("edited before its release", vec![claimed(1), edit.clone(), released(1, Some("s1")), claimed(1)], true),
            ("edited while released", vec![claimed(1), released(1, Some("s1")), edit.clone(), claimed(1)], true),
            ("started afresh after an edit", vec![claimed(1), edit.clone(), released(1, None), edit.clone(), claimed(1)], false),
            ("in another worktree", vec![claimed(1), edit.clone(), released(1, Some("s1")), elsewhere], false),
            ("released a second time", vec![claimed(1), edit.clone(), released(1, Some("s1")), claimed(1), released(1, Some("s1")), claimed(1)], true),
        ];
        for (name, events, edited) in cases {
            let epic = folded(&[base.to_vec(), events].concat()).unwrap();
            let ticket = &epic.tickets[&1];
            assert_eq!((ticket.status, ticket.claim.as_ref().map(|claim| claim.edited)), (InProgress, Some(edited)), "{name}");
            assert_eq!(ticket.kept, None, "{name}: a Claim takes what was kept");
        }

        let resolved = folded(&[base.to_vec(), vec![claimed(1), released(1, Some("s1")), claimed(1), event("ticket.skipped", 1), reopened(1)]].concat()).unwrap();
        assert_eq!((&resolved.tickets[&1].worktree, &resolved.tickets[&1].kept), (&None, &None), "reopening a resolved Ticket keeps nothing");

        let keep = Keep { worktree: PathBuf::from("/w/1"), agent: Some(Agent::ClaudeCode), session: None };
        let release = Event::Released { ticket: 1, keep };
        for (name, events, status) in [
            ("not started", vec![], "not started"),
            ("already released", vec![claimed(1), released(1, None)], "not started"),
            ("merging after its Verdict", vec![claimed(1), completed(1)], "merging"),
            ("merged", vec![claimed(1), completed(1), event("ticket.merged", 1)], "merged"),
            ("skipped", vec![claimed(1), event("ticket.skipped", 1)], "skipped"),
        ] {
            let epic = folded(&[base.to_vec(), events].concat()).unwrap();
            assert_eq!(epic.check(&release), Err(format!("loop/1 is {status}, so it has no Claim to release")), "{name}");
        }
    }

    #[test]
    fn a_review_waits_for_every_ticket_and_remediates_once_every_finding_is_judged() {
        let cut = r#"{"at":2,"kind":"branch.cut","base":"main","worktree":"/w/integration"}"#.to_owned();
        let run = r#"{"at":7,"kind":"run.started","rig":"Claude","parallel":1,"reviewers":["Codex","Claude"],"remediator":"Claude"}"#;
        let base = [ADDED.to_owned(), SETUP.to_owned(), cut, ticket(1, &[]), ticket(2, &[]), run.to_owned(), claimed(1), completed(1)];
        let done = [event("ticket.merged", 1), claimed(2), event("ticket.skipped", 2)];
        let started = |reviewer| Event::ReviewerStarted {
            reviewer,
            rig: "Codex".to_owned(),
            chat: format!("reviewer{reviewer}"),
            worktree: PathBuf::from(format!("/w/review-{reviewer}")),
        };
        let finding = |finding| Event::FindingAdded { finding, rig: "Codex".to_owned(), title: "T".to_owned(), body: "B".to_owned() };
        let judged = |finding| Event::FindingJudged { finding, accepted: true, reason: "R".to_owned() };
        let remediation = || Event::RemediationStarted { rig: "Claude".to_owned(), chat: "remediator".to_owned() };
        let remediated = || Event::Remediated { commit: "abc".to_owned() };
        let reviewers = || Event::RunChanged { rig: None, parallel: None, reviewers: Some(vec![]), remediator: None };

        let merging = folded(&base).unwrap();
        assert_eq!(merging.check(&Event::ReviewStarted), Err("loop/1 is merging, and a Review waits for every Ticket".to_owned()));
        let mut epic = folded(&[base.to_vec(), done.to_vec()].concat()).unwrap();
        let steps: Vec<(Event, Result<(), &str>)> = vec![
            (finding(1), Err("loop is not reviewing")),
            (Event::ReviewStarted, Ok(())),
            (Event::ReviewStarted, Err("loop's Run is already reviewing")),
            (remediation(), Err("loop's Reviewers have not all reported")),
            (started(3), Err("loop's Run names no Reviewer 3")),
            (started(1), Ok(())),
            (started(1), Err("loop's Reviewer 1 has started")),
            (reviewers(), Err("loop's Review has begun, so its Reviewers are settled")),
            (Event::ReviewerReported { reviewer: 2 }, Err("loop's Reviewer 2 has not started")),
            (Event::ReviewerFailed { reviewer: 2, rig: "Claude".to_owned(), reason: "W".to_owned() }, Ok(())),
            (finding(1), Ok(())),
            (finding(1), Err("loop already has Finding 1")),
            (remediation(), Err("loop's Reviewers have not all reported")),
            (Event::ReviewerReported { reviewer: 1 }, Ok(())),
            (Event::ReviewerReported { reviewer: 1 }, Err("loop's Reviewer 1 is done")),
            (judged(1), Err("loop is not remediating")),
            (remediation(), Ok(())),
            (remediated(), Err("loop's Finding 1 is not judged yet; accept or reject every Finding first")),
            (judged(2), Err("loop has no Finding 2")),
            (judged(1), Ok(())),
            (remediated(), Ok(())),
            (judged(1), Err("loop is not remediating")),
            (Event::RunFinished, Ok(())),
            (Event::ReviewStarted, Err("loop has no live Run")),
        ];
        for (event, expected) in steps {
            let checked = epic.check(&event);
            assert_eq!(checked, expected.map_err(str::to_owned), "{event:?}");
            if checked.is_ok() {
                epic.apply(&Entry { at: 9, event });
            }
        }
        let review = epic.review.as_ref().unwrap();
        assert!(review.ended && epic.reviewing().is_none());
        let states: Vec<_> = review.reviewers.iter().map(|reviewer| (reviewer.number, reviewer.reported, reviewer.failed.as_deref())).collect();
        assert_eq!(states, [(1, true, None), (2, false, Some("W"))]);
        assert_eq!(review.remediator.as_ref().unwrap().commit.as_deref(), Some("abc"));
        assert_eq!(epic.findings[0].judgment, Some(Judgment { accepted: true, reason: "R".to_owned(), at: 9 }));

        let unremediated = Event::RunStarted { rig: "Claude".to_owned(), parallel: 1, reviewers: vec!["Codex".to_owned()], remediator: None };
        assert_eq!(epic.check(&unremediated), Err(UNREMEDIATED.to_owned()));
    }
}
