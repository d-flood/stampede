use crate::common;

use std::{env, fs, path::Path, path::PathBuf, process::Command, time::Duration};

use common::{Server, control, git, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const URL: &str = "https://github.com/acme/widgets.git";

async fn server(prefix: &str) -> Server {
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    Server::with_env(prefix, "", vec![("PATH", path.into())]).await
}

fn fixture(root: &Path, state: &str) {
    let fixture = json!({
        "user": "david",
        "repo": { "nameWithOwner": "acme/widgets", "url": "https://github.com/acme/widgets" },
        "prs": [{
            "number": 7, "title": "Add widgets", "author": { "login": "octo" }, "isDraft": false,
            "updatedAt": "2026-11-01T00:00:00Z", "state": state, "baseRefName": "main",
            "url": "https://github.com/acme/widgets/pull/7", "requested": true
        }]
    });
    fs::write(root.join("gh.json"), fixture.to_string()).unwrap();
}

/// A repository whose origin, reached through `insteadOf`, carries PR 7 only as `refs/pull/7/head`.
fn repository(root: &Path) -> (PathBuf, String) {
    let origin = root.join("origin.git");
    git(root, &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
    let repo = root.join("widgets");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::write(repo.join("README.md"), "widgets\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "Initial commit"]);
    git(&repo, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&repo, &["push", "-q", "origin", "main"]);
    let pr = push_pr(&repo, "main", "widget.txt", "one\n");
    git(&repo, &["remote", "set-url", "origin", URL]);
    git(&repo, &["config", &format!("url.{}.insteadOf", origin.display()), URL]);
    (fs::canonicalize(&repo).unwrap(), pr)
}

/// Commits `file` on `base` and force-pushes it as PR 7's head.
fn push_pr(repo: &Path, base: &str, file: &str, text: &str) -> String {
    git(repo, &["switch", "-q", "--detach", base]);
    fs::write(repo.join(file), text).unwrap();
    git(repo, &["add", "."]);
    git(repo, &["commit", "-qm", "Add widgets"]);
    let sha = rev_parse(repo, "HEAD");
    git(repo, &["push", "-qf", "origin", &format!("{sha}:refs/pull/7/head")]);
    git(repo, &["switch", "-q", "main"]);
    sha
}

fn rev_parse(repo: &Path, rev: &str) -> String {
    let output = Command::new("git").arg("-C").arg(repo).args(["rev-parse", rev]).output().unwrap();
    assert!(output.status.success(), "git rev-parse {rev}");
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}

fn hidden_refs(repo: &Path) -> String {
    let output = Command::new("git").arg("-C").arg(repo).args(["for-each-ref", "--format=%(refname)", "refs/stampede/"]).output().unwrap();
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}

async fn device(port: u16, name: &str) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": name })).await;
    next_control(&mut socket, "hello").await;
    socket
}

async fn open(socket: &mut Socket, dir: &Path) -> String {
    control(socket, json!({ "type": "project.open", "path": dir })).await;
    next_control(socket, "project.opened").await["projectId"].as_str().unwrap().to_owned()
}

async fn changed(socket: &mut Socket, project_id: &str) -> Value {
    let heard = timeout(Duration::from_secs(10), async {
        loop {
            let message = next_control(socket, "pr-reviews.changed").await;
            if message["projectId"] == project_id {
                return message["reviews"].clone();
            }
        }
    });
    heard.await.expect("pr-reviews.changed never came")
}

async fn request(method: reqwest::Method, port: u16, path: &str, body: Option<Value>) -> (u16, Value) {
    let mut request = reqwest::Client::new().request(method, format!("http://127.0.0.1:{port}{path}"));
    if let Some(body) = body {
        request = request.header("Content-Type", "application/json").body(body.to_string());
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

async fn get(port: u16, path: &str) -> (u16, Value) {
    request(reqwest::Method::GET, port, path, None).await
}

async fn post(port: u16, path: &str, body: Value) -> (u16, Value) {
    request(reqwest::Method::POST, port, path, Some(body)).await
}

#[tokio::test]
async fn opening_a_pr_fetches_hidden_refs_that_every_worktree_shares() {
    let server = server("pr-reviews").await;
    let root = server.root.path();
    fixture(root, "OPEN");
    let (repo, pr) = repository(root);
    let mut first = device(server.port, "first").await;
    let id = open(&mut first, &repo).await;

    let (code, listed) = get(server.port, &format!("/api/projects/{id}/prs")).await;
    assert_eq!(code, 200, "{listed}");
    let expected = json!([{ "number": 7, "title": "Add widgets", "author": "octo", "isDraft": false, "requested": true, "updatedAt": "2026-11-01T00:00:00Z" }]);
    assert_eq!((&listed["available"], &listed["prs"]), (&json!(true), &expected));

    let (code, review) = post(server.port, &format!("/api/projects/{id}/pr-reviews"), json!({ "pr": 7 })).await;
    assert_eq!(code, 200, "{review}");
    let main = rev_parse(&repo, "main");
    assert_eq!(
        review,
        json!({
            "number": 7, "title": "Add widgets", "url": "https://github.com/acme/widgets/pull/7", "author": "octo",
            "mine": false, "state": "open", "target": { "branch": "main", "commit": main }, "source": { "commit": pr },
            "mergeBase": main, "viewed": {}, "drafts": []
        })
    );
    assert_eq!(rev_parse(&repo, "refs/stampede/pr/7/source"), pr);
    assert_eq!(rev_parse(&repo, "refs/stampede/pr/7/target"), main);
    let (_, refs) = get(server.port, &format!("/api/projects/{id}/git/refs")).await;
    assert_eq!((&refs["local"], &refs["remote"]), (&json!(["main"]), &json!(["origin/main"])));
    let compare = format!("/api/projects/{id}/git/compare?target=refs/stampede/pr/7/target&source=refs/stampede/pr/7/source");
    let blob = rev_parse(&repo, &format!("{pr}:widget.txt"));
    assert_eq!(get(server.port, &compare).await.1["files"], json!([{ "path": "widget.txt", "kind": "added", "blob": blob }]));

    let reviews = format!("/api/projects/{id}/pr-reviews");
    let (code, again) = post(server.port, &reviews, json!({ "pr": "https://github.com/acme/widgets/pull/7/files" })).await;
    assert_eq!((code, &again), (200, &review));
    let (code, refused) = post(server.port, &reviews, json!({ "pr": "https://github.com/acme/gadgets/pull/7" })).await;
    assert_eq!(code, 400, "{refused}");
    let (code, unknown) = post(server.port, &reviews, json!({ "pr": 99 })).await;
    assert_eq!(code, 404);
    assert!(unknown.as_str().unwrap().contains("Could not resolve to a PullRequest"), "{unknown}");

    let worktree = root.join("worktree");
    git(&repo, &["worktree", "add", "-q", "--detach", worktree.to_str().unwrap()]);
    let mut second = device(server.port, "second").await;
    let other = open(&mut second, &fs::canonicalize(&worktree).unwrap()).await;
    control(&mut second, json!({ "type": "pr-reviews.subscribe", "projectId": other })).await;
    assert_eq!(changed(&mut second, &other).await, json!([review]));
    assert_eq!(get(server.port, &format!("/api/projects/{other}/pr-reviews")).await, (200, json!([review])));

    let (code, refreshed) = post(server.port, &format!("/api/projects/{id}/pr-reviews/7/refresh"), Value::Null).await;
    assert_eq!((code, &refreshed), (200, &review));
    assert_eq!(changed(&mut second, &other).await, json!([review]));
}

#[tokio::test]
async fn without_gh_signed_in_the_pr_button_says_why() {
    let server = server("pr-reviews-signed-out").await;
    let root = server.root.path();
    let (repo, ..) = repository(root);
    let mut socket = device(server.port, "device").await;
    let id = open(&mut socket, &repo).await;
    let (code, listed) = get(server.port, &format!("/api/projects/{id}/prs")).await;
    assert_eq!(code, 200, "{listed}");
    assert_eq!((&listed["available"], &listed["hidden"], &listed["prs"]), (&json!(false), &json!(false), &json!([])));
    assert!(listed["reason"].as_str().unwrap().contains("not signed in"), "{listed}");

    fixture(root, "OPEN");
    git(&repo, &["remote", "set-url", "origin", "https://github.com/acme/gadgets.git"]);
    let listed = get(server.port, &format!("/api/projects/{id}/prs")).await.1;
    assert_eq!((&listed["available"], &listed["hidden"]), (&json!(false), &json!(true)));
}

#[tokio::test]
async fn refresh_notices_a_merged_pr_and_discarding_removes_its_record_and_refs() {
    let server = server("pr-reviews-merged").await;
    let root = server.root.path();
    fixture(root, "OPEN");
    let (repo, _) = repository(root);
    let mut socket = device(server.port, "device").await;
    let id = open(&mut socket, &repo).await;
    let reviews = format!("/api/projects/{id}/pr-reviews");
    assert_eq!(post(server.port, &reviews, json!({ "pr": 7 })).await.0, 200);

    let pushed = push_pr(&repo, "main", "gadget.txt", "two\n");
    fixture(root, "MERGED");
    let (code, refreshed) = post(server.port, &format!("{reviews}/7/refresh"), Value::Null).await;
    assert_eq!(code, 200, "{refreshed}");
    assert_eq!((&refreshed["state"], &refreshed["source"]["commit"]), (&json!("merged"), &json!(pushed)));
    assert_eq!(rev_parse(&repo, "refs/stampede/pr/7/source"), pushed);

    let (code, _) = request(reqwest::Method::DELETE, server.port, &format!("{reviews}/7"), None).await;
    assert_eq!(code, 204);
    assert_eq!(get(server.port, &reviews).await, (200, json!([])));
    assert_eq!(hidden_refs(&repo), "");
    assert_eq!(fs::read_dir(root.join("data/pr-reviews")).unwrap().flatten().flat_map(|dir| fs::read_dir(dir.path()).unwrap()).count(), 0);
    assert_eq!(request(reqwest::Method::DELETE, server.port, &format!("{reviews}/7"), None).await.0, 404);
}

async fn put(port: u16, path: &str, body: Value) -> (u16, Value) {
    request(reqwest::Method::PUT, port, path, Some(body)).await
}

/// Each compared file's Source blob, or null when the Source deletes it.
async fn blobs(port: u16, id: &str) -> Value {
    let compare = format!("/api/projects/{id}/git/compare?target=refs/stampede/pr/7/target&source=refs/stampede/pr/7/source");
    let files = get(port, &compare).await.1["files"].as_array().unwrap().clone();
    files.iter().map(|file| (file["path"].as_str().unwrap().to_owned(), file.get("blob").cloned().unwrap_or(Value::Null))).collect::<serde_json::Map<_, _>>().into()
}

#[tokio::test]
async fn viewed_files_stay_viewed_until_the_source_changes_them_and_survive_force_pushes_and_gc() {
    let server = server("pr-reviews-viewed").await;
    let root = server.root.path();
    fixture(root, "OPEN");
    let (repo, pr) = repository(root);
    let first = push_pr(&repo, &pr, "gadget.txt", "gadget\n");
    let mut socket = device(server.port, "device").await;
    let id = open(&mut socket, &repo).await;
    control(&mut socket, json!({ "type": "pr-reviews.subscribe", "projectId": id })).await;
    changed(&mut socket, &id).await;
    let reviews = format!("/api/projects/{id}/pr-reviews");
    assert_eq!(post(server.port, &reviews, json!({ "pr": 7 })).await.0, 200);
    changed(&mut socket, &id).await;

    let viewed = format!("{reviews}/7/viewed");
    let blob = |rev: &str, path: &str| rev_parse(&repo, &format!("{rev}:{path}"));
    let mut marks = json!({});
    for path in ["widget.txt", "gadget.txt"] {
        let (code, review) = put(server.port, &viewed, json!({ "path": path, "viewed": true })).await;
        assert_eq!(code, 200, "{review}");
        marks[path] = json!({ "commit": first, "blob": blob(&first, path) });
        assert_eq!(changed(&mut socket, &id).await[0]["viewed"], marks, "every Device hears each mark");
    }
    assert_eq!(rev_parse(&repo, &format!("refs/stampede/pr/7/viewed/{first}")), first);
    assert_eq!(put(server.port, &viewed, json!({ "path": "nowhere.txt", "viewed": true })).await.0, 404);
    assert_eq!(put(server.port, &viewed, json!({ "path": "../widget.txt", "viewed": true })).await.0, 403);

    let second = push_pr(&repo, &first, "gadget.txt", "gadget, changed\n");
    let (code, refreshed) = post(server.port, &format!("{reviews}/7/refresh"), Value::Null).await;
    assert_eq!(code, 200, "{refreshed}");
    assert_eq!(refreshed["viewed"], marks);
    let files = blobs(server.port, &id).await;
    assert_eq!(files["widget.txt"], marks["widget.txt"]["blob"], "a push that leaves widget.txt alone keeps it Viewed");
    assert_ne!(files["gadget.txt"], marks["gadget.txt"]["blob"], "a push that changes gadget.txt unviews it");

    push_pr(&repo, "main", "widget.txt", "rewritten\n");
    assert_eq!(post(server.port, &format!("{reviews}/7/refresh"), Value::Null).await.0, 200);
    let files = blobs(server.port, &id).await;
    assert_ne!(files["widget.txt"], marks["widget.txt"]["blob"]);
    assert!(files.get("gadget.txt").is_none());
    git(&repo, &["reflog", "expire", "--expire=now", "--all"]);
    git(&repo, &["gc", "-q", "--prune=now"]);
    let gone = Command::new("git").arg("-C").arg(&repo).args(["cat-file", "-e", &format!("{second}^{{commit}}")]).status().unwrap();
    assert!(!gone.success(), "gc kept a commit nothing names");
    let show = |path: &str| format!("/api/projects/{id}/git/show?sha={first}&path={path}");
    assert_eq!(get(server.port, &show("widget.txt")).await, (200, json!("one\n")));
    assert_eq!(get(server.port, &show("gadget.txt")).await, (200, json!("gadget\n")));

    let (_, review) = put(server.port, &viewed, json!({ "path": "gadget.txt", "viewed": false })).await;
    assert_eq!(review["viewed"], json!({ "widget.txt": marks["widget.txt"] }));
}

#[tokio::test]
async fn open_as_worktree_detaches_at_the_source_without_a_branch_and_deletes_by_hand() {
    let server = server("pr-reviews-worktree").await;
    let root = server.root.path();
    fixture(root, "OPEN");
    let (repo, pr) = repository(root);
    let mut socket = device(server.port, "device").await;
    let id = open(&mut socket, &repo).await;
    assert_eq!(post(server.port, &format!("/api/projects/{id}/pr-reviews"), json!({ "pr": 7 })).await.0, 200);
    let heads = || {
        let output = Command::new("git").arg("-C").arg(&repo).args(["for-each-ref", "--format=%(refname)", "refs/heads/"]).output().unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    let before = heads();

    let route = format!("/api/projects/{id}/pr-reviews/7/worktree");
    let (code, created) = post(server.port, &route, Value::Null).await;
    assert_eq!(code, 200, "{created}");
    let worktree = created["projectId"].as_str().unwrap().to_owned();
    let projects = listed(&mut socket, |projects| projects.iter().any(|project| project["id"] == worktree)).await;
    let nested = projects.iter().find(|project| project["id"] == worktree).unwrap();
    let path = fs::canonicalize(root).unwrap().join(format!("data/worktrees/_branches/{id}/pr-7"));
    assert_eq!((&nested["path"], &nested["worktree"]), (&json!(path), &json!({ "parent": id, "commit": pr })));
    assert_eq!(rev_parse(&path, "HEAD"), pr);
    assert!(!Command::new("git").arg("-C").arg(&path).args(["symbolic-ref", "-q", "HEAD"]).status().unwrap().success(), "HEAD is detached");
    assert_eq!(heads(), before);

    let (code, again) = post(server.port, &route, Value::Null).await;
    assert_eq!(code, 200, "{again}");
    let taken = again["projectId"].as_str().unwrap().to_owned();
    let projects = listed(&mut socket, |projects| projects.iter().any(|project| project["id"] == taken)).await;
    assert!(projects.iter().any(|project| project["id"] == taken && project["path"].as_str().unwrap().ends_with("/pr-7-2")));

    git(&path, &["commit", "-q", "--allow-empty", "-m", "Local work"]);
    let delete = |project: &str| format!("/api/projects/{project}/worktree?force=false");
    let (code, refused) = request(reqwest::Method::DELETE, server.port, &delete(&worktree), None).await;
    assert_eq!((code, refused), (409, json!("It has 1 commit no other branch has. Deleting it discards them.")));
    git(&path, &["reset", "-q", "--hard", &pr]);
    assert_eq!(request(reqwest::Method::DELETE, server.port, &delete(&worktree), None).await.0, 204);
    listed(&mut socket, |projects| !projects.iter().any(|project| project["id"] == worktree)).await;
    assert!(!path.exists());
    assert_eq!(heads(), before);
}

async fn listed(socket: &mut Socket, until: impl Fn(&[Value]) -> bool) -> Vec<Value> {
    let heard = timeout(Duration::from_secs(10), async {
        loop {
            let message = next_control(socket, "projects.changed").await;
            let projects = message["projects"].as_array().unwrap();
            if until(projects) {
                return projects.clone();
            }
        }
    });
    heard.await.expect("projects.changed never came")
}
