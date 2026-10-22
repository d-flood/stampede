mod common;

use std::{fs, path::Path, process::Command, time::Duration};

use common::{Server, control, git, init, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn an_adhoc_worktree_branches_from_any_ref_beside_its_project_until_deleted() {
    let server = Server::start("worktrees").await;
    let root = fs::canonicalize(server.root.path()).unwrap();
    let repo = init(&root, "repo");
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Initial commit"]);
    let base = rev_parse(&repo, "HEAD");
    git(&repo, &["update-ref", "refs/remotes/origin/main", &base]);
    git(&repo, &["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Ahead of origin"]);
    let (mut socket, project_id) = open(server.port, &repo).await;
    let http = Http(server.port);

    assert_eq!(http.get(&format!("/api/projects/{project_id}/git")).await["isRoot"], true);
    assert_eq!(http.get(&format!("/api/projects/{project_id}/git/refs")).await, json!({ "local": ["master"], "remote": ["origin/main"] }));
    let (status, created) = http.create(&project_id, "feat/x", "origin/main").await;
    assert_eq!(status, 200, "{created}");
    let id = created["projectId"].as_str().unwrap().to_owned();
    let projects = changed(&mut socket, |projects| projects.len() == 2).await;
    let nested = projects.iter().find(|project| project["id"] == id).unwrap();
    let path = root.join(format!("data/worktrees/_branches/{project_id}/feat-x"));
    assert_eq!((&nested["path"], &nested["worktree"]), (&json!(path), &json!({ "parent": project_id, "branch": "feat/x" })));
    assert_eq!(rev_parse(&path, "HEAD"), base);
    assert!(!Command::new("git").arg("-C").arg(&repo).args(["rev-parse", "-q", "--verify", "feat/x@{upstream}"]).output().unwrap().status.success());
    assert_eq!(http.get(&format!("/api/projects/{id}/git")).await["branch"], "feat/x");

    let refused = [
        ("feat/x", "master", format!("{} already exists", path.display())),
        ("bad..name", "master", "bad..name is not a valid branch name".to_owned()),
        ("other", "nowhere", "nowhere is not a branch or commit".to_owned()),
    ];
    for (branch, from, error) in refused {
        assert_eq!(http.create(&project_id, branch, from).await, (400, json!(error)));
    }
    fs::create_dir(repo.join("sub")).unwrap();
    let (_, sub) = open(server.port, &repo.join("sub")).await;
    assert_eq!(http.get(&format!("/api/projects/{sub}/git")).await["isRoot"], false);
    let not_root = format!("{} is not the root of a git repository", repo.join("sub").display());
    assert_eq!(http.create(&sub, "other", "master").await, (400, json!(not_root)));

    git(&path, &["commit", "-q", "--allow-empty", "-m", "Unmerged"]);
    let unmerged = "It has 1 commit no other branch has. Deleting it and feat/x discards them.";
    assert_eq!(http.delete(&id, false).await, (409, unmerged.to_owned()));
    assert_eq!(http.delete(&id, true).await, (204, String::new()));
    changed(&mut socket, |projects| !projects.iter().any(|project| project["id"] == id)).await;
    assert!(!path.exists());
    assert!(!Command::new("git").arg("-C").arg(&repo).args(["rev-parse", "-q", "--verify", "refs/heads/feat/x"]).output().unwrap().status.success());

    let (_, created) = http.create(&project_id, "clean", "master").await;
    let id = created["projectId"].as_str().unwrap().to_owned();
    assert_eq!(http.delete(&id, false).await, (204, String::new()), "a worktree holding nothing unmerged goes without asking");
}

#[tokio::test]
async fn a_worktree_checks_out_an_existing_branch_and_leaves_it_when_deleted() {
    let server = Server::start("worktrees-existing").await;
    let root = fs::canonicalize(server.root.path()).unwrap();
    let repo = init(&root, "repo");
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Initial commit"]);
    let base = rev_parse(&repo, "HEAD");
    git(&repo, &["branch", "feat/y"]);
    git(&repo, &["remote", "add", "origin", "https://example.com/repo.git"]);
    git(&repo, &["update-ref", "refs/remotes/origin/topic", &base]);
    let (mut socket, project_id) = open(server.port, &repo).await;
    let http = Http(server.port);

    let (status, created) = http.checkout(&project_id, "feat/y").await;
    assert_eq!(status, 200, "{created}");
    let id = created["projectId"].as_str().unwrap().to_owned();
    let projects = changed(&mut socket, |projects| projects.len() == 2).await;
    let nested = projects.iter().find(|project| project["id"] == id).unwrap();
    let path = root.join(format!("data/worktrees/_branches/{project_id}/feat-y"));
    assert_eq!((&nested["path"], &nested["worktree"]), (&json!(path), &json!({ "parent": project_id, "branch": "feat/y", "existing": true })));
    assert_eq!(http.get(&format!("/api/projects/{id}/git")).await["branch"], "feat/y");

    assert_eq!(http.checkout(&project_id, &base).await, (400, json!(format!("{base} is not a branch"))));
    assert_eq!(http.checkout(&project_id, "master").await.0, 400, "git refuses a branch checked out elsewhere");

    git(&path, &["commit", "-q", "--allow-empty", "-m", "Kept"]);
    let kept = rev_parse(&path, "HEAD");
    fs::write(path.join("draft"), "").unwrap();
    let uncommitted = "It has uncommitted changes. Deleting it discards them.";
    assert_eq!(http.delete(&id, false).await, (409, uncommitted.to_owned()));
    fs::remove_file(path.join("draft")).unwrap();
    assert_eq!(http.delete(&id, false).await, (204, String::new()), "commits stay on the branch");
    changed(&mut socket, |projects| !projects.iter().any(|project| project["id"] == id)).await;
    assert!(!path.exists());
    assert_eq!(rev_parse(&repo, "feat/y"), kept);

    let (status, created) = http.checkout(&project_id, "origin/topic").await;
    assert_eq!(status, 200, "{created}");
    let id = created["projectId"].as_str().unwrap().to_owned();
    let projects = changed(&mut socket, |projects| projects.len() == 2).await;
    assert_eq!(projects.iter().find(|project| project["id"] == id).unwrap()["worktree"], json!({ "parent": project_id, "branch": "topic" }));
    assert_eq!(rev_parse(&repo, "topic@{upstream}"), base);
    assert_eq!(http.delete(&id, false).await, (204, String::new()));
    changed(&mut socket, |projects| !projects.iter().any(|project| project["id"] == id)).await;
    assert!(!Command::new("git").arg("-C").arg(&repo).args(["rev-parse", "-q", "--verify", "refs/heads/topic"]).output().unwrap().status.success());
}

struct Http(u16);

impl Http {
    async fn get(&self, path: &str) -> Value {
        serde_json::from_str(&reqwest::get(format!("http://127.0.0.1:{}{path}", self.0)).await.unwrap().text().await.unwrap()).unwrap()
    }

    async fn create(&self, project_id: &str, branch: &str, from: &str) -> (u16, Value) {
        self.post(project_id, json!({ "branch": branch, "from": from })).await
    }

    async fn checkout(&self, project_id: &str, from: &str) -> (u16, Value) {
        self.post(project_id, json!({ "from": from })).await
    }

    async fn post(&self, project_id: &str, body: Value) -> (u16, Value) {
        let url = format!("http://127.0.0.1:{}/api/projects/{project_id}/worktrees", self.0);
        let body = body.to_string();
        let response = reqwest::Client::new().post(url).header("content-type", "application/json").body(body).send().await.unwrap();
        let status = response.status().as_u16();
        let text = response.text().await.unwrap();
        (status, serde_json::from_str(&text).unwrap_or(json!(text)))
    }

    async fn delete(&self, project_id: &str, force: bool) -> (u16, String) {
        let url = format!("http://127.0.0.1:{}/api/projects/{project_id}/worktree?force={force}", self.0);
        let response = reqwest::Client::new().delete(url).send().await.unwrap();
        (response.status().as_u16(), response.text().await.unwrap())
    }
}

async fn open(port: u16, dir: &Path) -> (Socket, String) {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": dir })).await;
    let id = next_control(&mut socket, "project.opened").await["projectId"].as_str().unwrap().to_owned();
    (socket, id)
}

async fn changed(socket: &mut Socket, until: impl Fn(&[Value]) -> bool) -> Vec<Value> {
    timeout(Duration::from_secs(10), async {
        loop {
            let message = next_control(socket, "projects.changed").await;
            let projects = message["projects"].as_array().unwrap();
            if until(projects) {
                return projects.clone();
            }
        }
    })
    .await
    .expect("the Projects never changed so")
}

fn rev_parse(dir: &Path, rev: &str) -> String {
    let output = Command::new("git").arg("-C").arg(dir).args(["rev-parse", rev]).output().unwrap();
    assert!(output.status.success(), "git rev-parse {rev}");
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}
