mod common;

use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use common::{Cli, Server, control, git, init, next, subscribe};
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
"#;

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
        let received = fs::read_to_string(self.root.join("home/.claude/fake-received.jsonl")).unwrap_or_default();
        received.lines().filter(|line| serde_json::from_str::<Value>(line).unwrap()["type"] == "user").count()
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
