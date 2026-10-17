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
    TicketAdded { ticket: u32, title: String, body: String, blockers: Vec<u32> },
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
    #[serde(rename = "worktree.removed")]
    WorktreeRemoved { ticket: u32 },
    #[serde(rename = "worktree.kept")]
    WorktreeKept { ticket: u32, reason: String },
    #[serde(other)]
    Unknown,
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
    Oikonomos,
}

impl fmt::Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Author::Agent => "agent",
            Author::David => "david",
            Author::Oikonomos => "oikonomos",
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
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Ticket {
    pub title: String,
    pub body: String,
    pub blockers: Vec<u32>,
    pub status: Status,
    pub notes: Vec<Note>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<Claim>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention: Option<String>,
    #[serde(skip)]
    pub commit: Option<String>,
}

/// The Chat that holds a Ticket, or held it when it gave its Verdict.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Claim {
    pub chat: String,
    pub rig: String,
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
        })
    }

    fn apply(&mut self, entry: &Entry) {
        match &entry.event {
            Event::BriefSet { brief } => self.brief = brief.clone(),
            Event::SetupSet { command } => self.setup = Some(command.clone()),
            Event::TicketAdded { ticket, title, body, blockers } => {
                self.tickets.entry(*ticket).or_insert_with(|| Ticket {
                    title: title.clone(),
                    body: body.clone(),
                    blockers: blockers.clone(),
                    status: Status::NotStarted,
                    notes: Vec::new(),
                    claim: None,
                    worktree: None,
                    attention: None,
                    commit: None,
                });
            }
            Event::BodySet { ticket, body } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.body = body.clone();
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
            }
            Event::Claimed { ticket, chat, rig, worktree } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.status = Status::InProgress;
                    ticket.claim = Some(Claim { chat: chat.clone(), rig: rig.clone() });
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
                    self.queue.push(*number);
                }
            }
            Event::Skipped { ticket, reason } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.status = Status::Skipped;
                    ticket.notes.push(Note { author: Author::Agent, text: reason.clone(), at: entry.at });
                }
            }
            Event::Merged { ticket: number } => {
                self.queue.retain(|queued| queued != number);
                if let Some(ticket) = self.tickets.get_mut(number) {
                    ticket.status = Status::Merged;
                    ticket.attention = None;
                }
            }
            Event::MergeFailed { ticket: number, reason } => {
                self.queue.retain(|queued| queued != number);
                if let Some(ticket) = self.tickets.get_mut(number) {
                    ticket.attention = Some(reason.clone());
                }
            }
            Event::WorktreeRemoved { ticket } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.worktree = None;
                }
            }
            Event::WorktreeKept { ticket, reason } => {
                if let Some(ticket) = self.tickets.get_mut(ticket) {
                    ticket.attention = Some(reason.clone());
                }
            }
            Event::EpicAdded { .. } | Event::Unknown => {}
        }
    }

    fn check(&self, event: &Event) -> Result<(), String> {
        let slug = &self.slug;
        match event {
            Event::BriefSet { brief } => filled(brief, "Brief"),
            Event::TicketAdded { ticket, title, body, blockers } => {
                if self.tickets.contains_key(ticket) {
                    return Err(format!("{slug}/{ticket} already exists"));
                }
                one_line(title)?;
                filled(body, "body")?;
                if blockers.contains(ticket) {
                    return Err(format!("{slug}/{ticket} cannot block itself"));
                }
                match blockers.iter().find(|blocker| !self.tickets.contains_key(blocker)) {
                    Some(missing) => Err(format!("{slug} has no Ticket {missing} to block {slug}/{ticket}; add Tickets in dependency order")),
                    None => Ok(()),
                }
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
            Event::SetupFailed { ticket } | Event::WorktreeRemoved { ticket } | Event::WorktreeKept { ticket, .. } => {
                self.ticket(*ticket).map(drop)
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
            _ => Ok(()),
        }
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
        Ok(Self { dir, epics: Mutex::new(epics), changes: broadcast::channel(64).0 })
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

    pub fn list(&self, project: &Path) -> Vec<Epic> {
        self.epics.lock().unwrap().values().filter(|epic| epic.project == project).cloned().collect()
    }

    pub fn add(&self, slug: &str, title: &str, brief: &str, project: &Path) -> Result<(), String> {
        if !valid_slug(slug) {
            return Err(format!("{slug:?} is not a valid slug; use lowercase letters, digits and hyphens, such as chat-ui"));
        }
        one_line(title)?;
        let mut epics = self.epics.lock().unwrap();
        if let Some(existing) = epics.get(slug) {
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
        epics.insert(slug.to_owned(), Epic::added(&entry).expect("an epic.added entry"));
        let _ = self.changes.send(slug.to_owned());
        Ok(())
    }

    pub fn record(&self, slug: &str, event: Event) -> Result<(), String> {
        let mut epics = self.epics.lock().unwrap();
        let epic = epics.get_mut(slug).ok_or_else(|| format!("there is no Epic {slug}"))?;
        epic.check(&event)?;
        let entry = stamp(event);
        let path = self.path(slug);
        OpenOptions::new()
            .append(true)
            .open(&path)
            .and_then(|file| append(file, &entry))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        epic.apply(&entry);
        let _ = self.changes.send(slug.to_owned());
        Ok(())
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
    fn tickets_fold_into_statuses_and_the_frontier() {
        let unknown = [
            r#"{"at":4,"kind":"run.started","rig":"Claude","parallel":2}"#.to_owned(),
            r#"{"at":5,"kind":"claim.released","ticket":1,"keep":{"worktree":"/w"}}"#.to_owned(),
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
            status: Status::NotStarted,
            notes: vec![
                Note { author: Author::Agent, text: "First".to_owned(), at: 6 },
                Note { author: Author::David, text: "Second".to_owned(), at: 7 },
            ],
            claim: None,
            worktree: None,
            attention: None,
            commit: None,
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
                Event::TicketAdded { ticket: 3, title: "T".to_owned(), body: "B".to_owned(), blockers: vec![1, 2] },
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
            (Event::WorktreeRemoved { ticket: 3 }, r#"{"at":9,"kind":"worktree.removed","ticket":3}"#),
            (Event::WorktreeKept { ticket: 3, reason: "R".to_owned() }, r#"{"at":9,"kind":"worktree.kept","ticket":3,"reason":"R"}"#),
        ];
        for (event, line) in cases {
            let entry = Entry { at: 9, event };
            assert_eq!(serde_json::to_string(&entry).unwrap(), line);
            assert_eq!(serde_json::from_str::<Entry>(line).unwrap(), entry);
        }
        let unknown: Entry = serde_json::from_str(r#"{"at":9,"kind":"run.finished"}"#).unwrap();
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
        assert_eq!(held.claim, Some(Claim { chat: "chat1".to_owned(), rig: "Claude".to_owned() }));
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
            claimed(3),
            completed(3),
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
        ];
        for (event, error) in refused {
            assert_eq!(epic.check(&event), Err(error.to_owned()));
        }
        assert_eq!(epic.check(&claim(1)), Ok(()));
        assert_eq!(epic.check(&Event::Merged { ticket: 3 }), Ok(()));
    }
}
