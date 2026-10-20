mod common;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use common::{Cli, Server, control, git, init, next};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{
    net::TcpStream,
    time::{Instant, sleep},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const RIGS: &str = r#"
[[rig]]
name = "Done"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "simple-turn" }

[[rig]]
name = "Reviewer A"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "FIXTURES/claude-code/reviewer.synthetic.jsonl" }

[[rig]]
name = "Reviewer B"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "FIXTURES/claude-code/reviewer.synthetic.jsonl" }

[[rig]]
name = "Clean Reviewer"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "FIXTURES/claude-code/reviewer-clean.synthetic.jsonl" }

[[rig]]
name = "Remediator"
agent = "claude-code"
env = { STAMPEDE_FAKE_FIXTURE = "FIXTURES/claude-code/remediator.synthetic.jsonl" }
"#;

/// Long enough for a Run that was going to start another Chat to have cut its worktree.
const SETTLE: Duration = Duration::from_millis(800);

/// A test server with an Epic `work` of one Ticket in a fresh repository, its fake Agents held at gates.
struct Loop {
    _server: Server,
    _socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    gates: TempDir,
    root: PathBuf,
    cli: Cli,
    repo: PathBuf,
    port: u16,
}

impl Loop {
    async fn new(prefix: &str) -> Self {
        let gates = tempfile::tempdir().unwrap();
        let fixtures = format!("{}/tests/fixtures/chats", env!("CARGO_MANIFEST_DIR"));
        let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
        let env = vec![("PATH", path.into()), ("STAMPEDE_FAKE_GATES", gates.path().into())];
        let server = Server::with_env(prefix, &RIGS.replace("FIXTURES", &fixtures), env).await;
        let root = fs::canonicalize(server.root.path()).unwrap();
        let repo = init(&root, "repo");
        for (key, value) in [("user.name", "stampede"), ("user.email", "test@stampede.invalid"), ("commit.gpgsign", "false")] {
            git(&repo, &["config", key, value]);
        }
        fs::write(repo.join("README"), "zero\n").unwrap();
        git(&repo, &["add", "README"]);
        git(&repo, &["commit", "-q", "-m", "Initial commit"]);
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
        control(&mut socket, json!({ "type": "hello", "deviceId": "loop-review" })).await;
        control(&mut socket, json!({ "type": "project.open", "path": repo })).await;
        while next(&mut socket).await["type"] != "project.opened" {}
        let cli = Cli { data_dir: root.join("data") };
        let port = server.port;
        let lp = Self { _server: server, _socket: socket, gates, root, cli, repo, port };
        lp.cli.ok(&lp.repo, &["epic", "add", "work", "--title", "Work", "--brief", "-"], "# Work\n\nThe Brief.\n");
        lp.cli.ok(&lp.repo, &["ticket", "add", "work", "1", "--title", "Shout", "--body", "-"], "Write work.txt.\n");
        lp.cli.ok(&lp.repo, &["epic", "set-setup", "work", ""], "");
        lp
    }

    /// Records an event as a Device does, giving the server's refusal.
    async fn record(&self, event: Value) -> Result<(), String> {
        let response = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{}/api/epics/work", self.port))
            .header("content-type", "application/json")
            .body(event.to_string())
            .send()
            .await
            .unwrap();
        if response.status().is_success() { Ok(()) } else { Err(response.text().await.unwrap()) }
    }

    /// Starts a Run with these Reviewers, then completes its one Ticket by hand so that the Review begins.
    async fn review(&self, parallel: u32, reviewers: &[&str]) {
        let run = json!({ "kind": "run.started", "rig": "Done", "parallel": parallel, "reviewers": reviewers, "remediator": "Remediator" });
        self.record(run).await.unwrap();
        self.until("the Ticket's Claim", |lp| !lp.entries("ticket.claimed").is_empty()).await;
        let worktree = self.worktree("1");
        fs::write(worktree.join("work.txt"), "HELLO\n").unwrap();
        self.cli.ok(&worktree, &["complete", "work/1", "-m", "Shout"], "");
    }

    fn worktree(&self, name: &str) -> PathBuf {
        self.root.join("data/worktrees/work").join(name)
    }

    fn log(&self) -> Vec<Value> {
        let log = fs::read_to_string(self.root.join("data/epics/work.jsonl")).unwrap();
        log.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    fn entries(&self, kind: &str) -> Vec<Value> {
        self.log().into_iter().filter(|entry| entry["kind"] == kind).collect()
    }

    fn kinds(&self) -> Vec<String> {
        let review = ["review.", "reviewer.", "finding.", "remediation.", "run.finished"];
        let kinds = self.log().into_iter().map(|entry| entry["kind"].as_str().unwrap().to_owned());
        kinds.filter(|kind| review.iter().any(|prefix| kind.starts_with(prefix))).collect()
    }

    fn chat_exists(&self, chat: &Value) -> bool {
        self.root.join(format!("data/chats/{}.json", chat.as_str().unwrap())).exists()
    }

    fn gate(&self) -> PathBuf {
        self.gates.path().join("review")
    }

    async fn until(&self, what: &str, done: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !done(self) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}; the log is {:#?}", self.log());
            sleep(Duration::from_millis(50)).await;
        }
    }
}

fn output(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[tokio::test]
async fn reviewers_take_the_runs_slots_in_turn_then_one_remediator_judges_and_fixes_before_it_finishes() {
    let lp = Loop::new("review").await;
    lp.review(1, &["Reviewer A", "Reviewer B"]).await;
    lp.until("Reviewer 1 at its gate", |lp| lp.gate().exists()).await;
    sleep(SETTLE).await;
    let first = lp.entries("reviewer.started");
    assert_eq!(first.len(), 1, "a Run with one slot starts one Reviewer at a time");
    assert_eq!((&first[0]["reviewer"], &first[0]["rig"]), (&json!(1), &json!("Reviewer A")));
    let review = lp.worktree("review-1");
    assert_eq!(first[0]["worktree"], json!(review));
    assert_eq!(output(&review, &["rev-parse", "HEAD"]), output(&lp.repo, &["rev-parse", "stampede/work/epic"]));
    assert_eq!(output(&review, &["symbolic-ref", "-q", "HEAD"]), "", "a Reviewer's worktree is detached");
    let finding = &lp.entries("finding.added")[0];
    assert_eq!((&finding["finding"], &finding["rig"], &finding["body"]), (&json!(1), &json!("Reviewer A"), &json!("work.txt is all capitals.\n")));

    let refused = lp.cli.refused(&lp.repo, &["finding", "add", "work", "--title", "Elsewhere", "--body", "-"], "Not a Reviewer.\n");
    assert!(refused.contains("only a Reviewer of work, from inside its worktree"), "{refused}");
    let integration = lp.worktree("integration");
    let refused = lp.cli.refused(&integration, &["remediated", "work", "-m", "Early"], "");
    assert!(refused.contains("work is not remediating"), "{refused}");
    let refused = lp.cli.refused(&lp.repo, &["finding", "accept", "work", "1", "Elsewhere"], "");
    assert!(refused.contains("only from inside work's Integration Worktree"), "{refused}");

    fs::remove_file(lp.gate()).unwrap();
    lp.until("Reviewer 2 at its gate", |lp| lp.entries("reviewer.started").len() == 2 && lp.gate().exists()).await;
    assert!(!review.exists(), "a Reviewer's worktree is discarded once it reports");
    assert!(!lp.chat_exists(&first[0]["chat"]), "a Reviewer's Chat ends once it reports");
    fs::remove_file(lp.gate()).unwrap();

    lp.until("the Run to finish", |lp| !lp.entries("run.finished").is_empty()).await;
    assert_eq!(
        lp.kinds(),
        [
            "review.started", "reviewer.started", "finding.added", "reviewer.reported", "reviewer.started", "finding.added",
            "reviewer.reported", "remediation.started", "finding.judged", "finding.judged", "review.remediated", "run.finished",
        ]
    );
    let judged: Vec<_> = lp.entries("finding.judged").iter().map(|entry| (entry["finding"].clone(), entry["accepted"].clone())).collect();
    assert_eq!(judged, [(json!(1), json!(true)), (json!(2), json!(false))]);
    assert_eq!(output(&integration, &["log", "-1", "--format=%s"]), "Quiet the greeting");
    assert_eq!(lp.entries("review.remediated")[0]["commit"], json!(output(&integration, &["rev-parse", "HEAD"])));
    assert_eq!(fs::read_to_string(integration.join("work.txt")).unwrap(), "Remediated.\n");
    assert_eq!(output(&integration, &["status", "--porcelain"]), "");
    let remediator = lp.entries("remediation.started")[0]["chat"].clone();
    lp.until("the Remediator's Chat to end", |lp| !lp.chat_exists(&remediator)).await;
    assert!(!lp.worktree("review-2").exists());
    let findings = lp.cli.ok(&lp.repo, &["finding", "list", "work"], "");
    assert!(findings.starts_with("# Finding 1: The greeting shouts\n\nFound by: Reviewer A\nAccepted: It does shout\n"), "{findings}");
}

#[tokio::test]
async fn a_review_without_findings_finishes_the_run_without_a_remediator() {
    let lp = Loop::new("clean").await;
    let unremediated = json!({ "kind": "run.started", "rig": "Done", "parallel": 2, "reviewers": ["Clean Reviewer"] });
    assert!(lp.record(unremediated).await.unwrap_err().contains("needs a Remediator"));
    let unknown = json!({ "kind": "run.started", "rig": "Done", "parallel": 2, "reviewers": ["Nobody"], "remediator": "Remediator" });
    assert_eq!(lp.record(unknown).await.unwrap_err(), "there is no Rig Nobody in the config");
    lp.review(2, &["Clean Reviewer", "Clean Reviewer"]).await;
    lp.until("the Run to finish", |lp| !lp.entries("run.finished").is_empty()).await;
    assert_eq!(lp.entries("reviewer.reported").len(), 2);
    assert!(lp.entries("remediation.started").is_empty());
    assert!(!lp.worktree("review-1").exists() && !lp.worktree("review-2").exists());
}

#[tokio::test]
async fn stopping_a_run_mid_review_ends_its_reviewers_and_discards_their_worktrees() {
    let lp = Loop::new("stop").await;
    lp.review(1, &["Reviewer A"]).await;
    lp.until("Reviewer 1 at its gate", |lp| lp.gate().exists()).await;
    let settled = json!({ "kind": "run.changed", "reviewers": [] });
    assert_eq!(lp.record(settled).await.unwrap_err(), "work's Review has begun, so its Reviewers are settled");
    let chat = lp.entries("reviewer.started")[0]["chat"].clone();
    lp.record(json!({ "kind": "run.stopped" })).await.unwrap();
    lp.until("the Reviewer's Chat to end", |lp| !lp.chat_exists(&chat)).await;
    lp.until("its worktree to go", |lp| !lp.worktree("review-1").exists()).await;
    assert!(lp.entries("reviewer.failed").is_empty(), "a stopped Review gives up on nobody");
    let refused = lp.cli.refused(&lp.repo, &["reviewed", "work"], "");
    assert!(refused.contains("only a Reviewer of work"), "{refused}");
}
