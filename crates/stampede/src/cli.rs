use std::{
    env, fs,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    net::UnixListener,
};

use crate::{
    epics::{Author, Epic, Epics, Event, Tag},
    git,
    runs::Runs,
};

#[derive(Subcommand)]
pub enum CliCommand {
    /// Create an Epic, or replace its Brief or Setup Command.
    ///
    /// An Epic is a bounded body of work in one Project: a Brief, and the Tickets that carry it out. Its slug, such as chat-ui, names it in every command, and its Tickets are referenced as <slug>/<n>, such as chat-ui/4.
    #[command(subcommand)]
    Epic(EpicCommand),
    /// Print a Ticket, or add a Ticket or replace its body.
    ///
    /// Given a reference such as chat-ui/4, prints that Ticket: its title, its status, its Blockers with their statuses, its body, and its Notes in the order they were added.
    Ticket(TicketArgs),
    /// Print an Epic's Brief.
    ///
    /// The Brief is the document every Ticket of the Epic answers to. Read it before working any of its Tickets.
    Brief {
        /// The Epic's slug, such as chat-ui.
        epic: String,
        #[command(flatten)]
        connection: Connection,
    },
    /// Attach a Note to a Ticket.
    ///
    /// A Note is a durable message on a Ticket: a decision you made, an answer the Ticket asked for, or a hand-off to whoever works it next. Notes are kept in the order they were added and are never edited, and `stampede ticket <epic>/<n>` prints them.
    Note {
        /// The Ticket, written as <epic>/<n>, such as chat-ui/4.
        #[arg(value_name = "EPIC/N")]
        reference: Reference,
        /// The Note's text. Quote it so that it reaches the command as one argument.
        text: String,
        #[command(flatten)]
        connection: Connection,
    },
    /// List the Tickets that can be taken now.
    ///
    /// A Ticket is on the Frontier when it is not started, no Chat holds it, and every Ticket blocking it is merged or skipped. Each line gives one Ticket's reference and title, in number order.
    Frontier {
        /// The Epic's slug, such as chat-ui.
        epic: String,
        #[command(flatten)]
        connection: Connection,
    },
    /// Record that the Ticket you hold is complete.
    ///
    /// Run it from inside the Ticket's worktree, as your last act on the Ticket. It commits everything left uncommitted there, new files included, with the given message, then records the Verdict, and Stampede merges the Ticket into its Epic's Integration Worktree. A clean worktree is accepted, so a Ticket with no diff, or whose work lives in another repository, can still be completed. Only a Ticket in progress takes a Verdict, and only one; it is refused from any other directory.
    Complete {
        /// The Ticket, written as <epic>/<n>, such as chat-ui/4.
        #[arg(value_name = "EPIC/N")]
        reference: Reference,
        /// The commit message for whatever is left uncommitted. Quote it so that it reaches the command as one argument.
        #[arg(short, long)]
        message: String,
        #[command(flatten)]
        connection: Connection,
    },
    /// Record that the Ticket you hold is moot, and retire it.
    ///
    /// Run it from inside the Ticket's worktree when the codebase has already made the Ticket unnecessary. The reason becomes a Note on the Ticket, and the Ticket becomes skipped, which resolves it as a Blocker. Only a Ticket in progress takes a Verdict, and only one; it is refused from any other directory.
    Skip {
        /// The Ticket, written as <epic>/<n>, such as chat-ui/4.
        #[arg(value_name = "EPIC/N")]
        reference: Reference,
        /// Why the Ticket is moot. Quote it so that it reaches the command as one argument.
        reason: String,
        #[command(flatten)]
        connection: Connection,
    },
    /// Record a Finding, list an Epic's Findings, or judge one.
    ///
    /// A Finding is a defect a Reviewer found once every Ticket of an Epic was done. The Run's Remediator judges every Finding, accepting the ones it fixes and rejecting the rest, each with a reason.
    #[command(subcommand)]
    Finding(FindingCommand),
    /// Report that your Review of an Epic is done.
    ///
    /// Run it from inside your Reviewer worktree, as your last act, once every Finding you verified is recorded with `stampede finding add`. Stampede then ends your Chat and discards the worktree. Once every Reviewer has reported, the Run's Remediator judges the Findings.
    Reviewed {
        /// The Epic's slug, such as chat-ui.
        epic: String,
        #[command(flatten)]
        connection: Connection,
    },
    /// Record that you have remediated an Epic.
    ///
    /// Run it from inside the Epic's Integration Worktree, as the Remediator's last act. It commits everything left uncommitted there, new files included, with the given message, then the Run finishes. It is refused while any Finding is unjudged.
    Remediated {
        /// The Epic's slug, such as chat-ui.
        epic: String,
        /// The commit message for whatever is left uncommitted. Quote it so that it reaches the command as one argument.
        #[arg(short, long)]
        message: String,
        #[command(flatten)]
        connection: Connection,
    },
}

#[derive(Subcommand)]
pub enum FindingCommand {
    /// Record a Finding of your Review.
    ///
    /// Run it from inside your Reviewer worktree while you review. An Epic's Findings are numbered from 1 in the order they are recorded, and are never edited.
    Add {
        /// The Epic's slug, such as chat-ui.
        epic: String,
        /// The defect, on one line.
        #[arg(long)]
        title: String,
        /// A Markdown file holding the Finding: where the defect is, what is wrong, a scenario that shows it, and the fix you would make. Pass - to read it from standard input instead.
        #[arg(long, value_name = "FILE")]
        body: PathBuf,
        #[command(flatten)]
        connection: Connection,
    },
    /// Print every Finding of an Epic, with its judgment.
    List {
        /// The Epic's slug, such as chat-ui.
        epic: String,
        #[command(flatten)]
        connection: Connection,
    },
    /// Accept a Finding, which you will fix.
    ///
    /// Run it from inside the Epic's Integration Worktree while you remediate. Judging a Finding again replaces its judgment.
    Accept {
        /// The Epic's slug, such as chat-ui.
        epic: String,
        /// The Finding's number.
        finding: u32,
        /// Why it is worth fixing. Quote it so that it reaches the command as one argument.
        reason: String,
        #[command(flatten)]
        connection: Connection,
    },
    /// Reject a Finding, which stays unfixed.
    ///
    /// Run it from inside the Epic's Integration Worktree while you remediate. Judging a Finding again replaces its judgment.
    Reject {
        /// The Epic's slug, such as chat-ui.
        epic: String,
        /// The Finding's number.
        finding: u32,
        /// Why it is not fixed: wrong, out of scope, deliberate, a duplicate, or not worth its cost. Quote it so that it reaches the command as one argument.
        reason: String,
        #[command(flatten)]
        connection: Connection,
    },
}

#[derive(Subcommand)]
pub enum EpicCommand {
    /// Create an Epic from its Brief.
    ///
    /// The Epic belongs to the Project of the current directory, which Stampede records by its git common directory, so run this inside the repository the Epic will change. A git worktree belongs to the same Project as its main checkout. Once the Epic exists, add its Tickets with `stampede ticket add`.
    Add {
        /// The Epic's slug, made of lowercase letters, digits and hyphens, such as chat-ui. It names the Epic in every reference, so no other Epic in any Project may already use it.
        slug: String,
        /// The Epic's title, on one line.
        #[arg(long)]
        title: String,
        /// A Markdown file holding the Brief, the document every Ticket of the Epic answers to. Pass - to read the Brief from standard input instead.
        #[arg(long, value_name = "FILE")]
        brief: PathBuf,
        #[command(flatten)]
        connection: Connection,
    },
    /// Replace an Epic's Brief.
    ///
    /// The new Brief replaces the old one whole; there is no partial edit.
    SetBrief {
        /// The Epic's slug, such as chat-ui.
        epic: String,
        /// A Markdown file holding the new Brief. Pass - to read it from standard input instead.
        #[arg(value_name = "FILE")]
        brief: PathBuf,
        #[command(flatten)]
        connection: Connection,
    },
    /// Record the Setup Command that makes a fresh worktree of the Epic runnable.
    ///
    /// The Setup Command is run in every new Ticket Worktree and Integration Worktree before any Chat starts there, for example to install dependencies. A Run refuses to start until the Epic has one, so record the empty string "" when a fresh worktree needs nothing. Recording it again replaces the old one.
    SetSetup {
        /// The Epic's slug, such as chat-ui.
        epic: String,
        /// The Setup Command, such as "pnpm install". Quote it so that it reaches the command as one argument.
        #[arg(value_name = "COMMAND")]
        setup: String,
        #[command(flatten)]
        connection: Connection,
    },
}

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
pub struct TicketArgs {
    #[command(subcommand)]
    command: Option<TicketCommand>,
    /// The Ticket to print, written as <epic>/<n>, such as chat-ui/4.
    #[arg(value_name = "EPIC/N", required = true)]
    reference: Option<Reference>,
    #[command(flatten)]
    connection: Connection,
}

#[derive(Subcommand)]
pub enum TicketCommand {
    /// Add a Ticket to an Epic.
    ///
    /// Add Tickets in dependency order, since each Blocker must already exist. A new Ticket is not started, and it reaches the Frontier once every one of its Blockers is merged or skipped.
    Add {
        /// The slug of the Epic the Ticket belongs to, such as chat-ui.
        epic: String,
        /// The Ticket's number, unused by any other Ticket of the Epic. With the slug it makes the Ticket's reference, such as chat-ui/4.
        #[arg(value_parser = clap::value_parser!(u32).range(1..))]
        number: u32,
        /// The Ticket's title, on one line.
        #[arg(long)]
        title: String,
        /// The numbers of the Tickets of the same Epic that must be merged or skipped before this one can be taken, separated by commas, such as 1,3. Leave it out when nothing blocks the Ticket.
        #[arg(long, value_name = "N,...", value_delimiter = ',')]
        blocked_by: Vec<u32>,
        #[arg(long, value_enum, value_delimiter = ',', help = "Optional Ticket tags")]
        tags: Vec<Tag>,
        /// A Markdown file holding the Ticket's body, which is what the Chat that works the Ticket is given. Pass - to read the body from standard input instead.
        #[arg(long, value_name = "FILE")]
        body: PathBuf,
        #[command(flatten)]
        connection: Connection,
    },
    /// Replace a Ticket's body.
    ///
    /// The new body replaces the old one whole; there is no partial edit.
    SetBody {
        /// The Ticket, written as <epic>/<n>, such as chat-ui/4.
        #[arg(value_name = "EPIC/N")]
        reference: Reference,
        /// A Markdown file holding the new body. Pass - to read it from standard input instead.
        #[arg(value_name = "FILE")]
        body: PathBuf,
        #[command(flatten)]
        connection: Connection,
    },
    #[command(about = "Replace a Ticket's tags; omit tags to clear them")]
    SetTags {
        #[arg(value_name = "EPIC/N")]
        reference: Reference,
        #[arg(value_enum, value_delimiter = ',')]
        tags: Vec<Tag>,
        #[command(flatten)]
        connection: Connection,
    },
}

#[derive(Args)]
pub struct Connection {
    /// The data directory of the Stampede server to talk to. The command reaches the server through the socket it keeps there, so this must match the server's own --data-dir.
    #[arg(long, value_name = "DIR", default_value_os_t = crate::data_dir())]
    data_dir: PathBuf,
}

#[derive(Clone)]
pub struct Reference {
    epic: String,
    ticket: u32,
}

impl FromStr for Reference {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        text.rsplit_once('/')
            .and_then(|(epic, ticket)| Some(Self { epic: epic.to_owned(), ticket: ticket.parse().ok()? }))
            .filter(|reference| !reference.epic.is_empty())
            .ok_or_else(|| "expected <epic>/<n>, such as chat-ui/4".to_owned())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum Request {
    #[serde(rename = "epic.add")]
    AddEpic { slug: String, title: String, brief: String, cwd: PathBuf },
    #[serde(rename = "epic.set-brief")]
    SetBrief { epic: String, brief: String },
    #[serde(rename = "epic.set-setup")]
    SetSetup { epic: String, setup: String },
    #[serde(rename = "ticket.add")]
    AddTicket { epic: String, ticket: u32, title: String, blockers: Vec<u32>, body: String, #[serde(default)] tags: Vec<Tag> },
    #[serde(rename = "ticket")]
    Ticket { epic: String, ticket: u32 },
    #[serde(rename = "ticket.set-body")]
    SetBody { epic: String, ticket: u32, body: String },
    #[serde(rename = "ticket.set-tags")]
    SetTags { epic: String, ticket: u32, tags: Vec<Tag> },
    #[serde(rename = "brief")]
    Brief { epic: String },
    #[serde(rename = "note")]
    Note { epic: String, ticket: u32, text: String },
    #[serde(rename = "frontier")]
    Frontier { epic: String },
    #[serde(rename = "complete")]
    Complete { epic: String, ticket: u32, message: String, cwd: PathBuf },
    #[serde(rename = "skip")]
    Skip { epic: String, ticket: u32, reason: String, cwd: PathBuf },
    #[serde(rename = "finding.add")]
    AddFinding { epic: String, title: String, body: String, cwd: PathBuf },
    #[serde(rename = "finding.list")]
    Findings { epic: String },
    #[serde(rename = "finding.judge")]
    Judge { epic: String, finding: u32, accepted: bool, reason: String, cwd: PathBuf },
    #[serde(rename = "reviewed")]
    Reviewed { epic: String, cwd: PathBuf },
    #[serde(rename = "remediated")]
    Remediated { epic: String, message: String, cwd: PathBuf },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Response {
    Output(String),
    Error(String),
}

fn socket_path(data_dir: &Path) -> PathBuf {
    data_dir.join("cli.sock")
}

pub fn cli(command: CliCommand) -> Result<(), String> {
    let (connection, request) = request(command)?;
    let path = socket_path(&connection.data_dir);
    let unreachable = |error: io::Error| {
        format!("cannot reach the Stampede server at {}: {error}; is it running with this --data-dir?", path.display())
    };
    let address = crate::unix::path(&path).map_err(unreachable)?;
    let mut stream = UnixStream::connect(&address).map_err(unreachable)?;
    let mut line = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    line.push(b'\n');
    stream.write_all(&line).map_err(unreachable)?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).map_err(unreachable)?;
    match serde_json::from_str(&reply).map_err(|error| format!("cannot read the server's reply: {error}"))? {
        Response::Output(text) if text.ends_with('\n') => print!("{text}"),
        Response::Output(text) => println!("{text}"),
        Response::Error(message) => return Err(message),
    }
    Ok(())
}

fn cwd() -> Result<PathBuf, String> {
    env::current_dir().map_err(|error| format!("cannot read the current directory: {error}"))
}

fn request(command: CliCommand) -> Result<(Connection, Request), String> {
    Ok(match command {
        CliCommand::Epic(EpicCommand::Add { slug, title, brief, connection }) => {
            (connection, Request::AddEpic { slug, title, brief: read(&brief, "Brief")?, cwd: cwd()? })
        }
        CliCommand::Epic(EpicCommand::SetBrief { epic, brief, connection }) => {
            (connection, Request::SetBrief { epic, brief: read(&brief, "Brief")? })
        }
        CliCommand::Epic(EpicCommand::SetSetup { epic, setup, connection }) => (connection, Request::SetSetup { epic, setup }),
        CliCommand::Ticket(TicketArgs { command: Some(TicketCommand::Add { epic, number, title, blocked_by, tags, body, connection }), .. }) => {
            let body = read(&body, "body")?;
            (connection, Request::AddTicket { epic, ticket: number, title, blockers: blocked_by, tags, body })
        }
        CliCommand::Ticket(TicketArgs { command: Some(TicketCommand::SetBody { reference, body, connection }), .. }) => {
            let Reference { epic, ticket } = reference;
            (connection, Request::SetBody { epic, ticket, body: read(&body, "body")? })
        }
        CliCommand::Ticket(TicketArgs { command: Some(TicketCommand::SetTags { reference, tags, connection }), .. }) => {
            let Reference { epic, ticket } = reference;
            (connection, Request::SetTags { epic, ticket, tags })
        }
        CliCommand::Ticket(TicketArgs { command: None, reference, connection }) => {
            let Reference { epic, ticket } = reference.expect("clap requires a reference without a subcommand");
            (connection, Request::Ticket { epic, ticket })
        }
        CliCommand::Brief { epic, connection } => (connection, Request::Brief { epic }),
        CliCommand::Note { reference: Reference { epic, ticket }, text, connection } => {
            (connection, Request::Note { epic, ticket, text })
        }
        CliCommand::Frontier { epic, connection } => (connection, Request::Frontier { epic }),
        CliCommand::Complete { reference: Reference { epic, ticket }, message, connection } => {
            (connection, Request::Complete { epic, ticket, message, cwd: cwd()? })
        }
        CliCommand::Skip { reference: Reference { epic, ticket }, reason, connection } => {
            (connection, Request::Skip { epic, ticket, reason, cwd: cwd()? })
        }
        CliCommand::Finding(FindingCommand::Add { epic, title, body, connection }) => {
            (connection, Request::AddFinding { epic, title, body: read(&body, "body")?, cwd: cwd()? })
        }
        CliCommand::Finding(FindingCommand::List { epic, connection }) => (connection, Request::Findings { epic }),
        CliCommand::Finding(FindingCommand::Accept { epic, finding, reason, connection }) => {
            (connection, Request::Judge { epic, finding, accepted: true, reason, cwd: cwd()? })
        }
        CliCommand::Finding(FindingCommand::Reject { epic, finding, reason, connection }) => {
            (connection, Request::Judge { epic, finding, accepted: false, reason, cwd: cwd()? })
        }
        CliCommand::Reviewed { epic, connection } => (connection, Request::Reviewed { epic, cwd: cwd()? }),
        CliCommand::Remediated { epic, message, connection } => (connection, Request::Remediated { epic, message, cwd: cwd()? }),
    })
}

fn read(source: &Path, what: &str) -> Result<String, String> {
    let text = if source == Path::new("-") {
        let mut text = String::new();
        io::stdin().read_to_string(&mut text).map(|_| text).map_err(|error| format!("cannot read the {what} from standard input: {error}"))
    } else {
        fs::read_to_string(source).map_err(|error| format!("cannot read the {what} from {}: {error}", source.display()))
    }?;
    if text.trim().is_empty() {
        return Err(format!("the {what} is empty"));
    }
    Ok(text)
}

/// Takes the socket early, so CLI requests made while the server is starting wait for `listen` instead of failing.
pub fn bind(data_dir: &Path) -> io::Result<UnixListener> {
    let path = socket_path(data_dir);
    match fs::remove_file(&path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let address = crate::unix::path(&path)?;
    UnixListener::bind(&address)
}

pub fn listen(listener: UnixListener, epics: Arc<Epics>, runs: Arc<Runs>) {
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    tokio::spawn(respond(stream, Arc::clone(&epics), Arc::clone(&runs)));
                }
                Err(error) => eprintln!("cannot accept a CLI connection: {error}"),
            }
        }
    });
}

async fn respond(stream: tokio::net::UnixStream, epics: Arc<Epics>, runs: Arc<Runs>) {
    let (reader, mut writer) = stream.into_split();
    let mut line = String::new();
    if tokio::io::BufReader::new(reader).read_line(&mut line).await.is_err() {
        return;
    }
    let handled = match serde_json::from_str(&line) {
        Ok(request) => handle(&epics, &runs, request).await,
        Err(error) => Err(format!("cannot read the request: {error}")),
    };
    let response = handled.map_or_else(Response::Error, Response::Output);
    let Ok(mut reply) = serde_json::to_vec(&response) else { return };
    reply.push(b'\n');
    let _ = writer.write_all(&reply).await;
}

async fn handle(epics: &Epics, runs: &Runs, request: Request) -> Result<String, String> {
    match request {
        Request::AddEpic { slug, title, brief, cwd } => {
            let project = git::common_dir(&cwd)
                .await
                .ok_or_else(|| format!("{} is not inside a git repository, so it names no Project", cwd.display()))?;
            epics.add(&slug, &title, &brief, &project)?;
            Ok(format!("Added Epic {slug} to the Project at {}.", project.display()))
        }
        Request::SetBrief { epic, brief } => {
            epics.record(&epic, Event::BriefSet { brief })?;
            Ok(format!("Replaced the Brief of {epic}."))
        }
        Request::SetSetup { epic, setup } => {
            epics.record(&epic, Event::SetupSet { command: setup })?;
            Ok(format!("Recorded the Setup Command of {epic}."))
        }
        Request::AddTicket { epic, ticket, title, blockers, body, tags } => {
            epics.record(&epic, Event::TicketAdded { ticket, title, body, blockers, tags })?;
            Ok(format!("Added {epic}/{ticket}."))
        }
        Request::Ticket { epic, ticket } => show(&epics.get(&epic)?, ticket),
        Request::SetBody { epic, ticket, body } => {
            epics.record(&epic, Event::BodySet { ticket, body })?;
            Ok(format!("Replaced the body of {epic}/{ticket}."))
        }
        Request::SetTags { epic, ticket, tags } => {
            epics.record(&epic, Event::TagsSet { ticket, tags })?;
            Ok(format!("Replaced the tags of {epic}/{ticket}."))
        }
        Request::Brief { epic } => Ok(epics.get(&epic)?.brief),
        Request::Note { epic, ticket, text } => {
            epics.record(&epic, Event::NoteAdded { ticket, author: Author::Agent, text })?;
            Ok(format!("Added a Note to {epic}/{ticket}."))
        }
        Request::Frontier { epic } => {
            let epic = epics.get(&epic)?;
            let lines: Vec<_> = epic.frontier().map(|(number, ticket)| format!("{}/{number} {}", epic.slug, ticket.title)).collect();
            Ok(if lines.is_empty() { format!("Nothing in {} is on the Frontier.", epic.slug) } else { lines.join("\n") })
        }
        Request::Complete { epic, ticket, message, cwd } => runs.complete(&epic, ticket, &cwd, &message).await,
        Request::Skip { epic, ticket, reason, cwd } => runs.skip(&epic, ticket, &cwd, &reason),
        Request::AddFinding { epic, title, body, cwd } => runs.add_finding(&epic, &cwd, &title, &body),
        Request::Findings { epic } => Ok(findings(&epics.get(&epic)?)),
        Request::Judge { epic, finding, accepted, reason, cwd } => runs.judge(&epic, finding, accepted, &reason, &cwd),
        Request::Reviewed { epic, cwd } => runs.reviewed(&epic, &cwd),
        Request::Remediated { epic, message, cwd } => runs.remediated(&epic, &cwd, &message).await,
    }
}

fn findings(epic: &Epic) -> String {
    if epic.findings.is_empty() {
        return format!("{} has no Findings.", epic.slug);
    }
    let mut text = String::new();
    for finding in &epic.findings {
        let judgment = match &finding.judgment {
            Some(judgment) => format!("{}: {}", if judgment.accepted { "Accepted" } else { "Rejected" }, judgment.reason.trim_end()),
            None => "Not judged yet".to_owned(),
        };
        text.push_str(&format!(
            "# Finding {}: {}\n\nFound by: {}\n{judgment}\n\n{}\n\n",
            finding.number,
            finding.title,
            finding.rig,
            finding.body.trim_end()
        ));
    }
    text.truncate(text.trim_end().len());
    text.push('\n');
    text
}

fn show(epic: &Epic, number: u32) -> Result<String, String> {
    let ticket = epic.ticket(number)?;
    let slug = &epic.slug;
    let status = |blocker| epic.tickets.get(blocker).map_or("missing".to_owned(), |blocker| blocker.status.to_string());
    let blockers: Vec<_> = ticket.blockers.iter().map(|blocker| format!("{slug}/{blocker} ({})", status(blocker))).collect();
    let blockers = if blockers.is_empty() { "nothing".to_owned() } else { blockers.join(", ") };
    let mut metadata = String::new();
    if !ticket.tags.is_empty() {
        metadata.push_str(&format!("Tags: {}\n", ticket.tags.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")));
    }
    if let Some(rig) = &ticket.rig {
        metadata.push_str(&format!("Rig: {rig}\n"));
    }
    let mut text = format!(
        "# {slug}/{number}: {}\n\nStatus: {}\nBlocked by: {blockers}\n{metadata}\n{}\n",
        ticket.title,
        ticket.status,
        ticket.body.trim_end()
    );
    if !ticket.notes.is_empty() {
        text.push_str("\n## Notes\n\n");
        for note in &ticket.notes {
            text.push_str(&format!("- {}: {}\n", note.author, note.text.trim_end().replace('\n', "\n  ")));
        }
    }
    Ok(text)
}
