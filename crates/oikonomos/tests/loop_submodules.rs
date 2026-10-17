mod common;

use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use common::{Cli, Server, control, next};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
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

/// Lets the server's git clone submodules from paths on disk, as the fixture's are.
const GITCONFIG: &str = "[protocol \"file\"]\n\tallow = always\n[user]\n\tname = oikonomos\n\temail = test@oikonomos.invalid\n[commit]\n\tgpgsign = false\n";

/// A test server with an Epic `work` of two Tickets, in a checkout of `origin.git` whose submodule `sub` is cloned
/// from `sub.git`, both bare repositories that nothing should push to.
struct Fixture {
    server: Server,
    cli: Cli,
    root: PathBuf,
    repo: PathBuf,
    socket: Socket,
    project_id: String,
    epic: Value,
    statuses: HashMap<String, String>,
}

impl Fixture {
    /// `checked_out` says whether David's checkout has the submodule checked out.
    async fn new(prefix: &str, checked_out: bool, setup: &str) -> Self {
        let fixtures = format!("{}/tests/fixtures/chats", env!("CARGO_MANIFEST_DIR"));
        let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
        let server = Server::with_env(prefix, &RIGS.replace("FIXTURES", &fixtures), vec![("PATH", path.into())]).await;
        let root = fs::canonicalize(server.root.path()).unwrap();
        fs::write(root.join("home/.gitconfig"), GITCONFIG).unwrap();
        let (sub, superproject) = (root.join("sub-src"), root.join("super-src"));
        for dir in [&sub, &superproject] {
            fs::create_dir(dir).unwrap();
            git(dir, &["init", "-q"]);
        }
        fs::write(sub.join("lib"), "zero\n").unwrap();
        git(&sub, &["add", "lib"]);
        git(&sub, &["commit", "-q", "-m", "Sub zero"]);
        git(&root, &["clone", "-q", "--bare", "sub-src", "sub.git"]);
        fs::write(superproject.join("README"), "zero\n").unwrap();
        git(&superproject, &["add", "README"]);
        git(&superproject, &["submodule", "add", "-q", root.join("sub.git").to_str().unwrap(), "sub"]);
        git(&superproject, &["commit", "-q", "-m", "Initial commit"]);
        git(&root, &["clone", "-q", "--bare", "super-src", "origin.git"]);
        let recurse = if checked_out { "--recurse-submodules" } else { "--no-recurse-submodules" };
        git(&root, &["clone", "-q", recurse, "origin.git", "repo"]);
        let repo = root.join("repo");

        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
        control(&mut socket, json!({ "type": "hello", "deviceId": "loop-submodules" })).await;
        control(&mut socket, json!({ "type": "project.open", "path": repo })).await;
        let project_id = received(&mut socket, "project.opened").await["projectId"].as_str().unwrap().to_owned();
        control(&mut socket, json!({ "type": "epics.subscribe", "projectId": project_id })).await;
        received(&mut socket, "epics.changed").await;
        let cli = Cli { data_dir: root.join("data") };
        cli.ok(&repo, &["epic", "add", "work", "--title", "Work", "--brief", "-"], "# Work\n");
        cli.ok(&repo, &["ticket", "add", "work", "1", "--title", "One", "--body", "-"], "Change sub.\n");
        cli.ok(&repo, &["ticket", "add", "work", "2", "--title", "Two", "--body", "-"], "Change more.\n");
        cli.ok(&repo, &["epic", "set-setup", "work", setup], "");
        Self { server, cli, root, repo, socket, project_id, epic: Value::Null, statuses: HashMap::new() }
    }

    async fn start(&mut self, ticket: u32, rig: &str) -> PathBuf {
        let start = json!({ "type": "ticket.start", "projectId": self.project_id, "epic": "work", "ticket": ticket, "rig": rig });
        control(&mut self.socket, start).await;
        let reply = self.follow(|message| matches!(message["type"].as_str(), Some("ticket.started" | "ticket.error"))).await;
        assert_eq!(reply["type"], "ticket.started", "{reply}");
        self.worktree(&ticket.to_string())
    }

    fn worktree(&self, name: &str) -> PathBuf {
        self.root.join("data/worktrees/work").join(name)
    }

    /// Commits a change inside a worktree's submodule, as a Ticket's Agent would, and gives the new commit.
    fn commit_inside(&self, worktree: &Path, text: &str) -> String {
        let sub = worktree.join("sub");
        fs::write(sub.join("lib"), text).unwrap();
        git(&sub, &["commit", "-q", "-am", "Change lib"]);
        git(&sub, &["rev-parse", "HEAD"])
    }

    async fn land(&self) -> Result<String, String> {
        let url = format!("http://127.0.0.1:{}/api/epics/work/land", self.server.port);
        let response = reqwest::Client::new().post(url).send().await.unwrap();
        let (ok, text) = (response.status().is_success(), response.text().await.unwrap());
        if ok { Ok(text) } else { Err(text) }
    }

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

    fn kinds(&self) -> Vec<String> {
        let log = fs::read_to_string(self.root.join("data/epics/work.jsonl")).unwrap();
        log.lines().map(|line| serde_json::from_str::<Value>(line).unwrap()["kind"].as_str().unwrap().to_owned()).collect()
    }

    /// Every ref of both bare repositories the fixture was cloned from.
    fn origins(&self) -> [String; 2] {
        ["origin.git", "sub.git"].map(|origin| git(&self.root.join(origin), &["for-each-ref", "--format=%(refname) %(objectname)"]))
    }

    /// What a refused Landing must leave as it was in the checkout, its submodule included.
    fn snapshot(&self) -> [String; 6] {
        [
            git(&self.repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
            git(&self.repo, &["rev-parse", "HEAD"]),
            git(&self.repo, &["status", "--porcelain", "--ignore-submodules=none"]),
            git(&self.repo, &["for-each-ref", "--format=%(refname) %(objectname)", "refs/heads"]),
            git(&self.repo, &["worktree", "list", "--porcelain"]),
            git(&self.repo, &["submodule", "status"]),
        ]
    }
}

/// Git as David runs it in the fixture, cloning submodules from paths on disk.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "protocol.file.allow=always", "-c", "user.name=oikonomos", "-c", "user.email=test@oikonomos.invalid"])
        .args(["-c", "commit.gpgsign=false", "-c", "init.defaultBranch=main", "-C"])
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} in {}: {}", dir.display(), String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}

fn has(dir: &Path, commit: &str) -> bool {
    Command::new("git").arg("-C").arg(dir).args(["cat-file", "-e", &format!("{commit}^{{commit}}")]).output().unwrap().status.success()
}

fn has_branch(repo: &Path, branch: &str) -> bool {
    !git(repo, &["branch", "--list", branch]).is_empty()
}

/// Nothing uncommitted in a worktree or in its submodule.
fn clean(dir: &Path) {
    assert_eq!(git(dir, &["status", "--porcelain", "--ignore-submodules=none"]), "", "{}", dir.display());
    assert_eq!(git(dir, &["submodule", "foreach", "--quiet", "git status --porcelain"]), "", "{}", dir.display());
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

#[tokio::test]
async fn a_tickets_submodule_commit_is_carried_into_the_integration_worktree_and_on_landing_into_the_checkout_never_pushed() {
    let mut fx = Fixture::new("loop-sub", true, "").await;
    let origins = fx.origins();
    let recorded = git(&fx.repo, &["rev-parse", "HEAD:sub"]);
    let one = fx.start(1, "Done").await;
    let integration = fx.worktree("integration");
    assert_eq!(git(&integration.join("sub"), &["rev-parse", "HEAD"]), recorded, "Oikonomos checks out the submodule the checkout has");
    assert_eq!(git(&one.join("sub"), &["rev-parse", "HEAD"]), recorded);

    let carried = fx.commit_inside(&one, "one\n");
    fx.cli.ok(&one, &["complete", "work/1", "-m", "Change sub"], "");
    fx.ticket_until(1, |ticket| ticket["status"] == "merged" && ticket.get("worktree").is_none()).await;
    assert_eq!(git(&integration, &["rev-parse", "HEAD:sub"]), carried);
    assert_eq!(git(&integration.join("sub"), &["rev-parse", "HEAD"]), carried, "the merged submodule commit is checked out");
    clean(&integration);
    assert!(!one.exists(), "a clean worktree with a submodule is removed");
    assert!(!fx.repo.join(".git/worktrees/1").exists());
    assert!(!has_branch(&fx.repo, "oikonomos/work/1"));

    let two = fx.start(2, "Done").await;
    assert_eq!(git(&two.join("sub"), &["rev-parse", "HEAD"]), carried, "a later worktree gets the commit its remote lacks");
    clean(&two);
    fx.cli.ok(&two, &["skip", "work/2", "Nothing more to change."], "");
    fx.ticket_until(2, |ticket| ticket["status"] == "skipped" && ticket.get("worktree").is_none()).await;
    assert!(!two.exists());

    let landed = fx.land().await.unwrap();
    let head = git(&fx.repo, &["rev-parse", "HEAD"]);
    assert_eq!(landed, format!("Landed work into main at {}.", &head[..12]));
    assert_eq!(git(&fx.repo, &["rev-parse", "HEAD:sub"]), carried);
    assert_eq!(git(&fx.repo.join("sub"), &["rev-parse", "HEAD"]), carried, "the checkout's submodule follows the Landing");
    clean(&fx.repo);
    assert!(!integration.exists(), "a clean Integration Worktree with a submodule is removed");
    assert!(!has_branch(&fx.repo, "oikonomos/work/epic"));

    assert_eq!(fx.origins(), origins, "nothing is pushed");
    assert!(!has(&fx.root.join("sub.git"), &carried));
}

#[tokio::test]
async fn a_submodule_commit_that_cannot_be_fetched_fails_the_merge_and_merges_nothing() {
    let mut fx = Fixture::new("loop-sub-merge", false, "").await;
    let origins = fx.origins();
    let one = fx.start(1, "Done").await;
    let integration = fx.worktree("integration");
    assert!(!integration.join("sub/.git").exists() && !one.join("sub/.git").exists(), "a submodule the checkout lacks stays out");
    let tip = git(&integration, &["rev-parse", "HEAD"]);

    git(&one, &["submodule", "update", "--init", "-q"]);
    let carried = fx.commit_inside(&one, "one\n");
    fx.cli.ok(&one, &["complete", "work/1", "-m", "Change sub"], "");
    let ticket = fx.ticket_until(1, |ticket| ticket.get("attention").is_some()).await;
    assert_eq!(ticket["status"], "merging");
    let reason = "Its submodule commit could not be fetched into the Integration Worktree, so Oikonomos did not merge it; its Note names the submodule and commit.";
    assert_eq!(ticket["attention"], reason);
    let note = ticket["notes"].as_array().unwrap().last().unwrap();
    let text = format!(
        "Oikonomos did not merge work/1, because the submodule sub is not checked out in {}, so its commit {carried} could not be fetched there.",
        integration.display()
    );
    assert_eq!((&note["author"], &note["text"]), (&json!("oikonomos"), &json!(text)));
    assert_eq!(fx.kinds().iter().rev().take(2).collect::<Vec<_>>(), ["merge.failed", "note.added"]);

    assert_eq!(git(&integration, &["rev-parse", "HEAD"]), tip, "nothing is merged");
    assert!(!fx.repo.join(".git/worktrees/integration/MERGE_HEAD").exists());
    clean(&integration);
    assert!(one.exists() && has_branch(&fx.repo, "oikonomos/work/1"), "the only copy of the commit is kept");
    assert_eq!(fx.origins(), origins);
}

#[tokio::test]
async fn a_landing_whose_submodule_commit_cannot_be_fetched_is_refused_and_changes_nothing() {
    let mut fx = Fixture::new("loop-sub-land", false, "git submodule update --init").await;
    let origins = fx.origins();
    let one = fx.start(1, "Done").await;
    let integration = fx.worktree("integration");
    assert!(integration.join("sub/.git").exists(), "the Setup Command checked it out");
    let carried = fx.commit_inside(&one, "one\n");
    fx.cli.ok(&one, &["complete", "work/1", "-m", "Change sub"], "");
    fx.ticket_until(1, |ticket| ticket["status"] == "merged" && ticket.get("worktree").is_none()).await;
    assert_eq!(git(&integration.join("sub"), &["rev-parse", "HEAD"]), carried);

    let before = fx.snapshot();
    let refusal = fx.land().await.unwrap_err();
    let repo = fx.repo.display();
    assert_eq!(
        refusal,
        format!("Oikonomos left {repo} untouched, because the submodule sub is not checked out in {repo}, so its commit {carried} could not be fetched there")
    );
    assert_eq!(fx.snapshot(), before, "a refusal changes nothing");
    assert!(integration.exists());
    assert_eq!(fx.kinds().last().unwrap(), "landing.refused");
    let note = format!("Landing was refused: {refusal}.");
    fx.follow(|message| message["type"] == "epic.changed" && message["epic"]["notes"].as_array().is_some_and(|notes| notes.last().unwrap()["text"] == note)).await;

    git(&fx.repo, &["submodule", "update", "--init", "-q"]);
    fx.land().await.unwrap();
    assert_eq!(git(&fx.repo.join("sub"), &["rev-parse", "HEAD"]), carried);
    clean(&fx.repo);
    assert!(!integration.exists());
    assert_eq!(fx.origins(), origins, "nothing is pushed");
}

#[tokio::test]
async fn a_merge_its_resolver_commits_checks_out_the_submodule_commit_it_brought() {
    let mut fx = Fixture::new("loop-sub-resolve", true, "").await;
    let one = fx.start(1, "Done").await;
    let two = fx.start(2, "Resolver").await;
    fs::write(one.join("README"), "one\n").unwrap();
    fs::write(two.join("README"), "two\n").unwrap();
    let carried = fx.commit_inside(&two, "two\n");
    fx.cli.ok(&one, &["complete", "work/1", "-m", "One"], "");
    fx.ticket_until(1, |ticket| ticket["status"] == "merged").await;
    fx.cli.ok(&two, &["complete", "work/2", "-m", "Two"], "");
    let resolver = fx.ticket_until(2, |ticket| ticket.get("resolver").is_some()).await["resolver"].as_str().unwrap().to_owned();
    if fx.statuses.get(&resolver).is_none_or(|status| status != "done") {
        fx.follow(|message| message["type"] == "chat.status" && message["chatId"] == resolver && message["status"] == "done").await;
    }

    control(&mut fx.socket, json!({ "type": "chat.send", "chatId": resolver, "text": "Keep both." })).await;
    fx.ticket_until(2, |ticket| ticket["status"] == "merged" && ticket.get("worktree").is_none()).await;
    let integration = fx.worktree("integration");
    assert_eq!(fs::read_to_string(integration.join("README")).unwrap(), "one\ntwo\n");
    assert_eq!(git(&integration.join("sub"), &["rev-parse", "HEAD"]), carried);
    clean(&integration);
    assert!(!two.exists());
}
