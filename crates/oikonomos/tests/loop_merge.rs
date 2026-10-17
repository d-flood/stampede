mod common;

use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use common::{Cli, Server, control, git, init, next};
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    time::{sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const RIGS: &str = r#"
[[rig]]
name = "Done"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "simple-turn" }

[[rig]]
name = "Resolver"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "FIXTURES/claude-code/resolver.synthetic.jsonl" }
"#;

/// A test server with an Epic `work` of three Tickets in a fresh repository, and a Device watching it.
struct Loop {
    server: Server,
    root: PathBuf,
    cli: Cli,
    repo: PathBuf,
    socket: Socket,
    project_id: String,
    epic: Value,
    statuses: HashMap<String, String>,
}

impl Loop {
    async fn new(prefix: &str) -> Self {
        let fixtures = format!("{}/tests/fixtures/chats", env!("CARGO_MANIFEST_DIR"));
        let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
        let server = Server::with_env(prefix, &RIGS.replace("FIXTURES", &fixtures), vec![("PATH", path.into())]).await;
        let root = fs::canonicalize(server.root.path()).unwrap();
        let repo = init(&root, "repo");
        for (key, value) in [("user.name", "oikonomos"), ("user.email", "test@oikonomos.invalid"), ("commit.gpgsign", "false")] {
            git(&repo, &["config", key, value]);
        }
        fs::write(repo.join("README"), "zero\n").unwrap();
        git(&repo, &["add", "README"]);
        git(&repo, &["commit", "-q", "-m", "Initial commit"]);
        let socket = connect(server.port).await;
        let cli = Cli { data_dir: root.join("data") };
        let mut lp = Self { server, root, cli, repo, socket, project_id: String::new(), epic: Value::Null, statuses: HashMap::new() };
        control(&mut lp.socket, json!({ "type": "project.open", "path": lp.repo })).await;
        let opened = lp.follow(|message| message["type"] == "project.opened").await;
        lp.project_id = opened["projectId"].as_str().unwrap().to_owned();
        lp.ok(&["epic", "add", "work", "--title", "Work", "--brief", "-"], "# Work\n\nThe Brief.\n");
        for (number, title) in [("1", "One"), ("2", "Two"), ("3", "Three")] {
            lp.ok(&["ticket", "add", "work", number, "--title", title, "--body", "-"], &format!("Write {title}.\n"));
        }
        lp.ok(&["epic", "set-setup", "work", ""], "");
        lp.subscribe().await;
        lp
    }

    /// Starts the three Tickets, merges the first, and completes the second, which rewrote the same line of README.
    async fn conflict(&mut self) -> String {
        for (number, rig) in [(1, "Done"), (2, "Resolver"), (3, "Done")] {
            self.start(number, rig).await;
        }
        fs::write(self.worktree("1").join("README"), "one\n").unwrap();
        fs::write(self.worktree("2").join("README"), "two\n").unwrap();
        fs::write(self.worktree("3").join("three.txt"), "three\n").unwrap();
        self.cli.ok(&self.worktree("1"), &["complete", "work/1", "-m", "One"], "");
        self.ticket_until(1, |ticket| ticket["status"] == "merged" && ticket.get("worktree").is_none()).await;
        self.cli.ok(&self.worktree("2"), &["complete", "work/2", "-m", "Two"], "");
        let conflicted = self.ticket_until(2, |ticket| ticket.get("resolver").is_some()).await;
        assert_eq!((&conflicted["status"], conflicted.get("attention")), (&json!("merging"), None));
        conflicted["resolver"].as_str().unwrap().to_owned()
    }

    async fn subscribe(&mut self) {
        control(&mut self.socket, json!({ "type": "epics.subscribe", "projectId": self.project_id })).await;
        let listed = self.follow(|message| message["type"] == "epics.changed").await;
        self.epic = listed["epics"][0].clone();
    }

    async fn restart(&mut self) -> Value {
        let _ = self.socket.close(None).await;
        self.socket = 'restarted: {
            // Under parallel tests a restarted server sometimes resets its first connection, or loses its port.
            for _ in 0..3 {
                self.server.restart().await;
                for _ in 0..5 {
                    if let Ok((mut socket, _)) = connect_async(format!("ws://127.0.0.1:{}/ws", self.server.port)).await {
                        control(&mut socket, json!({ "type": "hello", "deviceId": "loop-merge" })).await;
                        break 'restarted socket;
                    }
                    sleep(Duration::from_millis(100)).await;
                }
            }
            panic!("the server never came back on port {}", self.server.port);
        };
        let hello = self.follow(|message| message["type"] == "hello").await;
        self.subscribe().await;
        hello
    }

    fn ok(&self, args: &[&str], stdin: &str) -> String {
        self.cli.ok(&self.repo, args, stdin)
    }

    fn worktree(&self, name: &str) -> PathBuf {
        self.root.join("data/worktrees/work").join(name)
    }

    async fn start(&mut self, ticket: u32, rig: &str) {
        let start = json!({ "type": "ticket.start", "projectId": self.project_id, "epic": "work", "ticket": ticket, "rig": rig });
        control(&mut self.socket, start).await;
        let reply = self.follow(|message| matches!(message["type"].as_str(), Some("ticket.started" | "ticket.error"))).await;
        assert_eq!(reply["type"], "ticket.started", "{reply}");
    }

    async fn send(&mut self, chat_id: &str, text: &str) {
        control(&mut self.socket, json!({ "type": "chat.send", "chatId": chat_id, "text": text })).await;
    }

    /// Reads the socket until a message satisfies `until`, keeping the latest Epic and Chat statuses.
    async fn follow(&mut self, until: impl Fn(&Value) -> bool) -> Value {
        timeout(Duration::from_secs(20), async {
            loop {
                let message = next(&mut self.socket).await;
                match message["type"].as_str() {
                    Some("epic.changed") => self.epic = message["epic"].clone(),
                    Some("chat.status") => {
                        let status = message["status"].as_str().unwrap().to_owned();
                        self.statuses.insert(message["chatId"].as_str().unwrap().to_owned(), status);
                    }
                    _ => {}
                }
                if until(&message) {
                    return message;
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out with the Epic at {}", self.epic))
    }

    async fn ticket_until(&mut self, number: usize, until: impl Fn(&Value) -> bool) -> Value {
        let ticket = |epic: &Value| epic["tickets"][number - 1].clone();
        if !until(&ticket(&self.epic)) {
            self.follow(|message| message["type"] == "epic.changed" && until(&ticket(&message["epic"]))).await;
        }
        ticket(&self.epic)
    }

    async fn chat_status(&mut self, chat_id: &str, status: &str) {
        if self.statuses.get(chat_id).is_none_or(|seen| seen != status) {
            self.follow(|message| message["type"] == "chat.status" && message["chatId"] == chat_id && message["status"] == status).await;
        }
    }

    /// Lets the merge queue act on whatever it was last told, then shows nothing has merged past the Resolver.
    async fn held(&mut self, third: &str) {
        sleep(Duration::from_millis(500)).await;
        let integration = self.worktree("integration");
        assert!(merging(&integration), "the merge is still in progress");
        assert_eq!(output(&integration, &["log", "--format=%s", "-1"]), "Merge work/1: One");
        assert_eq!(output(&integration, &["diff", "--name-only", "--diff-filter=U"]), "README");
        let statuses: Vec<_> = self.epic["tickets"].as_array().unwrap().iter().map(|ticket| ticket["status"].clone()).collect();
        assert_eq!(statuses, ["merged", "merging", third]);
        assert!(self.epic["tickets"][1].get("attention").is_none() && self.epic["tickets"][2].get("attention").is_none());
    }

    fn log(&self) -> Vec<Value> {
        let log = fs::read_to_string(self.root.join("data/epics/work.jsonl")).unwrap();
        log.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    /// The log from Ticket 2's Verdict on, each kind with its Ticket.
    fn settled(&self) -> Vec<String> {
        let log = self.log();
        let from = log.iter().position(|entry| entry["kind"] == "ticket.completed" && entry["ticket"] == 2).unwrap();
        log[from..].iter().map(|entry| format!("{} {}", entry["kind"].as_str().unwrap(), entry["ticket"])).collect()
    }

    /// Ticket 3 merged after the resolved Ticket 2, and both worktrees are gone.
    async fn resolved(&mut self, resolver: &str, lost: bool) {
        let merged = self.ticket_until(3, |ticket| ticket["status"] == "merged" && ticket.get("worktree").is_none()).await;
        assert_eq!(merged.get("attention"), None);
        let two = self.ticket_until(2, |ticket| ticket.get("worktree").is_none()).await;
        assert_eq!((&two["status"], two.get("resolver"), two.get("attention")), (&json!("merged"), None, None));
        let mut settled = vec!["ticket.completed 2", "merge.conflicted 2", "ticket.completed 3"];
        if lost {
            settled.insert(2, "restore.failed 2");
        }
        settled.extend(["merge.resolved 2", "ticket.merged 3", "worktree.removed 2", "worktree.removed 3"]);
        assert_eq!(self.settled(), settled);
        let conflicted = self.log().into_iter().find(|entry| entry["kind"] == "merge.conflicted").unwrap();
        assert_eq!(conflicted, json!({ "at": conflicted["at"], "kind": "merge.conflicted", "ticket": 2, "resolver": resolver }));

        let integration = self.worktree("integration");
        let subjects = output(&integration, &["log", "--first-parent", "--format=%s"]);
        assert_eq!(subjects, "Merge work/3: Three\nMerge work/2: Two\nMerge work/1: One\nInitial commit");
        let completed = |number: u32| {
            let log = self.log();
            let entry = log.iter().find(|entry| entry["kind"] == "ticket.completed" && entry["ticket"] == number).unwrap();
            entry["commit"].as_str().unwrap().to_owned()
        };
        assert_eq!(output(&integration, &["rev-parse", "HEAD~1^2"]), completed(2), "the Resolver committed the merge git started");
        assert_eq!(output(&integration, &["rev-parse", "HEAD^2"]), completed(3));
        assert_eq!(fs::read_to_string(integration.join("README")).unwrap(), "one\ntwo\n");
        assert_eq!(fs::read_to_string(integration.join("three.txt")).unwrap(), "three\n");
        assert_eq!(output(&integration, &["status", "--porcelain"]), "");
        assert!(!merging(&integration));
        assert!(!self.worktree("2").exists() && !self.worktree("3").exists());
    }
}

async fn connect(port: u16) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "loop-merge" })).await;
    socket
}

async fn received(socket: &mut Socket, kind: &str) -> Value {
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next(socket).await;
            if message["type"] == kind {
                return message;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no {kind} arrived"))
}

fn output(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(output.status.success(), "git {args:?} in {}: {}", dir.display(), String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}

fn merging(dir: &Path) -> bool {
    Command::new("git").arg("-C").arg(dir).args(["rev-parse", "-q", "--verify", "MERGE_HEAD"]).output().unwrap().status.success()
}

fn chat<'a>(chats: &'a Value, id: &str) -> &'a Value {
    chats.as_array().unwrap().iter().find(|chat| chat["id"] == id).unwrap_or_else(|| panic!("no Chat {id} in {chats}"))
}

#[tokio::test]
async fn a_conflicted_merge_is_resolved_by_a_resolver_chat_while_later_merges_wait() {
    let mut lp = Loop::new("loop-merge").await;
    let resolver = lp.conflict().await;
    let integration = lp.worktree("integration");
    assert!(merging(&integration), "the conflicted merge is left in progress");

    lp.chat_status(&resolver, "done").await;
    control(&mut lp.socket, json!({ "type": "chat.attach", "chatId": resolver })).await;
    let items = lp.follow(|message| message["type"] == "chat.items" && message["chatId"] == resolver).await;
    let data = lp.root.join("data").display().to_string();
    let commit = lp.log().iter().find(|entry| entry["kind"] == "ticket.completed" && entry["ticket"] == 2).unwrap()["commit"].clone();
    let merge = format!(
        "Your merge is work/2, Two: its commit `{}` conflicted in README when merged into `oikonomos/work/epic`. On this Host `oikonomos` is `{}`; run it by that path if it is not on your PATH.",
        commit.as_str().unwrap(),
        env!("CARGO_BIN_EXE_oikonomos")
    );
    assert_eq!(items["items"][0], json!({ "kind": "user", "text": format!("/resolver {merge}"), "images": 0, "template": "resolver" }));
    let stdin = fs::read_to_string(lp.root.join("home/.claude/fake-received.jsonl")).unwrap();
    let template = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/templates/resolver.md")).unwrap();
    let note = format!("This Oikonomos server keeps its data in {data}, so pass `--data-dir {data}` to every `oikonomos` command.");
    let prompt = json!(format!("{}\n\n{note}\n\n{merge}", template.trim_end()));
    let sent = stdin.lines().map(|line| serde_json::from_str::<Value>(line).unwrap()).filter(|message| message["type"] == "user");
    assert_eq!(sent.filter(|message| message["message"]["content"] == prompt).count(), 1, "the Resolver got the Template and its merge");

    let mut device = connect(lp.server.port).await;
    let hello = received(&mut device, "hello").await;
    let listed = chat(&hello["chats"], &resolver);
    assert_eq!((&listed["projectId"], &listed["cwd"], &listed["rig"]), (&json!(lp.project_id), &json!(integration), &json!("Resolver")));
    let attention = received(&mut device, "attention.changed").await;
    let item = json!({ "projectId": lp.project_id, "chatId": resolver, "label": "Resolver", "status": "done" });
    assert!(attention["items"].as_array().unwrap().contains(&item), "a stuck Resolver Needs attention: {attention}");
    lp.held("in-progress").await;

    lp.cli.ok(&lp.worktree("3"), &["complete", "work/3", "-m", "Three"], "");
    lp.ticket_until(3, |ticket| ticket["status"] == "merging").await;
    lp.held("merging").await;

    lp.send(&resolver, "Keep both.").await;
    lp.resolved(&resolver, false).await;
}

#[tokio::test]
async fn a_conflict_with_no_resolver_to_start_is_aborted_and_flagged() {
    let mut lp = Loop::new("loop-merge-abort").await;
    lp.start(1, "Done").await;
    lp.start(2, "Done").await;
    fs::write(lp.worktree("1").join("README"), "one\n").unwrap();
    fs::write(lp.worktree("2").join("README"), "two\n").unwrap();
    lp.cli.ok(&lp.worktree("1"), &["complete", "work/1", "-m", "One"], "");
    lp.ticket_until(1, |ticket| ticket["status"] == "merged").await;
    control(&mut lp.socket, json!({ "type": "project.close", "projectId": lp.project_id })).await;
    lp.follow(|message| message["type"] == "projects.changed" && message["projects"] == json!([])).await;

    lp.cli.ok(&lp.worktree("2"), &["complete", "work/2", "-m", "Two"], "");
    let failed = timeout(Duration::from_secs(10), async {
        loop {
            if let Some(failed) = lp.log().into_iter().find(|entry| entry["kind"] == "merge.failed") {
                return failed;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the merge never failed");
    assert_eq!(
        failed["reason"],
        "Its merge into the Integration Worktree conflicted in README, so Oikonomos aborted it. No open Project checks out work's repository, so Oikonomos could not start a Chat."
    );
    let integration = lp.worktree("integration");
    assert!(!merging(&integration));
    assert_eq!(output(&integration, &["status", "--porcelain"]), "");
    assert_eq!(output(&integration, &["log", "--format=%s", "-1"]), "Merge work/1: One");
    assert!(!lp.log().iter().any(|entry| entry["kind"] == "merge.conflicted"));
}

#[tokio::test]
async fn a_restart_recovers_the_resolver_before_the_queue_moves() {
    let mut lp = Loop::new("loop-merge-restart").await;
    let resolver = lp.conflict().await;
    lp.chat_status(&resolver, "done").await;
    lp.cli.ok(&lp.worktree("3"), &["complete", "work/3", "-m", "Three"], "");
    lp.ticket_until(3, |ticket| ticket["status"] == "merging").await;

    let hello = lp.restart().await;
    assert_eq!(chat(&hello["chats"], &resolver)["status"], "done", "the recorded Resolver is restored, not replaced");
    assert_eq!(hello["chats"].as_array().unwrap().len(), 4, "three Ticket Chats and one Resolver");
    assert_eq!(lp.epic["tickets"][1]["resolver"], resolver);
    lp.held("merging").await;
    assert_eq!(lp.log().iter().filter(|entry| entry["kind"] == "merge.conflicted").count(), 1);

    lp.send(&resolver, "Keep both.").await;
    lp.resolved(&resolver, false).await;
}

#[tokio::test]
async fn a_resolver_lost_in_a_restart_flags_its_ticket_and_the_queue_waits_for_the_merge() {
    let mut lp = Loop::new("loop-merge-lost").await;
    let resolver = lp.conflict().await;
    lp.chat_status(&resolver, "done").await;
    control(&mut lp.socket, json!({ "type": "chat.close", "chatId": resolver })).await;
    lp.follow(|message| message["type"] == "chats.changed" && !message["chats"].as_array().unwrap().iter().any(|chat| chat["id"] == resolver))
        .await;

    lp.restart().await;
    let flagged = lp.epic["tickets"][1].clone();
    assert_eq!((&flagged["status"], &flagged["resolver"]), (&json!("merging"), &json!(resolver)));
    let attention = flagged["attention"].as_str().unwrap();
    assert!(attention.starts_with(&format!("Its Resolver Chat {resolver} did not come back")), "{attention}");
    sleep(Duration::from_millis(500)).await;
    let integration = lp.worktree("integration");
    assert!(merging(&integration), "the merge is still in progress");
    assert_eq!(lp.log().iter().filter(|entry| entry["kind"] == "merge.conflicted").count(), 1, "no second Resolver");

    fs::write(integration.join("README"), "one\ntwo\n").unwrap();
    git(&integration, &["add", "README"]);
    git(&integration, &["commit", "-q", "--no-edit"]);
    lp.cli.ok(&lp.worktree("3"), &["complete", "work/3", "-m", "Three"], "");
    lp.resolved(&resolver, true).await;
}
