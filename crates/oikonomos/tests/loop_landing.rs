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
name = "Done"
agent = "claude-code"
env = { OIKONOMOS_FAKE_FIXTURE = "simple-turn" }
"#;

/// A test server with an Epic `work` whose one Ticket has merged `README` = "one" into its Integration Worktree.
struct Landed {
    server: Server,
    cli: Cli,
    root: PathBuf,
    repo: PathBuf,
    base: String,
    socket: Socket,
    project_id: String,
    epic: Value,
}

impl Landed {
    async fn new(prefix: &str) -> Self {
        let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
        let server = Server::with_env(prefix, RIGS, vec![("PATH", path.into())]).await;
        let root = fs::canonicalize(server.root.path()).unwrap();
        let repo = init(&root, "repo");
        for (key, value) in [("user.name", "oikonomos"), ("user.email", "test@oikonomos.invalid"), ("commit.gpgsign", "false")] {
            git(&repo, &["config", key, value]);
        }
        fs::write(repo.join("README"), "zero\n").unwrap();
        git(&repo, &["add", "README"]);
        git(&repo, &["commit", "-q", "-m", "Initial commit"]);
        let base = output(&repo, &["symbolic-ref", "--short", "HEAD"]);
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
        control(&mut socket, json!({ "type": "hello", "deviceId": "loop-landing" })).await;
        control(&mut socket, json!({ "type": "project.open", "path": repo })).await;
        let project_id = received(&mut socket, "project.opened").await["projectId"].as_str().unwrap().to_owned();
        control(&mut socket, json!({ "type": "epics.subscribe", "projectId": project_id })).await;
        received(&mut socket, "epics.changed").await;
        let cli = Cli { data_dir: root.join("data") };
        cli.ok(&repo, &["epic", "add", "work", "--title", "Work", "--brief", "-"], "# Work\n");
        cli.ok(&repo, &["ticket", "add", "work", "1", "--title", "One", "--body", "-"], "Write one.\n");
        cli.ok(&repo, &["epic", "set-setup", "work", ""], "");
        let mut landed = Self { server, cli, root, repo, base, socket, project_id, epic: Value::Null };
        let refusal = landed.land().await.unwrap_err();
        assert_eq!(refusal, "work has no Epic branch yet, so there is nothing to land");

        let worktree = landed.start(1).await;
        fs::write(worktree.join("README"), "one\n").unwrap();
        landed.cli.ok(&worktree, &["complete", "work/1", "-m", "One"], "");
        landed.until(|epic| epic["tickets"][0]["status"] == "merged" && epic["tickets"][0].get("worktree").is_none()).await;
        landed
    }

    async fn start(&mut self, ticket: u32) -> PathBuf {
        let start = json!({ "type": "ticket.start", "projectId": self.project_id, "epic": "work", "ticket": ticket, "rig": "Done" });
        control(&mut self.socket, start).await;
        let reply = self.follow(|message| matches!(message["type"].as_str(), Some("ticket.started" | "ticket.error"))).await;
        assert_eq!(reply["type"], "ticket.started", "{reply}");
        self.worktree(&ticket.to_string())
    }

    fn worktree(&self, name: &str) -> PathBuf {
        self.root.join("data/worktrees/work").join(name)
    }

    async fn land(&self) -> Result<String, String> {
        let url = format!("http://127.0.0.1:{}/api/epics/work/land", self.server.port);
        let response = reqwest::Client::new().post(url).send().await.unwrap();
        let (ok, text) = (response.status().is_success(), response.text().await.unwrap());
        if ok { Ok(text) } else { Err(text) }
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

    async fn until(&mut self, until: impl Fn(&Value) -> bool) {
        if !until(&self.epic) {
            self.follow(|message| message["type"] == "epic.changed" && until(&message["epic"])).await;
        }
    }

    fn kinds(&self) -> Vec<String> {
        let log = fs::read_to_string(self.root.join("data/epics/work.jsonl")).unwrap();
        log.lines().map(|line| serde_json::from_str::<Value>(line).unwrap()["kind"].as_str().unwrap().to_owned()).collect()
    }

    /// Everything a refused Landing must leave as it was: the checkout's branch, HEAD, tree, and every branch and worktree.
    fn snapshot(&self) -> [String; 5] {
        [
            output(&self.repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
            output(&self.repo, &["rev-parse", "HEAD"]),
            output(&self.repo, &["status", "--porcelain"]),
            output(&self.repo, &["for-each-ref", "--format=%(refname) %(objectname)", "refs/heads"]),
            output(&self.repo, &["worktree", "list", "--porcelain"]),
        ]
    }

    /// Asserts that Landing refuses with `reason`, leaving everything as it was and the refusal as the Epic's latest Note.
    async fn refused(&mut self, reason: &str) {
        let before = self.snapshot();
        let refusal = self.land().await.unwrap_err();
        assert!(refusal.contains(reason), "{refusal}");
        assert_eq!(self.snapshot(), before, "a refusal changes nothing");
        assert!(!self.repo.join(".git/MERGE_HEAD").exists());
        assert!(self.worktree("integration").exists());
        assert_eq!(self.kinds().last().unwrap(), "landing.refused");
        let note = format!("Landing was refused: {refusal}.");
        self.until(|epic| epic["notes"].as_array().and_then(|notes| notes.last()).is_some_and(|last| last["text"] == note)).await;
        assert_eq!(self.epic["notes"].as_array().unwrap().last().unwrap()["author"], "oikonomos");
        assert!(self.epic.get("landed").is_none());
    }
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

fn has_branch(repo: &Path, branch: &str) -> bool {
    !output(repo, &["branch", "--list", branch]).is_empty()
}

#[tokio::test]
async fn a_clean_landing_merges_the_epic_branch_into_its_base_and_removes_the_integration_worktree_and_branch() {
    let mut lp = Landed::new("loop-landing").await;
    let integration = lp.worktree("integration");
    let tip = output(&integration, &["rev-parse", "HEAD"]);
    fs::write(lp.repo.join("NOTES"), "David's own work.\n").unwrap();
    git(&lp.repo, &["add", "NOTES"]);
    git(&lp.repo, &["commit", "-q", "-m", "Meanwhile"]);
    let moved = output(&lp.repo, &["rev-parse", "HEAD"]);

    let landed = lp.land().await.unwrap();
    let head = output(&lp.repo, &["rev-parse", "HEAD"]);
    assert_eq!(landed, format!("Landed work into {} at {}.", lp.base, &head[..12]));
    assert_eq!(output(&lp.repo, &["log", "--format=%s", "-1"]), "Land work: Work");
    assert_eq!(output(&lp.repo, &["rev-parse", "HEAD^1", "HEAD^2"]), format!("{moved}\n{tip}"), "a merge commit when the base moved");
    assert_eq!(output(&lp.repo, &["symbolic-ref", "--short", "HEAD"]), lp.base);
    assert_eq!(output(&lp.repo, &["status", "--porcelain"]), "");
    assert_eq!(fs::read_to_string(lp.repo.join("README")).unwrap(), "one\n");
    assert_eq!(fs::read_to_string(lp.repo.join("NOTES")).unwrap(), "David's own work.\n");
    assert!(!integration.exists());
    assert!(!has_branch(&lp.repo, "oikonomos/work/epic"));
    assert_eq!(output(&lp.repo, &["worktree", "list", "--porcelain"]).matches("worktree ").count(), 1);
    assert_eq!(lp.kinds().iter().rev().take(2).collect::<Vec<_>>(), ["integration.removed", "epic.landed"]);
    lp.until(|epic| epic.get("integration").is_none()).await;
    assert_eq!(lp.epic["landed"], json!({ "base": lp.base, "head": head }));
    assert_eq!(lp.epic.get("kept"), None);
    assert_eq!(lp.epic["notes"].as_array().unwrap().len(), 1, "only the refusal before the Epic branch was cut");
    assert_eq!(lp.land().await.unwrap_err(), "work has no Epic branch yet, so there is nothing to land");
}

#[tokio::test]
async fn landing_into_a_dirty_checkout_is_refused_and_changes_nothing() {
    let mut lp = Landed::new("loop-landing-dirty").await;
    fs::write(lp.repo.join("README"), "David is editing this.\n").unwrap();
    fs::write(lp.repo.join("scratch.txt"), "Not yet added.\n").unwrap();
    let repo = lp.repo.display().to_string();
    lp.refused(&format!("{repo} has uncommitted changes in README, scratch.txt; commit them or put them aside and land again")).await;
    assert_eq!(fs::read_to_string(lp.repo.join("README")).unwrap(), "David is editing this.\n");

    fs::remove_file(lp.repo.join("scratch.txt")).unwrap();
    git(&lp.repo, &["add", "README"]);
    lp.refused(&format!("{repo} has uncommitted changes in README;")).await;
}

#[tokio::test]
async fn landing_from_a_checkout_on_another_branch_is_refused_and_changes_nothing() {
    let mut lp = Landed::new("loop-landing-branch").await;
    git(&lp.repo, &["checkout", "-q", "-b", "topic"]);
    let (repo, base) = (lp.repo.display().to_string(), lp.base.clone());
    lp.refused(&format!(
        "{repo} is on topic rather than {base}, and Oikonomos never switches branches in your checkout; switch to {base} and land again"
    ))
    .await;
    git(&lp.repo, &["checkout", "-q", "--detach"]);
    lp.refused(&format!("{repo} is not on a branch; check out {base} and land again")).await;
    assert_eq!(lp.kinds().iter().filter(|kind| *kind == "landing.refused").count(), 3);
    assert_eq!(lp.epic["notes"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn a_landing_that_would_conflict_is_refused_and_changes_nothing() {
    let mut lp = Landed::new("loop-landing-conflict").await;
    fs::write(lp.repo.join("README"), "David's line.\n").unwrap();
    git(&lp.repo, &["commit", "-q", "-am", "Conflicting"]);
    let (repo, base) = (lp.repo.display().to_string(), lp.base.clone());
    lp.refused(&format!(
        "merging oikonomos/work/epic into {base} would conflict in README, so Oikonomos left {repo} untouched; merge {base} into the Integration Worktree, resolve it there, and land again"
    ))
    .await;
    assert_eq!(fs::read_to_string(lp.repo.join("README")).unwrap(), "David's line.\n");
    assert_eq!(output(&lp.worktree("integration"), &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn a_landing_keeps_an_integration_worktree_that_is_dirty_or_still_has_a_ticket_to_merge_and_says_so() {
    let mut lp = Landed::new("loop-landing-kept").await;
    let integration = lp.worktree("integration");
    let tip = output(&integration, &["rev-parse", "HEAD"]);
    lp.cli.ok(&lp.repo, &["ticket", "add", "work", "2", "--title", "Two", "--body", "-"], "Write two.\n");
    let second = lp.start(2).await;
    lp.until(|epic| epic["tickets"][1]["status"] == "in-progress").await;
    let landed = lp.land().await.unwrap();
    assert_eq!(output(&lp.repo, &["rev-parse", "HEAD"]), tip, "a fast-forward when the base has not moved");
    let held = format!(
        "Oikonomos kept the Integration Worktree at {} and the Epic branch oikonomos/work/epic, because work/2 is in progress and merges there.",
        integration.display()
    );
    assert_eq!(landed, format!("Landed work into {} at {}. {held}", lp.base, &tip[..12]));
    assert!(integration.exists() && has_branch(&lp.repo, "oikonomos/work/epic"));
    lp.cli.ok(&second, &["skip", "work/2", "Nothing to do."], "");
    lp.until(|epic| epic["tickets"][1]["status"] == "skipped" && epic["tickets"][1].get("worktree").is_none()).await;

    fs::write(integration.join("left.txt"), "Not committed.\n").unwrap();
    let landed = lp.land().await.unwrap();
    let kept = format!(
        "Oikonomos kept the Integration Worktree at {} and the Epic branch oikonomos/work/epic, because the worktree has uncommitted changes.",
        integration.display()
    );
    assert_eq!(landed, format!("Landed work into {} at {}. {kept}", lp.base, &tip[..12]));
    assert!(integration.join("left.txt").exists());
    assert!(has_branch(&lp.repo, "oikonomos/work/epic"));
    assert_eq!(lp.kinds().last().unwrap(), "integration.kept");
    lp.until(|epic| epic["kept"] == kept).await;
    assert_eq!((&lp.epic["kept"], &lp.epic["integration"]), (&json!(kept), &json!(integration)));
    assert_eq!(lp.epic["landed"]["head"], tip);

    fs::remove_file(integration.join("left.txt")).unwrap();
    lp.land().await.unwrap();
    assert!(!integration.exists());
    assert!(!has_branch(&lp.repo, "oikonomos/work/epic"));
    lp.until(|epic| epic.get("integration").is_none()).await;
    assert_eq!(lp.epic.get("kept"), None);
}
