use std::{
    collections::BTreeMap,
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
}

impl fmt::Display for Author {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Author::Agent => "agent",
            Author::David => "david",
        })
    }
}

#[cfg_attr(not(test), expect(dead_code, reason = "Claim, Verdict and merge events set the other statuses"))]
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
    #[serde(serialize_with = "numbered")]
    pub tickets: BTreeMap<u32, Ticket>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Ticket {
    pub title: String,
    pub body: String,
    pub blockers: Vec<u32>,
    pub status: Status,
    pub notes: Vec<Note>,
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
            tickets: BTreeMap::new(),
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
            Event::EpicAdded { .. } | Event::Unknown => {}
        }
    }

    fn check(&self, event: &Event) -> Result<(), String> {
        let slug = &self.slug;
        match event {
            Event::TicketAdded { ticket, title, blockers, .. } => {
                if self.tickets.contains_key(ticket) {
                    return Err(format!("{slug}/{ticket} already exists"));
                }
                one_line(title)?;
                if blockers.contains(ticket) {
                    return Err(format!("{slug}/{ticket} cannot block itself"));
                }
                match blockers.iter().find(|blocker| !self.tickets.contains_key(blocker)) {
                    Some(missing) => Err(format!("{slug} has no Ticket {missing} to block {slug}/{ticket}; add Tickets in dependency order")),
                    None => Ok(()),
                }
            }
            Event::BodySet { ticket, .. } | Event::BlockersSet { ticket, .. } => self.ticket(*ticket).map(drop),
            Event::NoteAdded { ticket, text, .. } => {
                self.ticket(*ticket)?;
                if text.trim().is_empty() { Err("the Note is empty".to_owned()) } else { Ok(()) }
            }
            _ => Ok(()),
        }
    }

    pub fn ticket(&self, number: u32) -> Result<&Ticket, String> {
        self.tickets.get(&number).ok_or_else(|| format!("{} has no Ticket {number}", self.slug))
    }

    pub fn frontier(&self) -> impl Iterator<Item = (u32, &Ticket)> {
        let resolved = |blocker: &u32| self.tickets.get(blocker).is_some_and(|ticket| ticket.status.resolved());
        self.tickets
            .iter()
            .filter(move |(_, ticket)| ticket.status == Status::NotStarted && ticket.blockers.iter().all(resolved))
            .map(|(number, ticket)| (*number, ticket))
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
            r#"{"at":5,"kind":"ticket.claimed","ticket":1,"chat":"0123","worktree":"/w"}"#.to_owned(),
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
        };
        let expected = Epic {
            slug: "loop".to_owned(),
            title: "The loop".to_owned(),
            project: PathBuf::from("/repo/.git"),
            brief: "Second brief".to_owned(),
            setup: Some(String::new()),
            archived: true,
            tickets: BTreeMap::from([(1, ticket)]),
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
        ];
        for (event, line) in cases {
            let entry = Entry { at: 9, event };
            assert_eq!(serde_json::to_string(&entry).unwrap(), line);
            assert_eq!(serde_json::from_str::<Entry>(line).unwrap(), entry);
        }
        let unknown: Entry = serde_json::from_str(r#"{"at":9,"kind":"ticket.merged","ticket":3}"#).unwrap();
        assert_eq!(unknown.event, Event::Unknown);
    }
}
