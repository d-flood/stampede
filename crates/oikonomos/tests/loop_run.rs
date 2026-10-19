mod common;

use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use common::{Cli, Server, control, git, init, leftovers, next, reboot, subscribe};
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    sync::mpsc::UnboundedReceiver,
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const RIGS: &str = r#"
[[rig]]
name = "Verdict"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "FIXTURES/claude-code/verdict.synthetic.jsonl" }

[[rig]]
name = "Done"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "simple-turn" }

[[rig]]
name = "Done Again"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "simple-turn" }

[[rig]]
name = "Codex Done"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "simple-turn" }

[[rig]]
name = "Claude Approval"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "approval" }

[[rig]]
name = "Codex Approval"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "approval" }

[[rig]]
name = "Resume"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "FIXTURES/claude-code/resume-verdict.synthetic.jsonl" }

[[rig]]
name = "Codex Approval Restart"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "approval-restart" }

[[rig]]
name = "Claude Reboot"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "FIXTURES/claude-code/loop-reboot.synthetic.jsonl" }

[[rig]]
name = "Codex Reboot"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "FIXTURES/codex/loop-reboot.synthetic.jsonl" }

[[rig]]
name = "No Session"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "FIXTURES/claude-code/no-session.synthetic.jsonl" }
"#;

/// The session the fake Claude's Verdict fixtures report.
const SESSION: &str = "5678b8c0-71a1-4040-8ccc-e6bcaf84f90e";

/// Long enough for a Run that was going to start another Ticket to have cut its worktree.
const SETTLE: Duration = Duration::from_millis(800);

/// A test server with an Epic `work` in a fresh repository, its Setup Command set, and a Device watching it.
struct Loop {
    server: Server,
    root: PathBuf,
    cli: Cli,
    repo: PathBuf,
    socket: Socket,
    project_id: String,
    hello: Value,
}

impl Loop {
    async fn new(prefix: &str, tickets: &[(u32, &str)]) -> Self {
        Self::with_rigs(prefix, tickets, RIGS).await
    }

    async fn with_rigs(prefix: &str, tickets: &[(u32, &str)], rigs: &str) -> Self {
        Self::with_rigs_env(prefix, tickets, rigs, Vec::new()).await
    }

    async fn with_env(prefix: &str, tickets: &[(u32, &str)], env: Vec<(&'static str, std::ffi::OsString)>) -> Self {
        Self::with_rigs_env(prefix, tickets, RIGS, env).await
    }

    async fn with_rigs_env(prefix: &str, tickets: &[(u32, &str)], rigs: &str, mut env: Vec<(&'static str, std::ffi::OsString)>) -> Self {
        let fixtures = format!("{}/tests/fixtures/chats", env!("CARGO_MANIFEST_DIR"));
        let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
        env.push(("PATH", path.into()));
        let server = Server::with_env(prefix, &rigs.replace("FIXTURES", &fixtures), env).await;
        let root = fs::canonicalize(server.root.path()).unwrap();
        let repo = init(&root, "repo");
        for (key, value) in [("user.name", "oikonomos"), ("user.email", "test@oikonomos.invalid"), ("commit.gpgsign", "false")] {
            git(&repo, &["config", key, value]);
        }
        fs::write(repo.join("README"), "zero\n").unwrap();
        git(&repo, &["add", "README"]);
        git(&repo, &["commit", "-q", "-m", "Initial commit"]);
        let cli = Cli { data_dir: root.join("data") };
        let (socket, project_id, hello) = connect(server.port, &repo).await;
        let lp = Self { server, root, cli, repo, socket, project_id, hello };
        lp.ok(&["epic", "add", "work", "--title", "Work", "--brief", "-"], "# Work\n\nThe Brief.\n");
        for &(number, blockers) in tickets {
            let number = number.to_string();
            let mut args = vec!["ticket", "add", "work", &number, "--title", "A Ticket"];
            if !blockers.is_empty() {
                args.extend(["--blocked-by", blockers]);
            }
            args.extend(["--body", "-"]);
            lp.ok(&args, "Do the work.\n");
        }
        lp.ok(&["epic", "set-setup", "work", ""], "");
        lp
    }

    fn ok(&self, args: &[&str], stdin: &str) -> String {
        self.cli.ok(&self.repo, args, stdin)
    }

    async fn restart(&mut self) {
        self.server.restart().await;
        let client = reqwest::Client::builder().timeout(Duration::from_millis(500)).build().unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while client.get(format!("http://127.0.0.1:{}/api/version", self.server.port)).send().await.is_err() {
            assert!(Instant::now() < deadline, "the restarted server never served requests");
            sleep(Duration::from_millis(50)).await;
        }
        (self.socket, self.project_id, self.hello) = connect(self.server.port, &self.repo).await;
    }

    /// Records an event as a Device does, giving the server's refusal.
    async fn record(&self, event: Value) -> Result<(), String> {
        let response = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{}/api/epics/work", self.server.port))
            .header("content-type", "application/json")
            .body(event.to_string())
            .send()
            .await
            .unwrap();
        if response.status().is_success() { Ok(()) } else { Err(response.text().await.unwrap()) }
    }

    async fn run(&self, rig: &str, parallel: u32) {
        self.record(json!({ "kind": "run.started", "rig": rig, "parallel": parallel })).await.unwrap();
    }

    fn worktree(&self, name: &str) -> PathBuf {
        self.root.join("data/worktrees/work").join(name)
    }

    /// Records the Verdict from the worktree of the Ticket's latest Claim.
    fn complete(&self, number: u64) {
        let worktree = PathBuf::from(self.claim(number)["worktree"].as_str().unwrap());
        self.cli.ok(&worktree, &["complete", &format!("work/{number}"), "-m", "Done by hand"], "");
    }

    fn claim(&self, number: u64) -> Value {
        self.entries("ticket.claimed").into_iter().rev().find(|entry| entry["ticket"] == number).unwrap()
    }

    fn log(&self) -> Vec<Value> {
        let log = fs::read_to_string(self.root.join("data/epics/work.jsonl")).unwrap();
        log.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    fn entries(&self, kind: &str) -> Vec<Value> {
        self.log().into_iter().filter(|entry| entry["kind"] == kind).collect()
    }

    /// The Tickets in the order their Chats were started.
    fn claims(&self) -> Vec<u64> {
        self.entries("ticket.claimed").iter().map(|entry| entry["ticket"].as_u64().unwrap()).collect()
    }

    fn chat_of(&self, number: u64) -> String {
        self.claim(number)["chat"].as_str().unwrap().to_owned()
    }

    async fn until(&self, what: &str, done: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !done(self) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}; the log is {:#?}", self.log());
            sleep(Duration::from_millis(50)).await;
        }
    }

    /// Waits for exactly these Claims, in any order since Tickets start side by side, and checks that no other
    /// Ticket is being started.
    async fn claimed(&self, expected: &[u64]) {
        self.until(&format!("the Claims {expected:?}"), |lp| lp.claims().len() >= expected.len()).await;
        sleep(SETTLE).await;
        let (mut claims, mut expected) = (self.claims(), expected.to_vec());
        claims.sort();
        expected.sort();
        assert_eq!(claims, expected, "no other Ticket is taken");
        for number in 1..=4 {
            if !expected.contains(&number) {
                assert!(!self.worktree(&number.to_string()).exists(), "work/{number} is not being started");
            }
        }
    }

    async fn chat_status(&self, number: u64, status: &str) {
        let record = self.root.join(format!("data/chats/{}.json", self.chat_of(number)));
        let current = || fs::read(&record).ok().and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok()).map(|chat| chat["status"].clone());
        self.until(&format!("work/{number}'s Chat to be {status}"), |_| current() == Some(json!(status))).await;
    }

    async fn finished(&self, runs: usize) {
        self.until("the Run to finish", |lp| lp.entries("run.finished").len() == runs).await;
    }

    /// The opening prompts the fake Claude received, one per Chat that started a Ticket.
    fn prompts(&self) -> usize {
        self.sent().len()
    }

    /// What every Chat of the fake Claude was sent, in order.
    fn sent(&self) -> Vec<String> {
        let received = fs::read_to_string(self.root.join("home/.claude/fake-received.jsonl")).unwrap_or_default();
        let messages = received.lines().map(|line| serde_json::from_str::<Value>(line).unwrap());
        let users = messages.filter(|message| message["type"] == "user");
        users.map(|message| message["message"]["content"].as_str().unwrap().to_owned()).collect()
    }

    fn chat_record(&self, chat_id: &str) -> Option<Value> {
        let bytes = fs::read(self.root.join(format!("data/chats/{chat_id}.json"))).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Makes the Setup Command note each worktree it runs in.
    fn note_setups(&self) {
        let command = format!("basename \"$PWD\" >> {}", self.root.join("setups").display());
        self.ok(&["epic", "set-setup", "work", &command], "");
    }

    fn setups(&self) -> Vec<String> {
        fs::read_to_string(self.root.join("setups")).unwrap_or_default().lines().map(str::to_owned).collect()
    }

    fn worktrees(&self) -> Vec<Value> {
        self.entries("ticket.claimed").iter().map(|entry| entry["worktree"].clone()).collect()
    }

    /// Closes a Ticket's Chat as a Device does, and waits for the Ticket to be given back.
    async fn close(&mut self, number: u64) -> Value {
        let chat_id = self.chat_of(number);
        self.drain().await;
        control(&mut self.socket, json!({ "type": "chat.close", "chatId": chat_id })).await;
        self.given_back(number, &chat_id).await
    }

    /// Reads every message the Device has been sent so far.
    async fn drain(&mut self) {
        while timeout(Duration::from_millis(300), next(&mut self.socket)).await.is_ok() {}
    }

    /// Waits for a Ticket to be given back, keeping its worktree, after what `drain` last read.
    async fn given_back(&mut self, number: u64, chat_id: &str) -> Value {
        self.message(|message| {
            let ticket = &message["epic"]["tickets"][number as usize - 1];
            message["type"] == "epic.changed"
                && ticket["status"] == "not-started"
                && ticket.get("claim").is_none()
                && ticket.get("worktree").is_some()
        })
        .await;
        assert!(self.chat_record(chat_id).is_none(), "work/{number}'s Chat stopped before its Ticket was given back");
        assert!(!leftovers(&self.root).iter().any(|process| process.contains(chat_id)), "{:?}", leftovers(&self.root));
        self.entries("claim.released").pop().unwrap()
    }

    async fn message(&mut self, until: impl Fn(&Value) -> bool) -> Value {
        timeout(Duration::from_secs(20), async {
            loop {
                let message = next(&mut self.socket).await;
                if until(&message) {
                    return message;
                }
            }
        })
        .await
        .expect("the message never arrived")
    }
}

async fn connect(port: u16, repo: &Path) -> (Socket, String, Value) {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "loop-run", "browserId": "near" })).await;
    control(&mut socket, json!({ "type": "project.open", "path": repo })).await;
    let mut hello = Value::Null;
    let project_id = loop {
        let message = next(&mut socket).await;
        match message["type"].as_str() {
            Some("hello") => hello = message,
            Some("project.opened") => break message["projectId"].as_str().unwrap().to_owned(),
            _ => {}
        }
    };
    control(&mut socket, json!({ "type": "epics.subscribe", "projectId": project_id })).await;
    (socket, project_id, hello)
}

fn has_epic(items: &Value) -> bool {
    items.as_array().unwrap().iter().any(|item| item["epic"] == "work")
}

#[tokio::test]
async fn ticket_rigs_override_the_run_default_and_survive_a_restart() {
    let mut lp = Loop::new("trigs", &[(1, ""), (2, ""), (3, "")]).await;
    for (ticket, rig) in [(1, Some("Codex Done")), (2, Some("Done Again")), (3, Some("Done Again")), (3, None)] {
        lp.record(json!({ "kind": "ticket.rig.set", "ticket": ticket, "rig": rig })).await.unwrap();
    }
    lp.restart().await;
    lp.run("Done", 3).await;
    lp.claimed(&[1, 2, 3]).await;
    assert_eq!(lp.claim(1)["rig"], "Codex Done");
    assert_eq!(lp.claim(2)["rig"], "Done Again");
    assert_eq!(lp.claim(3)["rig"], "Done");
    assert_eq!(lp.chat_record(&lp.chat_of(1)).unwrap()["agent"], "codex");
    assert!(lp.record(json!({ "kind": "ticket.rig.set", "ticket": 1, "rig": null })).await.unwrap_err().contains("in progress"));
}

#[tokio::test]
async fn selecting_another_agent_for_a_returned_ticket_keeps_its_worktree() {
    let mut lp = Loop::new("trchange", &[(1, "")]).await;
    lp.run("Done", 1).await;
    lp.claimed(&[1]).await;
    let first = lp.claim(1);
    lp.record(json!({ "kind": "run.paused", "reason": "david" })).await.unwrap();
    lp.close(1).await;
    lp.record(json!({ "kind": "ticket.rig.set", "ticket": 1, "rig": "Codex Done" })).await.unwrap();
    lp.record(json!({ "kind": "run.resumed" })).await.unwrap();
    lp.claimed(&[1, 1]).await;
    let second = lp.claim(1);
    assert_eq!(second["rig"], "Codex Done");
    assert_eq!(second["worktree"], first["worktree"]);
    assert_ne!(second["chat"], first["chat"]);
    assert_eq!(lp.chat_record(&lp.chat_of(1)).unwrap()["agent"], "codex");
}

/// The attention feed as a newly connected Device sees it.
async fn feed(port: u16) -> Value {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "loop-run-feed" })).await;
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next(&mut socket).await;
            if message["type"] == "attention.changed" {
                return message["items"].clone();
            }
        }
    })
    .await
    .expect("the feed never arrived")
}

/// The next push for the Epic, past those for its Chats.
async fn epic_pushed(pushes: &mut UnboundedReceiver<(String, Value)>) -> (String, Value) {
    timeout(Duration::from_secs(20), async {
        loop {
            let pushed = pushes.recv().await.unwrap();
            if pushed.1.get("epic").is_some() {
                return pushed;
            }
        }
    })
    .await
    .expect("no push for the Epic arrived")
}

/// The most Tickets in progress or merging at once, replayed from the log.
fn most_at_once(log: &[Value]) -> usize {
    let (mut busy, mut most) = (0usize, 0);
    for entry in log {
        match entry["kind"].as_str().unwrap() {
            "ticket.claimed" => busy += 1,
            "ticket.merged" | "ticket.skipped" => busy -= 1,
            _ => {}
        }
        most = most.max(busy);
    }
    most
}

async fn works_three_tickets_through(prefix: &str, parallel: u32, order: [u64; 3]) {
    let mut lp = Loop::new(prefix, &[(1, ""), (2, "1"), (3, "")]).await;
    let mut pushes = subscribe(&lp.server, &["away"]).await;
    let refused = lp.record(json!({ "kind": "run.started", "rig": "Nobody", "parallel": parallel })).await;
    assert_eq!(refused, Err("there is no Rig Nobody in the config".to_owned()));
    lp.run("Verdict", parallel).await;
    let refused = lp.record(json!({ "kind": "run.started", "rig": "Verdict", "parallel": parallel })).await;
    assert_eq!(refused, Err("work already has a live Run".to_owned()));
    lp.finished(1).await;

    let mut first = lp.claims();
    first[..parallel as usize].sort();
    assert_eq!(first, order, "Tickets start in number order, and work/2 only once work/1 is merged");
    assert_eq!(most_at_once(&lp.log()), parallel as usize);
    let log = lp.log();
    let finished = log.iter().position(|entry| entry["kind"] == "run.finished").unwrap();
    let merged = log.iter().rposition(|entry| entry["kind"] == "ticket.merged").unwrap();
    assert!(merged < finished, "{log:#?}");
    assert_eq!(lp.entries("ticket.merged").len(), 3);
    assert_eq!(lp.prompts(), 3, "one opening prompt per Ticket");
    lp.until("every Ticket Worktree to be removed", |lp| lp.entries("worktree.removed").len() == 3).await;
    for number in ["1", "2", "3"] {
        assert!(!lp.worktree(number).exists());
    }
    assert_eq!(fs::read_to_string(lp.worktree("integration").join("work.txt")).unwrap(), "Done by the fake.\n");

    let item = json!({ "projectId": lp.project_id, "epic": "work", "label": "Epic work", "status": "done" });
    assert_eq!(epic_pushed(&mut pushes).await, ("away".into(), item.clone()));
    let attention = lp.message(|message| message["type"] == "attention.changed" && has_epic(&message["items"])).await;
    assert!(attention["items"].as_array().unwrap().contains(&item), "{attention}");
    let all_merged = |epic: &Value| epic["tickets"].as_array().unwrap().iter().all(|ticket| ticket["status"] == "merged");
    lp.message(|message| message["type"] == "epic.changed" && all_merged(&message["epic"]) && message["epic"].get("run").is_none()).await;

    control(&mut lp.socket, json!({ "type": "device.view", "chatIds": [], "epics": ["work"] })).await;
    lp.message(|message| message["type"] == "attention.changed" && !has_epic(&message["items"])).await;
    assert_eq!(lp.log().last().unwrap()["kind"], "epic.viewed");
    let claims = lp.claims();
    lp.restart().await;
    sleep(SETTLE).await;
    assert!(!has_epic(&feed(lp.server.port).await), "a viewed finish stays out of the feed after a restart");
    assert_eq!(lp.claims(), claims, "a finished Run starts nothing after a restart");
    assert_eq!(lp.entries("run.finished").len(), 1);
}

#[tokio::test]
async fn one_at_a_time_a_run_works_its_tickets_in_frontier_order_and_finishes() {
    works_three_tickets_through("loop-run-serial", 1, [1, 2, 3]).await;
}

#[tokio::test]
async fn two_at_a_time_a_run_takes_the_unblocked_tickets_first() {
    works_three_tickets_through("loop-run-parallel", 2, [1, 3, 2]).await;
}

#[tokio::test]
async fn an_unseen_finish_needs_attention_across_a_restart_and_a_shown_one_pushes_only_elsewhere() {
    let mut lp = Loop::new("loop-run-shown", &[(1, "")]).await;
    let mut pushes = subscribe(&lp.server, &["near", "away"]).await;
    lp.run("Verdict", 1).await;
    lp.finished(1).await;
    let mut browsers = [epic_pushed(&mut pushes).await.0, epic_pushed(&mut pushes).await.0];
    browsers.sort();
    assert_eq!(browsers, ["away", "near"], "a finish no Device shows pushes to every browser");
    lp.restart().await;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !has_epic(&feed(lp.server.port).await) {
        assert!(Instant::now() < deadline, "an unseen finish is still called after a restart");
        sleep(Duration::from_millis(100)).await;
    }

    lp.ok(&["ticket", "add", "work", "2", "--title", "Another", "--body", "-"], "More.\n");
    control(&mut lp.socket, json!({ "type": "device.view", "chatIds": [], "epics": ["work"] })).await;
    lp.message(|message| message["type"] == "attention.changed" && !has_epic(&message["items"])).await;

    lp.run("Verdict", 1).await;
    lp.finished(2).await;
    assert_eq!(epic_pushed(&mut pushes).await.0, "away");
    lp.until("the finish to be seen", |lp| lp.entries("epic.viewed").len() == 2).await;
    sleep(SETTLE).await;
    assert!(std::iter::from_fn(|| pushes.try_recv().ok()).all(|(_, item)| item.get("epic").is_none()), "the browser showing the Epic gets no push");
    assert!(!has_epic(&feed(lp.server.port).await));
}

#[tokio::test]
async fn the_rig_and_the_limit_change_mid_run_for_the_chats_started_afterwards() {
    let lp = Loop::new("loop-run-change", &[(1, ""), (2, "1"), (3, "")]).await;
    lp.run("Done", 1).await;
    lp.claimed(&[1]).await;
    lp.chat_status(1, "done").await;
    lp.claimed(&[1]).await;

    let zero = lp.record(json!({ "kind": "run.changed", "parallel": 0 })).await;
    assert_eq!(zero, Err("a Run works on at least one Ticket at a time".to_owned()));
    lp.record(json!({ "kind": "run.changed", "rig": "Done Again", "parallel": 2 })).await.unwrap();
    lp.claimed(&[1, 3]).await;
    lp.complete(1);
    lp.claimed(&[1, 3, 2]).await;
    let rigs: Vec<Value> = lp.entries("ticket.claimed").iter().map(|entry| entry["rig"].clone()).collect();
    assert_eq!(rigs, ["Done", "Done Again", "Done Again"]);
    lp.record(json!({ "kind": "run.changed", "parallel": 1 })).await.unwrap();
    lp.complete(3);
    lp.complete(2);
    lp.finished(1).await;
    assert_eq!(most_at_once(&lp.log()), 2);
    let late = lp.record(json!({ "kind": "run.changed", "parallel": 3 })).await;
    assert_eq!(late, Err("work has no live Run".to_owned()));
}

#[tokio::test]
async fn waiting_and_done_without_a_verdict_hold_their_slots_for_both_agents() {
    for rig in ["Claude Approval", "Codex Approval"] {
        let mut lp = Loop::new("loop-run-slots", &[(1, ""), (2, "")]).await;
        lp.run(rig, 1).await;
        lp.claimed(&[1]).await;
        let chat_id = lp.chat_of(1);
        control(&mut lp.socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
        let request = lp.message(|message| message["type"] == "chat.request" && message["chatId"] == chat_id).await;
        lp.chat_status(1, "waiting").await;
        lp.claimed(&[1]).await;

        let answer = json!({ "type": "chat.answer", "chatId": chat_id, "requestId": request["request"]["id"], "answer": { "allow": true } });
        control(&mut lp.socket, answer).await;
        lp.chat_status(1, "done").await;
        lp.claimed(&[1]).await;
        assert!(lp.ok(&["ticket", "work/1"], "").contains("Status: in progress"), "{rig}");

        lp.record(json!({ "kind": "run.changed", "parallel": 2 })).await.unwrap();
        lp.claimed(&[1, 2]).await;
    }
}

#[tokio::test]
async fn a_run_carries_on_through_a_restart_with_the_same_chats_and_no_prompt_sent_twice() {
    let mut lp = Loop::new("loop-run-restart", &[(1, ""), (2, "1"), (3, "")]).await;
    lp.run("Done", 2).await;
    lp.claimed(&[1, 3]).await;
    lp.chat_status(1, "done").await;
    lp.chat_status(3, "done").await;
    let mut chats = vec![lp.chat_of(1), lp.chat_of(3)];
    chats.sort();

    lp.restart().await;
    let mut listed: Vec<String> = lp.hello["chats"].as_array().unwrap().iter().map(|chat| chat["id"].as_str().unwrap().to_owned()).collect();
    listed.sort();
    assert_eq!(listed, chats, "the same Chats, and no others");
    lp.claimed(&[1, 3]).await;
    assert_eq!(lp.prompts(), 2);
    assert!(lp.entries("restore.failed").is_empty());

    lp.complete(1);
    lp.claimed(&[1, 3, 2]).await;
    assert!(!chats.contains(&lp.chat_of(2)));
    lp.complete(3);
    lp.chat_status(2, "done").await;
    lp.complete(2);
    lp.finished(1).await;
    assert_eq!(lp.prompts(), 3, "one opening prompt per Ticket");
    assert_eq!(most_at_once(&lp.log()), 2);
}

#[tokio::test]
async fn a_failing_setup_command_flags_its_ticket_and_the_run_moves_on() {
    let lp = Loop::new("loop-run-setup", &[(1, ""), (2, ""), (3, "1")]).await;
    lp.ok(&["epic", "set-setup", "work", r#"test "$(basename "$PWD")" != 1 || { echo broken; exit 3; }"#], "");
    lp.run("Verdict", 1).await;
    lp.finished(1).await;
    let ticket = lp.ok(&["ticket", "work/1"], "");
    assert!(ticket.contains("Status: not started"), "{ticket}");
    assert_eq!(lp.claims(), [2], "the flagged Ticket is not retried, and its dependent never unblocks");
    assert_eq!(lp.entries("setup.failed").len(), 1);
    assert!(lp.entries("start.failed").is_empty(), "the failed setup already flagged it");

    lp.ok(&["epic", "set-setup", "work", ""], "");
    lp.run("Verdict", 1).await;
    lp.finished(2).await;
    assert_eq!(lp.claims(), [2, 1, 3], "a new Run retries the flagged Ticket");
}

#[tokio::test]
async fn a_chat_that_fails_to_restore_keeps_its_claim_and_worktree_and_no_other_chat_takes_it() {
    let mut lp = Loop::new("loop-run-lost", &[(1, ""), (2, "")]).await;
    lp.run("Codex Done", 1).await;
    lp.claimed(&[1]).await;
    lp.chat_status(1, "done").await;
    let chat_id = lp.chat_of(1);

    lp.server.stop().await;
    let path = lp.root.join(format!("data/chats/{chat_id}.json"));
    let mut record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record.as_object_mut().unwrap().remove("agentSession").expect("a thread id");
    fs::write(&path, record.to_string()).unwrap();
    lp.restart().await;

    assert_eq!(lp.hello["chats"], json!([]));
    lp.until("the lost Chat to be flagged", |lp| !lp.entries("restore.failed").is_empty()).await;
    let ticket = lp.ok(&["ticket", "work/1"], "");
    assert!(ticket.contains("Status: in progress"), "{ticket}");
    assert_eq!(lp.entries("restore.failed")[0]["chat"], chat_id.as_str());
    assert_eq!(lp.chat_of(1), chat_id, "it keeps its Claim");
    assert!(lp.worktree("1").exists());
    lp.claimed(&[1]).await;

    lp.restart().await;
    lp.claimed(&[1]).await;
    assert_eq!(lp.entries("restore.failed").len(), 1, "flagged once");
    lp.record(json!({ "kind": "run.changed", "parallel": 2 })).await.unwrap();
    lp.claimed(&[1, 2]).await;

    lp.drain().await;
    lp.record(json!({ "kind": "ticket.reopened", "ticket": 1 })).await.unwrap();
    let released = lp.given_back(1, &chat_id).await;
    assert_eq!(released["keep"], json!({ "worktree": lp.worktree("1"), "agent": "codex" }), "moved back, it keeps its worktree");
    lp.claimed(&[1, 2, 1]).await;
    assert_eq!(lp.worktrees()[2], json!(lp.worktree("1")), "its flag went with its Claim, so the Run took it again");
    assert_eq!(lp.entries("claim.released").len(), 1);
}

#[tokio::test]
async fn a_ticket_reopened_mid_run_is_taken_again_when_a_slot_frees() {
    let lp = Loop::new("loop-run-reopen", &[(1, ""), (2, "1")]).await;
    lp.run("Done", 1).await;
    lp.claimed(&[1]).await;
    lp.complete(1);
    lp.claimed(&[1, 2]).await;
    lp.until("work/1's cleanup", |lp| !lp.entries("worktree.removed").is_empty()).await;
    lp.record(json!({ "kind": "ticket.reopened", "ticket": 1 })).await.unwrap();
    lp.claimed(&[1, 2]).await;

    lp.complete(2);
    lp.claimed(&[1, 2, 1]).await;
    lp.complete(1);
    lp.finished(1).await;
}

#[tokio::test]
async fn a_paused_run_starts_nothing_through_a_restart_and_a_resume_fills_its_slots() {
    let mut lp = Loop::new("loop-run-pause", &[(1, ""), (2, ""), (3, "")]).await;
    let pause = json!({ "kind": "run.paused", "reason": "david" });
    assert_eq!(lp.record(pause.clone()).await, Err("work has no live Run".to_owned()));
    lp.run("Done", 1).await;
    lp.claimed(&[1]).await;
    assert_eq!(lp.record(json!({ "kind": "run.resumed" })).await, Err("work's Run is not paused".to_owned()));
    lp.record(pause.clone()).await.unwrap();
    assert_eq!(lp.record(pause).await, Err("work's Run is already paused".to_owned()));
    let paused = |message: &Value| message["type"] == "epic.changed" && message["epic"]["run"]["paused"] == json!({ "reason": "david" });
    lp.message(paused).await;

    lp.chat_status(1, "done").await;
    lp.complete(1);
    lp.until("work/1's merge", |lp| !lp.entries("ticket.merged").is_empty()).await;
    lp.record(json!({ "kind": "run.changed", "parallel": 2 })).await.unwrap();
    lp.claimed(&[1]).await;
    lp.restart().await;
    lp.claimed(&[1]).await;
    assert!(lp.entries("run.finished").is_empty());

    lp.record(json!({ "kind": "run.resumed" })).await.unwrap();
    lp.claimed(&[1, 2, 3]).await;
    lp.complete(2);
    lp.complete(3);
    lp.finished(1).await;
}

#[tokio::test]
async fn a_stopped_run_gives_back_its_tickets_and_the_next_run_resumes_each_session_in_its_worktree() {
    let gates = tempfile::tempdir().unwrap();
    let mut lp = Loop::with_env("loop-run-stop", &[(1, "")], vec![("OIKONOMOS_FAKE_GATES", gates.path().into())]).await;
    lp.note_setups();
    lp.run("Resume", 1).await;
    lp.claimed(&[1]).await;
    lp.chat_status(1, "done").await;
    let chat_id = lp.chat_of(1);
    assert_eq!(lp.chat_record(&chat_id).unwrap()["agentSession"], SESSION);

    lp.drain().await;
    lp.record(json!({ "kind": "run.stopped" })).await.unwrap();
    let released = lp.given_back(1, &chat_id).await;
    let keep = json!({ "worktree": lp.worktree("1"), "agent": "claude-code", "session": SESSION });
    assert_eq!((released["ticket"].as_u64(), &released["keep"]), (Some(1), &keep));
    assert!(lp.ok(&["ticket", "work/1"], "").contains("Status: not started"));
    assert!(lp.worktree("1").exists());
    assert_eq!(lp.record(json!({ "kind": "run.stopped" })).await, Err("work has no live Run".to_owned()));
    sleep(SETTLE).await;
    assert_eq!((lp.claims(), lp.entries("claim.released").len()), (vec![1], 1));
    assert!(lp.entries("run.finished").is_empty(), "a stopped Run has not finished");

    lp.run("Resume", 1).await;
    lp.until("the resumed Chat's Verdict gate", |_| gates.path().join("verdict").exists()).await;
    assert_eq!(lp.worktrees(), vec![json!(lp.worktree("1")); 2], "retaken in its old worktree");
    let resumed = lp.chat_of(1);
    assert_eq!(lp.chat_record(&resumed).unwrap()["agentSession"], SESSION);
    assert!(leftovers(&lp.root).iter().any(|process| process.contains(&format!("--resume {SESSION}"))));
    fs::remove_file(gates.path().join("verdict")).unwrap();
    lp.finished(1).await;
    lp.until("the resumed Chat to close", |lp| lp.chat_record(&resumed).is_none()).await;
    assert_eq!(lp.setups(), ["integration", "1"], "its kept worktree is not set up again");
    let sent = lp.sent();
    assert_eq!(sent.len(), 2);
    assert!(sent[0].starts_with("You hold one Ticket"), "{}", sent[0]);
    assert!(sent[1].starts_with("Oikonomos stopped this Chat before work/1 had its Verdict"), "{}", sent[1]);
    assert_eq!(lp.entries("ticket.merged").len(), 1);
    assert_eq!(fs::read_to_string(lp.worktree("integration").join("work.txt")).unwrap(), "Done by the fake.\n");
}

#[tokio::test]
async fn closing_a_loop_chat_gives_back_its_ticket_once_after_its_agent_stops_for_both_agents() {
    for rig in ["Done", "Codex Done"] {
        let mut lp = Loop::new("loop-run-close", &[(1, "")]).await;
        lp.run(rig, 1).await;
        lp.claimed(&[1]).await;
        lp.chat_status(1, "done").await;
        let session = lp.chat_record(&lp.chat_of(1)).unwrap()["agentSession"].clone();
        let agent = if rig == "Done" { "claude-code" } else { "codex" };
        lp.record(json!({ "kind": "run.paused", "reason": "david" })).await.unwrap();

        let released = lp.close(1).await;
        assert_eq!(released["keep"], json!({ "worktree": lp.worktree("1"), "agent": agent, "session": session }), "{rig}");
        sleep(SETTLE).await;
        assert_eq!(lp.entries("claim.released").len(), 1, "{rig}: released once");
        assert_eq!(lp.claims(), [1], "{rig}: the paused Run does not take it back");
        assert!(lp.ok(&["ticket", "work/1"], "").contains("Status: not started"), "{rig}");
        assert!(lp.worktree("1").exists(), "{rig}");
    }
}

#[tokio::test]
async fn an_in_progress_ticket_moved_back_resumes_its_session_and_stopping_gives_it_back_again() {
    let mut lp = Loop::new("loop-run-back", &[(1, "")]).await;
    lp.run("Done", 1).await;
    lp.claimed(&[1]).await;
    lp.chat_status(1, "done").await;
    let first = lp.chat_of(1);
    let session = lp.chat_record(&first).unwrap()["agentSession"].clone();

    lp.drain().await;
    lp.record(json!({ "kind": "ticket.reopened", "ticket": 1 })).await.unwrap();
    lp.given_back(1, &first).await;
    lp.claimed(&[1, 1]).await;
    let second = lp.chat_of(1);
    assert_ne!(second, first);
    assert_eq!(lp.worktrees(), vec![json!(lp.worktree("1")); 2]);
    assert_eq!(lp.chat_record(&second).unwrap()["agentSession"], session);
    lp.chat_status(1, "done").await;

    lp.drain().await;
    lp.record(json!({ "kind": "run.stopped" })).await.unwrap();
    let released = lp.given_back(1, &second).await;
    assert_eq!(released["keep"]["session"], session);
    sleep(SETTLE).await;
    assert_eq!((lp.claims(), lp.entries("claim.released").len()), (vec![1, 1], 2));
    let refused = lp.record(json!({ "kind": "ticket.reopened", "ticket": 1 })).await;
    assert_eq!(refused, Err("work/1 is not started; only an in-progress, merged or skipped Ticket can be moved back to not started".to_owned()));
}

#[tokio::test]
async fn closing_before_the_session_began_keeps_the_worktree_and_starts_a_fresh_session_there() {
    let mut lp = Loop::new("loop-run-fresh", &[(1, "")]).await;
    lp.note_setups();
    lp.run("No Session", 1).await;
    lp.claimed(&[1]).await;
    lp.until("the opening prompt to arrive", |lp| lp.prompts() == 1).await;
    assert_eq!(lp.chat_record(&lp.chat_of(1)).unwrap().get("agentSession"), None);
    lp.record(json!({ "kind": "run.changed", "rig": "Verdict" })).await.unwrap();

    let released = lp.close(1).await;
    assert_eq!(released["keep"], json!({ "worktree": lp.worktree("1"), "agent": "claude-code" }));
    lp.finished(1).await;
    let rigs: Vec<Value> = lp.entries("ticket.claimed").iter().map(|entry| entry["rig"].clone()).collect();
    assert_eq!(rigs, ["No Session", "Verdict"]);
    assert_eq!(lp.worktrees(), vec![json!(lp.worktree("1")); 2]);
    assert_eq!(lp.setups(), ["integration", "1"]);
    let sent = lp.sent();
    assert_eq!(sent.len(), 2);
    assert!(sent.iter().all(|text| text.starts_with("You hold one Ticket")), "a fresh session gets the Template: {sent:?}");
    assert_eq!(lp.entries("claim.released").len(), 1);
}

#[tokio::test]
async fn closing_a_chat_after_its_verdict_leaves_its_ticket_resolved() {
    let mut lp = Loop::new("loop-run-after", &[(1, ""), (2, "")]).await;
    lp.run("Done", 1).await;
    lp.claimed(&[1]).await;
    lp.chat_status(1, "done").await;
    lp.complete(1);
    lp.claimed(&[1, 2]).await;
    lp.until("work/1's merge", |lp| !lp.entries("ticket.merged").is_empty()).await;

    let chat_id = lp.chat_of(1);
    control(&mut lp.socket, json!({ "type": "chat.close", "chatId": chat_id })).await;
    lp.until("work/1's Chat to stop", |lp| lp.chat_record(&chat_id).is_none()).await;
    sleep(SETTLE).await;
    assert!(lp.entries("claim.released").is_empty());
    assert!(lp.ok(&["ticket", "work/1"], "").contains("Status: merged"));
    assert_eq!(lp.claims(), [1, 2]);
}

#[tokio::test]
async fn a_live_runs_worktrees_open_as_projects_beside_its_own_and_close_once_resolved_or_removed() {
    let mut lp = Loop::new("loop-run-nest", &[(1, ""), (2, "")]).await;
    let roles = |message: &Value| -> Vec<Value> {
        message["projects"].as_array().unwrap().iter().filter_map(|project| Some(project.get("worktree")?["role"].clone())).collect()
    };
    lp.run("Done", 2).await;
    lp.claimed(&[1, 2]).await;
    let changed = lp.message(|message| message["type"] == "projects.changed" && roles(message).len() == 3).await;
    let projects = changed["projects"].as_array().unwrap();
    let nested = |role: Value| projects.iter().find(|project| project["worktree"]["role"] == role).unwrap().clone();
    let integration = nested(json!("integration"));
    assert_eq!(integration["path"], lp.worktree("integration").display().to_string());
    assert_eq!(integration["worktree"], json!({ "parent": lp.project_id, "epic": "work", "role": "integration" }));
    for number in [1, 2] {
        let project = nested(json!({ "ticket": number }));
        assert_eq!(project["path"], lp.worktree(&number.to_string()).display().to_string());
        assert_eq!(project["worktree"], json!({ "parent": lp.project_id, "epic": "work", "role": { "ticket": number } }));
        let chat = lp.chat_record(&lp.chat_of(number)).unwrap();
        assert_eq!(chat["projectId"], project["id"], "work/{number}'s Chat belongs to its worktree Project");
    }

    let first = lp.chat_of(1);
    lp.complete(1);
    let changed = lp.message(|message| message["type"] == "projects.changed" && roles(message).len() == 2).await;
    assert!(!roles(&changed).contains(&json!({ "ticket": 1 })), "{changed}");
    lp.until("work/1's Chat to close", |lp| lp.chat_record(&first).is_none()).await;
    lp.complete(2);
    lp.finished(1).await;
    lp.message(|message| message["type"] == "projects.changed" && roles(message).is_empty()).await;
    let second = lp.chat_of(2);
    lp.until("work/2's Chat to close", |lp| lp.chat_record(&second).is_none()).await;
    assert!(leftovers(&lp.root).iter().all(|process| !process.contains(&first) && !process.contains(&second)));
    assert!(lp.entries("claim.released").is_empty(), "closing a worktree Project gives no Ticket back");

    let integration = lp.worktree("integration");
    control(&mut lp.socket, json!({ "type": "project.open", "path": integration })).await;
    lp.message(|message| message["type"] == "projects.changed" && message["projects"].as_array().unwrap().len() == 2).await;
    let landed = reqwest::Client::new().post(format!("http://127.0.0.1:{}/api/epics/work/land", lp.server.port)).send().await.unwrap();
    assert!(landed.status().is_success(), "{}", landed.text().await.unwrap());
    let changed = lp.message(|message| message["type"] == "projects.changed" && message["projects"].as_array().unwrap().len() == 1).await;
    assert_eq!(changed["projects"][0]["id"], lp.project_id, "a Project opened on the Integration Worktree closes once Landing removes it");
}

struct UsageFixture {
    root: tempfile::TempDir,
    rigs: String,
    resets_at: Option<u64>,
}

impl UsageFixture {
    fn new(ending: &str, reset_after: Option<Duration>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let resets_at = reset_after.map(|after| (SystemTime::now() + after).duration_since(UNIX_EPOCH).unwrap().as_secs() * 1000);
        let usage = |used: f64| json!({ "stdout": { "type": "rate_limit_event", "rate_limit_info": { "unifiedWindows": {
            "five_hour": { "utilization": used, "resetsAt": resets_at.map(|at| at / 1000) }
        } } } });
        let mut entries = vec![
            json!({ "stdin": { "type": "control_request", "request_id": "init", "request": { "subtype": "initialize" } } }),
            json!({ "stdout": { "type": "control_response", "response": { "subtype": "success", "request_id": "init", "response": {} } } }),
            json!({ "stdin": { "type": "user" } }),
            json!({ "stdout": { "type": "system", "subtype": "init", "session_id": SESSION } }),
            usage(0.95),
        ];
        if ending == "notice-only" {
            entries.pop();
        }
        if ending == "queued" {
            entries.push(json!({ "gate": "turn" }));
        }
        if ending != "done" {
            entries.push(json!({ "stdout": { "type": "assistant", "error": "rate_limit", "message": { "id": "limited", "content": [{ "type": "text", "text": "Usage limit" }] } } }));
        }
        if ending == "waiting" {
            entries.push(json!({ "stdout": { "type": "control_request", "request_id": "permission", "request": {
                "subtype": "can_use_tool", "tool_name": "Bash", "input": { "command": "echo permission" }
            } } }));
        } else {
            entries.push(json!({ "stdout": { "type": "result", "subtype": "success" } }));
        }
        entries.extend([
            json!({ "gate": "reset" }),
            usage(0.1),
            json!({ "gate": "after-reset" }),
            json!({ "stdin": { "type": "user" } }),
            json!({ "stdout": { "type": "result", "subtype": "success" } }),
        ]);
        if ending == "queued" {
            let completed = entries.pop().unwrap();
            entries.extend([
                json!({ "gate": "continue-done" }),
                completed.clone(),
                json!({ "stdin": { "type": "user" } }),
                completed.clone(),
                json!({ "stdin": { "type": "user" } }),
                completed,
            ]);
        }
        fs::write(root.path().join("usage.jsonl"), entries.into_iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
        let rigs = format!("{RIGS}\n[[rig]]\nname = \"Usage\"\nagent = \"claude-code\"\nenv = {{ OIKONOMOS_FAKE_FIXTURE = \"{}\", OIKONOMOS_FAKE_GATES = \"{}\" }}\n", root.path().join("usage.jsonl").display(), root.path().display());
        Self { root, rigs, resets_at }
    }

    fn release(&self, gate: &str) {
        fs::remove_file(self.root.path().join(gate)).unwrap();
    }

    async fn gated(&self, gate: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.root.path().join(gate).exists() {
            assert!(Instant::now() < deadline, "fake never reached {gate}");
            sleep(Duration::from_millis(20)).await;
        }
    }
}

#[tokio::test]
async fn usage_pauses_pushes_and_continues_a_cut_off_turn_once_across_restarts() {
    let fixture = UsageFixture::new("limit", Some(Duration::from_secs(6)));
    let mut lp = Loop::with_rigs("ur", &[(1, ""), (2, "1")], &fixture.rigs).await;
    let mut pushes = subscribe(&lp.server, &["away"]).await;
    lp.run("Usage", 1).await;
    fixture.gated("reset").await;
    lp.until("the usage pause and cut-off turn", |lp| !lp.entries("run.paused").is_empty() && !lp.entries("turn.cut").is_empty()).await;
    let paused = lp.entries("run.paused").pop().unwrap();
    assert_eq!(paused["reason"], "usage");
    assert_eq!(paused["agent"], "claude-code");
    assert_eq!(paused["window"], "five-hour");
    assert_eq!(paused["resetsAt"], fixture.resets_at.unwrap());
    assert_eq!(epic_pushed(&mut pushes).await.1["status"], "waiting");
    let attention = feed(lp.server.port).await;
    assert!(attention.as_array().unwrap().iter().any(|item| item["epic"] == "work" && item["status"] == "waiting"));
    lp.server.stop().await;
    let until_reset = fixture.resets_at.unwrap().saturating_sub(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64);
    sleep(Duration::from_millis(until_reset + 80)).await;
    fixture.release("reset");
    fixture.gated("after-reset").await;
    lp.restart().await;
    fixture.release("after-reset");
    lp.until("one continue", |lp| lp.sent().iter().filter(|text| *text == "continue").count() == 1).await;
    lp.chat_status(1, "done").await;
    assert_eq!(lp.entries("run.resumed").len(), 1);
    assert_eq!(lp.entries("turn.continued").len(), 1);
    assert!(!has_epic(&feed(lp.server.port).await));
    lp.restart().await;
    sleep(SETTLE).await;
    assert_eq!(lp.sent().iter().filter(|text| *text == "continue").count(), 1);
    assert_eq!(lp.entries("turn.cut").len(), 1);
    assert_eq!(lp.claims(), [1]);
}

#[tokio::test]
async fn usage_resets_resume_normal_done_and_waiting_chats_without_continuing_them() {
    for ending in ["done", "waiting"] {
        let fixture = UsageFixture::new(ending, None);
        let lp = Loop::with_rigs("un", &[(1, "")], &fixture.rigs).await;
        lp.run("Usage", 1).await;
        fixture.gated("reset").await;
        lp.until("usage pause", |lp| !lp.entries("run.paused").is_empty()).await;
        lp.chat_status(1, if ending == "waiting" { "waiting" } else { "done" }).await;
        fixture.release("reset");
        fixture.gated("after-reset").await;
        lp.until("usage reset resumes the Run", |lp| !lp.entries("run.resumed").is_empty()).await;
        sleep(SETTLE).await;
        assert_eq!(lp.sent().len(), 1, "{ending}");
        assert!(lp.entries("turn.cut").is_empty(), "{ending}");
    }
}

#[tokio::test]
async fn usage_from_a_ticket_rig_pauses_a_run_with_another_default_agent() {
    let fixture = UsageFixture::new("limit", None);
    let lp = Loop::with_rigs("trusage", &[(1, "")], &fixture.rigs).await;
    lp.record(json!({ "kind": "ticket.rig.set", "ticket": 1, "rig": "Usage" })).await.unwrap();
    lp.run("Codex Done", 1).await;
    fixture.gated("reset").await;
    lp.until("the assigned Rig's usage pause", |lp| !lp.entries("run.paused").is_empty()).await;
    assert_eq!(lp.claim(1)["rig"], "Usage");
    assert_eq!(lp.entries("run.paused")[0]["agent"], "claude-code");
}

#[tokio::test]
async fn stopping_or_manually_pausing_a_usage_paused_run_cancels_continuation() {
    for action in [json!({ "kind": "run.paused", "reason": "david" }), json!({ "kind": "run.stopped" })] {
        let fixture = UsageFixture::new("limit", None);
        let mut lp = Loop::with_rigs("uc", &[(1, "")], &fixture.rigs).await;
        lp.run("Usage", 1).await;
        fixture.gated("reset").await;
        lp.until("the usage pause and cut-off turn", |lp| !lp.entries("run.paused").is_empty() && !lp.entries("turn.cut").is_empty()).await;
        lp.record(action.clone()).await.unwrap();
        fixture.release("reset");
        sleep(SETTLE).await;
        lp.restart().await;
        sleep(SETTLE).await;
        assert_eq!(lp.sent().len(), 1, "{action}");
        assert!(lp.entries("run.resumed").is_empty());
        assert!(lp.entries("turn.continued").is_empty());
        if action["kind"] == "run.paused" {
            lp.record(json!({ "kind": "run.resumed" })).await.unwrap();
            sleep(SETTLE).await;
            assert_eq!(lp.sent().len(), 1);
        }
    }
}

#[tokio::test]
async fn manual_input_after_a_cut_off_turn_cancels_the_automatic_continue() {
    let fixture = UsageFixture::new("limit", None);
    let mut lp = Loop::with_rigs("um", &[(1, "")], &fixture.rigs).await;
    lp.run("Usage", 1).await;
    fixture.gated("reset").await;
    lp.until("the usage pause and cut-off turn", |lp| !lp.entries("run.paused").is_empty() && !lp.entries("turn.cut").is_empty()).await;
    let chat = lp.chat_of(1);
    control(&mut lp.socket, json!({ "type": "chat.send", "chatId": chat, "text": "I handled this" })).await;
    lp.chat_status(1, "working").await;
    fixture.release("reset");
    fixture.gated("after-reset").await;
    fixture.release("after-reset");
    lp.chat_status(1, "done").await;
    sleep(SETTLE).await;
    assert_eq!(lp.sent().len(), 2);
    assert_eq!(lp.sent()[1], "I handled this");
    assert_eq!(lp.entries("turn.cut").len(), 1);
}

#[tokio::test]
async fn a_known_usage_reset_ignores_early_low_readings_and_resumes_on_time() {
    let fixture = UsageFixture::new("limit", Some(Duration::from_secs(5)));
    let lp = Loop::with_rigs("ut", &[(1, "")], &fixture.rigs).await;
    lp.run("Usage", 1).await;
    fixture.gated("reset").await;
    lp.until("the usage pause and cut-off turn", |lp| !lp.entries("run.paused").is_empty() && !lp.entries("turn.cut").is_empty()).await;
    fixture.release("reset");
    fixture.gated("after-reset").await;
    sleep(SETTLE).await;
    assert!(lp.entries("run.resumed").is_empty());
    fixture.release("after-reset");
    lp.until("the timer resumes and continues", |lp| lp.sent().iter().filter(|text| *text == "continue").count() == 1).await;
    assert_eq!(lp.entries("run.resumed").len(), 1);
}

#[tokio::test]
async fn a_usage_cutoff_missed_before_restart_is_discovered_from_the_restored_transcript() {
    let fixture = UsageFixture::new("limit", None);
    let mut lp = Loop::with_rigs("ud", &[(1, "")], &fixture.rigs).await;
    lp.run("Usage", 1).await;
    fixture.gated("reset").await;
    lp.until("the usage pause and cut-off turn", |lp| !lp.entries("run.paused").is_empty() && !lp.entries("turn.cut").is_empty()).await;
    lp.server.stop().await;
    let without_cut = lp.log().into_iter().filter(|entry| entry["kind"] != "turn.cut").map(|entry| format!("{entry}\n")).collect::<String>();
    fs::write(lp.root.join("data/epics/work.jsonl"), without_cut).unwrap();
    lp.restart().await;
    lp.until("the reconstructed cutoff", |lp| lp.entries("turn.cut").len() == 1).await;
    fixture.release("reset");
    fixture.gated("after-reset").await;
    fixture.release("after-reset");
    lp.until("the recovered continue", |lp| lp.sent().iter().filter(|text| *text == "continue").count() == 1).await;
    lp.chat_status(1, "done").await;
    lp.restart().await;
    sleep(SETTLE).await;
    assert_eq!(lp.sent().iter().filter(|text| *text == "continue").count(), 1);
}

#[tokio::test]
async fn usage_pauses_at_the_configured_threshold_and_rechecks_when_it_changes() {
    let fixture = UsageFixture::new("limit", None);
    let rigs = format!("[loop]\npause_at = 0.99\n{}", fixture.rigs);
    let lp = Loop::with_rigs("uf", &[(1, "")], &rigs).await;
    lp.run("Usage", 1).await;
    fixture.gated("reset").await;
    sleep(SETTLE).await;
    assert!(lp.entries("run.paused").is_empty());
    fs::write(lp.root.join("config.toml"), rigs.replace("0.99", "0.9")).unwrap();
    lp.until("a pause at the new threshold", |lp| lp.entries("run.paused").len() == 1).await;
    fixture.release("reset");
    fixture.gated("after-reset").await;
    fixture.release("after-reset");
    lp.until("one continuation after reset", |lp| lp.sent().iter().filter(|text| *text == "continue").count() == 1).await;
}

#[tokio::test]
async fn a_usage_limit_retains_queued_messages_and_images_until_the_continuation_finishes() {
    let fixture = UsageFixture::new("queued", None);
    let mut lp = Loop::with_rigs("uq", &[(1, "")], &fixture.rigs).await;
    lp.run("Usage", 1).await;
    fixture.gated("turn").await;
    let chat = lp.chat_of(1);
    let image = json!({ "mediaType": "image/png", "data": "aGVsbG8=" });
    control(&mut lp.socket, json!({ "type": "chat.send", "chatId": chat, "text": "First queued", "queue": true, "images": [image] })).await;
    control(&mut lp.socket, json!({ "type": "chat.send", "chatId": chat, "text": "Second queued", "queue": true })).await;
    let queued = |lp: &Loop| lp.chat_record(&chat).is_some_and(|record| record["queued"] == json!(["First queued", "Second queued"]));
    lp.until("both queued messages", queued).await;
    fixture.release("turn");
    fixture.gated("reset").await;
    lp.until("the cut-off turn", |lp| lp.entries("turn.cut").len() == 1).await;
    lp.chat_status(1, "done").await;
    assert!(queued(&lp));
    assert_eq!(lp.sent().len(), 1);
    lp.restart().await;
    assert!(queued(&lp));
    assert_eq!(lp.chat_record(&chat).unwrap()["queuedImages"], json!([[image], []]));
    fixture.release("reset");
    fixture.gated("after-reset").await;
    fixture.release("after-reset");
    fixture.gated("continue-done").await;
    assert_eq!(lp.sent().last().unwrap(), "continue");
    assert!(queued(&lp));
    fixture.release("continue-done");
    lp.chat_status(1, "done").await;
    let received = fs::read_to_string(lp.root.join("home/.claude/fake-received.jsonl")).unwrap();
    let messages: Vec<Value> = received.lines().map(|line| serde_json::from_str::<Value>(line).unwrap()).filter(|message| message["type"] == "user").collect();
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[1]["message"]["content"], "continue");
    assert_eq!(messages[2]["message"]["content"], json!([
        { "type": "text", "text": "First queued" },
        { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "aGVsbG8=" } }
    ]));
    assert_eq!(messages[3]["message"]["content"], "Second queued");
    let record = lp.chat_record(&chat).unwrap();
    assert_eq!(record["queued"], json!([]));
    assert!(record.get("queuedImages").is_none());
    assert_eq!(lp.entries("turn.continued").len(), 1);
}

#[tokio::test]
async fn changing_the_rig_does_not_bypass_the_old_agents_usage_reset() {
    let fixture = UsageFixture::new("limit", Some(Duration::from_secs(5)));
    let lp = Loop::with_rigs("us", &[(1, "")], &fixture.rigs).await;
    lp.run("Usage", 1).await;
    fixture.gated("reset").await;
    lp.until("the usage pause and cut-off turn", |lp| !lp.entries("run.paused").is_empty() && !lp.entries("turn.cut").is_empty()).await;
    lp.record(json!({ "kind": "run.changed", "rig": "Codex Done" })).await.unwrap();
    sleep(SETTLE).await;
    assert!(lp.entries("run.resumed").is_empty());
    assert_eq!(lp.sent().len(), 1);
    fixture.release("reset");
    fixture.gated("after-reset").await;
    fixture.release("after-reset");
    lp.until("the old Agent resets before continuation", |lp| lp.sent().iter().filter(|text| *text == "continue").count() == 1).await;
    assert_eq!(lp.entries("run.resumed").len(), 1);
}

#[tokio::test]
async fn a_limit_notice_without_a_usage_reading_keeps_attention_without_guessing_a_reset() {
    let fixture = UsageFixture::new("notice-only", None);
    let lp = Loop::with_rigs("ul", &[(1, "")], &fixture.rigs).await;
    lp.run("Usage", 1).await;
    fixture.gated("reset").await;
    lp.chat_status(1, "done").await;
    let chat = lp.chat_of(1);
    let attention = feed(lp.server.port).await;
    assert!(attention.as_array().unwrap().iter().any(|item| item["chatId"] == chat && item["status"] == "done"));
    fixture.release("reset");
    fixture.gated("after-reset").await;
    sleep(SETTLE).await;
    assert!(lp.entries("run.paused").is_empty());
    assert!(lp.entries("run.resumed").is_empty());
    assert_eq!(lp.sent().len(), 1);
}

#[tokio::test]
async fn a_usage_pause_without_a_reset_adopts_the_next_readings_reset_time() {
    for seconds_ahead in [0, 4] {
        let fixture = UsageFixture::new("limit", None);
        let reset = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + seconds_ahead;
        let path = fixture.root.path().join("usage.jsonl");
        let mut entries: Vec<Value> = fs::read_to_string(&path).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        let changed = entries.iter_mut().filter(|entry| entry["stdout"]["type"] == "rate_limit_event").nth(1).unwrap();
        changed["stdout"]["rate_limit_info"]["unifiedWindows"]["five_hour"] = json!({ "utilization": 0.95, "resetsAt": reset });
        fs::write(path, entries.into_iter().map(|entry| format!("{entry}\n")).collect::<String>()).unwrap();
        let lp = Loop::with_rigs("ua", &[(1, "")], &fixture.rigs).await;
        lp.run("Usage", 1).await;
        fixture.gated("reset").await;
        lp.until("the initial pause", |lp| !lp.entries("run.paused").is_empty()).await;
        fixture.release("reset");
        fixture.gated("after-reset").await;
        if seconds_ahead > 0 {
            lp.until("the announced reset time", |lp| lp.entries("run.paused").iter().any(|entry| entry["resetsAt"] == reset * 1000)).await;
            assert!(lp.entries("run.resumed").is_empty());
        }
        lp.until("resumption at the learned reset", |lp| lp.entries("run.resumed").len() == 1).await;
        fixture.release("after-reset");
        lp.until("one continue", |lp| lp.sent().iter().filter(|text| *text == "continue").count() == 1).await;
    }
}


fn recovery_inputs(lp: &Loop, rig: &str) -> Vec<Value> {
    let (path, method) = if rig.starts_with("Claude") {
        ("home/.claude/fake-received.jsonl", "user")
    } else {
        ("home/.codex/fake-daemon/received.jsonl", "turn/start")
    };
    fs::read_to_string(lp.root.join(path)).unwrap_or_default().lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|message| message["type"] == method || message["method"] == method).collect()
}

async fn interruptible(lp: &mut Loop, gates: &Path) {
    lp.claimed(&[1]).await;
    lp.chat_status(1, "done").await;
    let chat = lp.chat_of(1);
    control(&mut lp.socket, json!({ "type": "chat.send", "chatId": chat, "text": "Interrupted work",
        "images": [{ "mediaType": "image/png", "data": "eA==" }] })).await;
    lp.until("the interrupted turn's gate", |_| gates.join("running").exists()).await;
    lp.chat_status(1, "working").await;
    control(&mut lp.socket, json!({ "type": "chat.send", "chatId": chat, "text": "Leave this queued", "queue": true,
        "images": [{ "mediaType": "image/png", "data": "eQ==" }] })).await;
    lp.until("the queued attachment", |lp| lp.chat_record(&chat).is_some_and(|record| record["queued"].as_array().is_some_and(|queued| queued.len() == 1))).await;
}

#[tokio::test]
async fn rebooted_loop_chats_retry_once_preserving_attachments_and_live_reattachments() {
    for rig in ["Claude Reboot", "Codex Reboot"] {
        let gates = tempfile::tempdir().unwrap();
        let mut lp = Loop::with_env("lr", &[(1, "")], vec![("OIKONOMOS_FAKE_GATES", gates.path().into())]).await;
        lp.run(rig, 1).await;
        interruptible(&mut lp, gates.path()).await;
        let chat = lp.chat_of(1);
        let queued = lp.chat_record(&chat).unwrap();
        lp.server.stop().await;
        reboot(&lp.root);
        lp.restart().await;
        lp.until("the recovery input", |_| gates.path().join("retried").exists()).await;
        lp.chat_status(1, "working").await;
        assert_eq!(lp.entries("turn.retried").len(), 1);
        let inputs = recovery_inputs(&lp, rig);
        assert_eq!(inputs.len(), 3, "{rig}: {inputs:?}");
        let recovered = if rig.starts_with("Claude") { &inputs[2]["message"]["content"] } else { &inputs[2]["params"]["input"] };
        assert_eq!(recovered[0]["text"], "Interrupted work");
        let image = recovered.as_array().unwrap().iter().find(|item| item["type"] == "image").unwrap();
        if rig.starts_with("Claude") { assert_eq!(image["source"]["data"], "eA=="); }
        else { assert_eq!(image["url"], "data:image/png;base64,eA=="); }
        for key in ["queued", "queuedImages"] { assert_eq!(lp.chat_record(&chat).unwrap()[key], queued[key]); }

        lp.restart().await;
        lp.chat_status(1, "working").await;
        sleep(SETTLE).await;
        assert_eq!(recovery_inputs(&lp, rig).len(), 3, "{rig} live reattachment");
        lp.server.stop().await;
        reboot(&lp.root);
        lp.restart().await;
        lp.chat_status(1, "done").await;
        sleep(SETTLE).await;
        assert_eq!(recovery_inputs(&lp, rig).len(), 3, "{rig} second interruption");
        assert_eq!(lp.entries("turn.retried").len(), 1);
        assert_eq!(lp.chat_of(1), chat);
        assert_eq!(lp.chat_record(&chat).unwrap()["viewedSinceDone"], false);
        for key in ["queued", "queuedImages"] { assert_eq!(lp.chat_record(&chat).unwrap()[key], queued[key]); }
    }
}

#[tokio::test]
async fn paused_and_archived_runs_leave_rebooted_chats_interrupted() {
    for rig in ["Claude Reboot", "Codex Reboot"] {
        for action in ["run.paused", "epic.archived"] {
            let gates = tempfile::tempdir().unwrap();
            let mut lp = Loop::with_env("lx", &[(1, "")], vec![("OIKONOMOS_FAKE_GATES", gates.path().into())]).await;
            lp.run(rig, 1).await;
            interruptible(&mut lp, gates.path()).await;
            let event = if action == "run.paused" { json!({ "kind": action, "reason": "david" }) } else { json!({ "kind": action }) };
            lp.record(event).await.unwrap();
            lp.server.stop().await;
            reboot(&lp.root);
            lp.restart().await;
            lp.chat_status(1, "done").await;
            sleep(SETTLE).await;
            assert_eq!(recovery_inputs(&lp, rig).len(), 2, "{rig} {action}");
            assert!(lp.entries("turn.retried").is_empty());
        }
    }
}


#[tokio::test]
async fn an_explicit_interrupt_never_recovers_even_after_a_paused_reboot() {
    for rig in ["Claude Reboot", "Codex Reboot"] {
        for restored in [false, true] {
            let gates = tempfile::tempdir().unwrap();
            let mut lp = Loop::with_env("li", &[(1, "")], vec![("OIKONOMOS_FAKE_GATES", gates.path().into())]).await;
            lp.run(rig, 1).await;
            interruptible(&mut lp, gates.path()).await;
            let chat = lp.chat_of(1);
            let gate = if restored {
                lp.record(json!({ "kind": "run.paused", "reason": "david" })).await.unwrap();
                lp.server.stop().await;
                reboot(&lp.root);
                lp.restart().await;
                lp.chat_status(1, "done").await;
                control(&mut lp.socket, json!({ "type": "chat.send", "chatId": chat, "text": "Continue by hand" })).await;
                lp.until("the manual continuation", |_| gates.path().join("retried").exists()).await;
                lp.chat_status(1, "working").await;
                "retried"
            } else { "running" };
            control(&mut lp.socket, json!({ "type": "chat.interrupt", "chatId": chat })).await;
            fs::remove_file(gates.path().join(gate)).unwrap();
            lp.chat_status(1, "done").await;
            if restored { lp.record(json!({ "kind": "run.resumed" })).await.unwrap(); }
            sleep(SETTLE).await;
            assert!(lp.entries("turn.retried").is_empty(), "{rig}, restored={restored}");
            assert_eq!(recovery_inputs(&lp, rig).len(), if restored { 3 } else { 2 });
        }
    }
}


fn append_event(lp: &Loop, mut event: Value) {
    use std::io::Write;
    event["at"] = json!(1);
    let mut file = fs::OpenOptions::new().append(true).open(lp.root.join("data/epics/work.jsonl")).unwrap();
    writeln!(file, "{event}").unwrap();
}

#[tokio::test]
async fn stopped_runs_released_claims_and_resolvers_never_receive_recovery_input() {
    for rig in ["Claude Reboot", "Codex Reboot"] {
        for state in ["stopped", "released", "resolver"] {
            let gates = tempfile::tempdir().unwrap();
            let mut lp = Loop::with_env("le", &[(1, "")], vec![("OIKONOMOS_FAKE_GATES", gates.path().into())]).await;
            lp.run(rig, 1).await;
            interruptible(&mut lp, gates.path()).await;
            let chat = lp.chat_of(1);
            let record = lp.chat_record(&chat).unwrap();
            lp.server.stop().await;
            reboot(&lp.root);
            match state {
                "stopped" => append_event(&lp, json!({ "kind": "run.stopped" })),
                "released" => {
                    append_event(&lp, json!({ "kind": "claim.released", "ticket": 1,
                        "keep": { "worktree": lp.claim(1)["worktree"], "agent": record["agent"], "session": record["agentSession"] } }));
                    append_event(&lp, json!({ "kind": "start.failed", "ticket": 1, "reason": "Keep the released Ticket for David" }));
                }
                _ => {
                    append_event(&lp, json!({ "kind": "ticket.completed", "ticket": 1, "commit": "missing-commit" }));
                    append_event(&lp, json!({ "kind": "merge.conflicted", "ticket": 1, "resolver": chat }));
                }
            }
            lp.restart().await;
            if state == "stopped" {
                lp.until("the stopped Claim to be released", |lp| !lp.entries("claim.released").is_empty()).await;
            } else { lp.chat_status(1, "done").await; }
            sleep(SETTLE).await;
            assert_eq!(recovery_inputs(&lp, rig).len(), 2, "{rig} {state}");
            assert!(lp.entries("turn.retried").is_empty());
        }
    }
}

#[tokio::test]
async fn pending_requests_in_live_runs_survive_restart_without_recovery_input() {
    for rig in ["Claude Approval", "Codex Approval Restart"] {
        let mut lp = Loop::new("lp", &[(1, "")]).await;
        lp.run(rig, 1).await;
        lp.claimed(&[1]).await;
        lp.chat_status(1, "waiting").await;
        let chat = lp.chat_of(1);
        control(&mut lp.socket, json!({ "type": "chat.attach", "chatId": chat })).await;
        let before = lp.message(|message| message["type"] == "chat.request" && message["chatId"] == chat).await;
        let sent = recovery_inputs(&lp, rig).len();
        lp.restart().await;
        lp.chat_status(1, "waiting").await;
        control(&mut lp.socket, json!({ "type": "chat.attach", "chatId": chat })).await;
        let after = lp.message(|message| message["type"] == "chat.request" && message["chatId"] == chat).await;
        assert_eq!(after["request"], before["request"]);
        sleep(SETTLE).await;
        assert_eq!(recovery_inputs(&lp, rig).len(), sent);
        assert!(lp.entries("turn.retried").is_empty());
    }
}
