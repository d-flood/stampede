mod common;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use common::{Cli, Server, control, git, init, next};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
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
name = "Claude Approval"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "approval" }

[[rig]]
name = "Codex Approval"
agent = "codex"
env = { OIKONOMOS_FAKE_FIXTURE = "approval" }
"#;

/// A test server with an Epic `work` in a fresh repository, and a Device watching it.
struct Loop {
    server: Server,
    root: PathBuf,
    cli: Cli,
    repo: PathBuf,
    socket: Socket,
    project_id: String,
    epic: Value,
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
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
        control(&mut socket, json!({ "type": "hello", "deviceId": "loop-ticket" })).await;
        control(&mut socket, json!({ "type": "project.open", "path": repo })).await;
        let opened = received(&mut socket, |message| message["type"] == "project.opened").await;
        let project_id = opened["projectId"].as_str().unwrap().to_owned();
        control(&mut socket, json!({ "type": "epics.subscribe", "projectId": project_id })).await;
        received(&mut socket, |message| message["type"] == "epics.changed").await;
        let cli = Cli { data_dir: root.join("data") };
        cli.ok(&repo, &["epic", "add", "work", "--title", "Work", "--brief", "-"], "# Work\n\nThe Brief.\n");
        Self { server, root, cli, repo, socket, project_id, epic: Value::Null }
    }

    fn ok(&self, args: &[&str], stdin: &str) -> String {
        self.cli.ok(&self.repo, args, stdin)
    }

    fn worktree(&self, name: &str) -> PathBuf {
        self.root.join("data/worktrees/work").join(name)
    }

    async fn start(&mut self, ticket: u32, rig: &str) -> Result<String, String> {
        let start = json!({ "type": "ticket.start", "projectId": self.project_id, "epic": "work", "ticket": ticket, "rig": rig });
        control(&mut self.socket, start).await;
        let reply = self.follow(|message| matches!(message["type"].as_str(), Some("ticket.started" | "ticket.error"))).await;
        assert_eq!((&reply["projectId"], &reply["epic"], &reply["ticket"]), (&json!(self.project_id), &json!("work"), &json!(ticket)));
        match reply["type"].as_str() {
            Some("ticket.started") => Ok(reply["chatId"].as_str().unwrap().to_owned()),
            _ => Err(reply["error"].as_str().unwrap().to_owned()),
        }
    }

    /// Reads the socket until a message satisfies `until`, keeping the latest state of the Epic.
    async fn follow(&mut self, until: impl Fn(&Value) -> bool) -> Value {
        timeout(Duration::from_secs(20), async {
            loop {
                let message = next(&mut self.socket).await;
                if message["type"] == "epic.changed" {
                    self.epic = message["epic"].clone();
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
        self.follow(|message| message["type"] == "chat.status" && message["chatId"] == chat_id && message["status"] == status).await;
    }

    async fn record(&self, event: &Value) -> (u16, String) {
        let url = format!("http://127.0.0.1:{}/api/epics/work", self.server.port);
        let request = reqwest::Client::new().post(url).header("Content-Type", "application/json").body(event.to_string());
        let response = request.send().await.unwrap();
        (response.status().as_u16(), response.text().await.unwrap())
    }

    /// Every first prompt and message the fake Claude Chats were sent.
    fn prompts(&self) -> Vec<String> {
        let stdin = fs::read_to_string(self.root.join("home/.claude/fake-received.jsonl")).unwrap();
        let messages = stdin.lines().map(|line| serde_json::from_str::<Value>(line).unwrap());
        messages.filter(|message| message["type"] == "user").map(|message| message["message"]["content"].as_str().unwrap().to_owned()).collect()
    }

    fn log(&self) -> Vec<Value> {
        let log = fs::read_to_string(self.root.join("data/epics/work.jsonl")).unwrap();
        log.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    fn kinds(&self) -> Vec<String> {
        self.log().iter().map(|entry| entry["kind"].as_str().unwrap().to_owned()).collect()
    }
}

async fn received(socket: &mut Socket, until: impl Fn(&Value) -> bool) -> Value {
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next(socket).await;
            if until(&message) {
                return message;
            }
        }
    })
    .await
    .expect("the message never arrived")
}

fn output(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(output.status.success(), "git {args:?} in {}: {}", dir.display(), String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}

fn has_branch(repo: &Path, branch: &str) -> bool {
    !output(repo, &["branch", "--list", branch]).is_empty()
}

#[tokio::test]
async fn a_started_ticket_is_worked_merged_and_cleaned_up_and_unblocks_the_next() {
    let mut lp = Loop::new("loop-ticket").await;
    lp.ok(&["ticket", "add", "work", "1", "--title", "Write work.txt", "--body", "-"], "Write **work.txt**.\n");
    lp.ok(&["ticket", "add", "work", "2", "--title", "Then more", "--blocked-by", "1", "--body", "-"], "More.\n");
    let error = lp.start(1, "Verdict").await.unwrap_err();
    assert!(error.contains("work has no Setup Command yet"), "{error}");
    let setup_log = lp.root.join("setup.log");
    lp.ok(&["epic", "set-setup", "work", &format!("pwd >> {}", setup_log.display())], "");
    let error = lp.start(2, "Verdict").await.unwrap_err();
    assert_eq!(error, "work/2 is not on the Frontier until work/1 are merged or skipped");

    let chat_id = lp.start(1, "Verdict").await.unwrap();
    let merged = lp.ticket_until(1, |ticket| ticket["status"] == "merged" && ticket.get("worktree").is_none()).await;
    assert_eq!(merged["claim"], json!({ "chat": chat_id, "rig": "Verdict" }));
    assert_eq!(merged.get("attention"), None);

    let (integration, worktree) = (lp.worktree("integration"), lp.worktree("1"));
    let base = output(&lp.repo, &["symbolic-ref", "--short", "HEAD"]);
    assert_eq!(lp.epic["base"], base);
    assert_eq!(lp.epic["integration"], json!(integration));
    assert_eq!(fs::read_to_string(&setup_log).unwrap(), format!("{}\n{}\n", integration.display(), worktree.display()));
    assert_eq!(
        lp.kinds(),
        ["epic.added", "ticket.added", "ticket.added", "setup.set", "branch.cut", "ticket.claimed", "ticket.completed", "ticket.merged", "worktree.removed"]
    );
    let log = lp.log();
    assert_eq!(log[4]["base"], base);
    assert_eq!((&log[5]["chat"], &log[5]["rig"], &log[5]["worktree"]), (&json!(chat_id), &json!("Verdict"), &json!(worktree)));

    let commit = log[6]["commit"].as_str().unwrap();
    assert_eq!(output(&integration, &["log", "--format=%s", "-1"]), "Merge work/1: Write work.txt");
    assert_eq!(output(&integration, &["rev-parse", "HEAD^1"]), output(&lp.repo, &["rev-parse", "HEAD"]));
    assert_eq!(output(&integration, &["rev-parse", "HEAD^2"]), commit);
    assert_eq!(output(&integration, &["log", "--format=%s", "-1", commit]), "Fake verdict");
    assert_eq!(output(&integration, &["show", "--name-only", "--format=", commit]), "work.txt");
    assert_eq!(fs::read_to_string(integration.join("work.txt")).unwrap(), "Done by the fake.\n");
    assert_eq!(output(&integration, &["status", "--porcelain"]), "");
    assert_eq!(output(&integration, &["branch", "--show-current"]), "oikonomos/work/epic");
    assert!(!worktree.exists());
    assert!(!has_branch(&lp.repo, "oikonomos/work/1"));
    assert_eq!(output(&lp.repo, &["log", "--format=%s"]), "Initial commit", "the Project's own checkout is untouched");
    assert!(!lp.repo.join("work.txt").exists());

    let stdin = fs::read_to_string(lp.root.join("home/.claude/fake-received.jsonl")).unwrap();
    let prompt = stdin
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|message| message["type"] == "user")
        .unwrap()["message"]["content"]
        .as_str()
        .unwrap()
        .to_owned();
    let template = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/templates/ticket.md")).unwrap();
    let data = lp.root.join("data").display().to_string();
    let note = format!("This Oikonomos server keeps its data in {data}, so pass `--data-dir {data}` to every `oikonomos` command.");
    let ticket = format!(
        "Your Ticket is work/1, on the branch `oikonomos/work/1`. On this Host `oikonomos` is `{}`; run it by that path if it is not on your PATH.\n\n## work/1: Write work.txt\n\nWrite **work.txt**.",
        env!("CARGO_BIN_EXE_oikonomos")
    );
    assert_eq!(prompt, format!("{}\n\n{note}\n\n{ticket}", template.trim_end()));

    let mut device = connect_async(format!("ws://127.0.0.1:{}/ws", lp.server.port)).await.unwrap().0;
    control(&mut device, json!({ "type": "hello", "deviceId": "second" })).await;
    let hello = received(&mut device, |message| message["type"] == "hello").await;
    let chat = hello["chats"].as_array().unwrap().iter().find(|chat| chat["id"] == chat_id).unwrap().clone();
    assert_eq!((&chat["projectId"], &chat["cwd"]), (&json!(lp.project_id), &json!(worktree)));
    control(&mut device, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    let items = received(&mut device, |message| message["type"] == "chat.items").await;
    let first = json!({ "kind": "user", "text": format!("/ticket {ticket}"), "images": 0, "template": "ticket" });
    assert_eq!(items["items"][0], first, "the Template collapses to /ticket and the Ticket");

    assert_eq!(lp.ok(&["frontier", "work"], ""), "work/2 Then more\n");
}

#[tokio::test]
async fn a_verdict_is_taken_only_inside_its_worktree_and_complete_accepts_a_clean_tree() {
    let mut lp = Loop::new("loop-ticket-verdicts").await;
    lp.ok(&["ticket", "add", "work", "1", "--title", "Nothing to do", "--body", "-"], "Already done.\n");
    lp.ok(&["ticket", "add", "work", "2", "--title", "Moot", "--blocked-by", "1", "--body", "-"], "Moot.\n");
    lp.ok(&["epic", "set-setup", "work", ""], "");
    let chat_id = lp.start(1, "Done").await.unwrap();
    lp.chat_status(&chat_id, "done").await;
    let held = lp.ticket_until(1, |ticket| ticket["status"] == "in-progress").await;
    assert_eq!(held["claim"]["chat"], chat_id, "a turn that ends without a Verdict keeps the Claim");
    assert!(lp.ok(&["ticket", "work/1"], "").contains("Status: in progress"));

    let worktree = lp.worktree("1");
    let refusals: [(&Path, &[&str], &str); 4] = [
        (&lp.repo, &["complete", "work/1", "-m", "Nope"], "a Verdict on work/1 is taken only from inside its Ticket Worktree"),
        (&lp.root, &["skip", "work/1", "Nope"], "a Verdict on work/1 is taken only from inside its Ticket Worktree"),
        (&worktree, &["complete", "work/2", "-m", "Nope"], "work/2 is not started, not in progress, so it takes no Verdict"),
        (&worktree, &["skip", "work/1", " "], "the reason is empty"),
    ];
    for (cwd, args, expected) in refusals {
        let error = lp.cli.refused(cwd, args, "");
        assert!(error.contains(expected), "{args:?}: {error}");
    }
    let initial = output(&lp.repo, &["rev-parse", "HEAD"]);
    let completed = lp.cli.ok(&worktree, &["complete", "work/1", "-m", "Nothing to add"], "");
    assert_eq!(completed, "Recorded the Verdict: work/1 is complete, and Oikonomos merges it into the Integration Worktree next.\n");
    lp.ticket_until(1, |ticket| ticket["status"] == "merged" && ticket.get("worktree").is_none()).await;
    assert_eq!(lp.log().iter().find(|entry| entry["kind"] == "ticket.completed").unwrap()["commit"], initial);
    let integration = lp.worktree("integration");
    assert_eq!(output(&integration, &["rev-parse", "HEAD"]), initial, "a clean tree merges nothing");
    let again = lp.cli.refused(&lp.repo, &["complete", "work/1", "-m", "Again"], "");
    assert!(again.contains("work/1 is merged, not in progress, so it takes no Verdict"), "{again}");

    lp.start(2, "Done").await.unwrap();
    let second = lp.worktree("2");
    fs::write(second.join("notes.txt"), "Left behind.\n").unwrap();
    assert_eq!(lp.cli.ok(&second, &["skip", "work/2", "The codebase already has it."], ""), "Recorded the Verdict: work/2 is skipped.\n");
    let kept = lp.ticket_until(2, |ticket| ticket["status"] == "skipped" && ticket.get("attention").is_some()).await;
    assert_eq!(kept["notes"][0]["author"], "agent");
    assert_eq!(kept["notes"][0]["text"], "The codebase already has it.");
    assert!(kept["attention"].as_str().unwrap().contains("has uncommitted changes, so Oikonomos kept it"), "{kept}");
    assert_eq!(kept["worktree"], json!(second));
    assert!(second.join("notes.txt").exists());
    assert!(has_branch(&lp.repo, "oikonomos/work/2"));
}

#[tokio::test]
async fn a_failing_setup_command_leaves_the_ticket_untaken_with_a_note() {
    let mut lp = Loop::new("loop-ticket-setup").await;
    lp.ok(&["ticket", "add", "work", "1", "--title", "Fails to set up", "--body", "-"], "Body.\n");
    lp.ok(&["epic", "set-setup", "work", "echo cutting; exit 4"], "");
    let error = lp.start(1, "Done").await.unwrap_err();
    assert_eq!(error, "The Setup Command failed for work/1; its Note has the end of the output");
    let failed = lp.ticket_until(1, |ticket| ticket.get("attention").is_some()).await;
    let note = failed["notes"][0]["text"].as_str().unwrap();
    assert!(note.contains("failed in the Epic's new Integration Worktree (exit status: 4)") && note.contains("cutting"), "{note}");
    assert!(!lp.worktree("integration").exists());
    assert!(!has_branch(&lp.repo, "oikonomos/work/epic"));
    assert!(!lp.kinds().contains(&"branch.cut".to_owned()));

    let setup = r#"test "$(basename "$PWD")" = integration || { echo first; echo broken >&2; exit 3; }"#;
    lp.ok(&["epic", "set-setup", "work", setup], "");
    let error = lp.start(1, "Done").await.unwrap_err();
    assert_eq!(error, "The Setup Command failed for work/1; its Note has the end of the output");
    let failed = lp.ticket_until(1, |ticket| ticket["notes"].as_array().unwrap().len() == 2).await;
    assert_eq!(failed["status"], "not-started");
    assert_eq!(failed["attention"], "The Setup Command failed; its Note has the end of the output.");
    assert_eq!(failed["notes"][1]["author"], "oikonomos");
    let note = failed["notes"][1]["text"].as_str().unwrap();
    assert!(note.starts_with("The Setup Command `test "), "{note}");
    assert!(note.contains("failed in this Ticket's new worktree (exit status: 3)"), "{note}");
    assert!(note.ends_with("```text\nfirst\nbroken\n```"), "{note}");
    assert!(failed.get("claim").is_none() && failed.get("worktree").is_none());
    assert!(lp.worktree("integration").exists());
    assert!(!lp.worktree("1").exists());
    assert!(!has_branch(&lp.repo, "oikonomos/work/1"));
    assert_eq!(lp.ok(&["frontier", "work"], ""), "work/1 Fails to set up\n");

    lp.ok(&["epic", "set-setup", "work", ""], "");
    lp.start(1, "Done").await.unwrap();
    let held = lp.ticket_until(1, |ticket| ticket["status"] == "in-progress").await;
    assert_eq!(held.get("attention"), None, "a Claim clears the flag");
}

#[tokio::test]
async fn an_approval_or_a_turn_without_a_verdict_never_completes_the_ticket() {
    for rig in ["Claude Approval", "Codex Approval"] {
        let mut lp = Loop::new("loop-ticket-approval").await;
        lp.ok(&["ticket", "add", "work", "1", "--title", "Asks first", "--body", "-"], "Run a command.\n");
        lp.ok(&["epic", "set-setup", "work", ""], "");
        let chat_id = lp.start(1, rig).await.unwrap();
        control(&mut lp.socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
        let (mut request, mut waiting) = (Value::Null, false);
        while request.is_null() || !waiting {
            let message = lp.follow(|message| message["chatId"] == chat_id).await;
            match message["type"].as_str() {
                Some("chat.request") => request = message["request"].clone(),
                Some("chat.status") => waiting = message["status"] == "waiting",
                _ => {}
            }
        }
        assert!(lp.ok(&["ticket", "work/1"], "").contains("Status: in progress"), "{rig}");
        let answer = json!({ "type": "chat.answer", "chatId": chat_id, "requestId": request["id"], "answer": { "allow": true } });
        control(&mut lp.socket, answer).await;
        lp.chat_status(&chat_id, "done").await;
        let held = lp.ticket_until(1, |ticket| ticket["status"] == "in-progress").await;
        assert_eq!(held["claim"]["chat"], chat_id, "{rig}");
        assert!(lp.ok(&["ticket", "work/1"], "").contains("Status: in progress"), "{rig}");
        assert_eq!(lp.kinds().last().unwrap(), "ticket.claimed", "{rig}");
    }
}

#[tokio::test]
async fn a_reopened_ticket_starts_again_in_a_fresh_worktree_and_an_edit_mid_chat_is_only_marked() {
    let mut lp = Loop::new("loop-ticket-reopen").await;
    lp.ok(&["ticket", "add", "work", "1", "--title", "Write work.txt", "--body", "-"], "Write **work.txt**.\n");
    lp.ok(&["ticket", "add", "work", "2", "--title", "Then more", "--blocked-by", "1", "--body", "-"], "More.\n");
    lp.ok(&["epic", "set-setup", "work", ""], "");
    let reopen = json!({ "kind": "ticket.reopened", "ticket": 1 });
    let refused = (400, "work/1 is not started; only an in-progress, merged or skipped Ticket can be moved back to not started".to_owned());
    assert_eq!(lp.record(&reopen).await, refused);

    let first = lp.start(1, "Verdict").await.unwrap();
    lp.ticket_until(1, |ticket| ticket["status"] == "merged" && ticket.get("worktree").is_none()).await;
    assert_eq!(lp.ok(&["frontier", "work"], ""), "work/2 Then more\n");
    assert_eq!(lp.record(&reopen).await, (204, String::new()));
    let reopened = lp.ticket_until(1, |ticket| ticket["status"] == "not-started").await;
    assert!(reopened.get("claim").is_none() && reopened.get("worktree").is_none(), "{reopened}");
    assert_eq!(lp.ok(&["frontier", "work"], ""), "work/1 Write work.txt\n", "its dependent waits for it again");

    let second = lp.start(1, "Done").await.unwrap();
    assert_ne!(second, first);
    lp.chat_status(&second, "done").await;
    let (worktree, integration) = (lp.worktree("1"), lp.worktree("integration"));
    assert_eq!(output(&worktree, &["rev-parse", "HEAD"]), output(&integration, &["rev-parse", "HEAD"]), "cut from the Epic branch");
    assert_eq!(fs::read_to_string(worktree.join("work.txt")).unwrap(), "Done by the fake.\n");

    let edit = json!({ "kind": "ticket.body.set", "ticket": 1, "body": "Write work.txt again.\n" });
    assert_eq!(lp.record(&edit).await, (204, String::new()));
    let edited = lp.ticket_until(1, |ticket| ticket["claim"]["edited"] == true).await;
    assert_eq!((&edited["claim"]["chat"], &edited["status"]), (&json!(second), &json!("in-progress")));
    assert_eq!(lp.prompts().len(), 2, "nothing is sent to the Chat");

    fs::write(worktree.join("notes.txt"), "Left behind.\n").unwrap();
    lp.cli.ok(&worktree, &["skip", "work/1", "Nothing to do."], "");
    let kept = lp.ticket_until(1, |ticket| ticket["status"] == "skipped" && ticket.get("attention").is_some()).await;
    assert_eq!(lp.record(&reopen).await, (204, String::new()));
    let reopened = lp.ticket_until(1, |ticket| ticket["status"] == "not-started").await;
    assert_eq!(reopened.get("attention"), None);
    let note = reopened["notes"].as_array().unwrap().last().unwrap();
    assert_eq!((&note["author"], &note["text"]), (&json!("oikonomos"), &kept["attention"]), "the kept worktree is Noted");

    let third = lp.start(1, "Verdict").await.unwrap();
    let merged = lp.ticket_until(1, |ticket| ticket["status"] == "merged" && ticket.get("worktree").is_none()).await;
    assert_eq!(merged["claim"]["chat"], third);
    let fresh = lp.worktree("1-2");
    let claimed: Vec<_> = lp.log().into_iter().filter(|entry| entry["kind"] == "ticket.claimed").map(|entry| entry["worktree"].clone()).collect();
    assert_eq!(claimed, [json!(worktree), json!(worktree), json!(fresh)], "a kept worktree is never reused");
    assert!(lp.prompts()[2].contains("on the branch `oikonomos/work/1-2`"));
    assert_eq!(fs::read_to_string(worktree.join("notes.txt")).unwrap(), "Left behind.\n");
    assert!(has_branch(&lp.repo, "oikonomos/work/1"));
    assert!(!fresh.exists() && !has_branch(&lp.repo, "oikonomos/work/1-2"), "the fresh worktree is cleaned up by its own name");
    assert_eq!(lp.kinds().iter().filter(|kind| *kind == "ticket.reopened").count(), 2);
}
