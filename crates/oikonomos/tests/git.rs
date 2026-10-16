mod common;

use std::{fs, path::Path, process::Command};

use common::{Server, control, next_control};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn changes_are_everything_uncommitted_against_head() {
    let server = Server::start("git").await;
    let repo = server.root.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    for name in ["unstaged", "staged", "both", "deleted", "removed", "renamed"] {
        fs::write(repo.join(format!("{name}.txt")), format!("{name}\none\ntwo\nthree\n")).unwrap();
    }
    fs::write(repo.join(".gitignore"), "*.log\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "Initial commit"]);

    fs::write(repo.join("unstaged.txt"), "edited\n").unwrap();
    fs::write(repo.join("staged.txt"), "edited\n").unwrap();
    git(&repo, &["add", "staged.txt"]);
    fs::write(repo.join("both.txt"), "staged\n").unwrap();
    git(&repo, &["add", "both.txt"]);
    fs::write(repo.join("both.txt"), "unstaged\n").unwrap();
    fs::remove_file(repo.join("deleted.txt")).unwrap();
    git(&repo, &["rm", "-q", "removed.txt"]);
    git(&repo, &["mv", "renamed.txt", "moved.txt"]);
    fs::write(repo.join("added.txt"), "added\n").unwrap();
    git(&repo, &["add", "added.txt"]);
    fs::create_dir(repo.join("dir")).unwrap();
    fs::write(repo.join("dir/new file.txt"), "new\n").unwrap();
    fs::write(repo.join("debug.log"), "ignored\n").unwrap();

    let id = open(server.port, &repo).await;
    let expected = json!({
        "isRepo": true,
        "changes": [
            { "path": "added.txt", "kind": "added" },
            { "path": "both.txt", "kind": "modified" },
            { "path": "deleted.txt", "kind": "deleted" },
            { "path": "dir/new file.txt", "kind": "untracked" },
            { "path": "moved.txt", "kind": "renamed", "from": "renamed.txt" },
            { "path": "removed.txt", "kind": "deleted" },
            { "path": "staged.txt", "kind": "modified" },
            { "path": "unstaged.txt", "kind": "modified" },
        ],
    });
    assert_eq!(get(server.port, &format!("/api/projects/{id}/git")).await, expected);
}

#[tokio::test]
async fn a_non_git_directory_is_not_a_repo() {
    let server = Server::start("git").await;
    let dir = server.root.path().join("plain");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("file.txt"), "plain\n").unwrap();

    let id = open(server.port, &dir).await;
    let expected = json!({ "isRepo": false, "changes": [] });
    assert_eq!(get(server.port, &format!("/api/projects/{id}/git")).await, expected);
    assert_eq!(get(server.port, &format!("/api/projects/{id}/git/log")).await, json!({ "commits": [] }));
}

#[tokio::test]
async fn a_commit_is_diffed_against_its_first_parent() {
    let server = Server::start("git").await;
    let repo = server.root.path().join("history");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::write(repo.join("a.txt"), "one\n").unwrap();
    fs::write(repo.join("b.txt"), "b\nb\nb\n").unwrap();
    fs::write(repo.join("gone.txt"), "gone\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "Root"]);
    git(&repo, &["checkout", "-qb", "side"]);
    fs::write(repo.join("side.txt"), "side\n").unwrap();
    git(&repo, &["add", "side.txt"]);
    git(&repo, &["commit", "-qm", "Side"]);
    git(&repo, &["checkout", "-q", "main"]);
    fs::write(repo.join("a.txt"), "two\n").unwrap();
    git(&repo, &["mv", "b.txt", "c.txt"]);
    git(&repo, &["rm", "-q", "gone.txt"]);
    git(&repo, &["commit", "-qam", "Main"]);
    git(&repo, &["merge", "-q", "--no-ff", "side", "-m", "Merge side"]);
    let [merge, main, side, root] = ["HEAD", "HEAD^", "HEAD^2", "HEAD~2"].map(|rev| rev_parse(&repo, rev));

    let id = open(server.port, &repo).await;
    let log = get(server.port, &format!("/api/projects/{id}/git/log")).await;
    let commits = log["commits"].as_array().unwrap();
    let shas: Vec<_> = commits.iter().map(|commit| commit["sha"].as_str().unwrap()).collect();
    assert_eq!(shas, [&merge, &main, &side, &root].map(String::as_str));
    assert_eq!((&commits[0]["subject"], &commits[0]["author"]), (&json!("Merge side"), &json!("oikonomos")));
    assert!(merge.starts_with(commits[0]["short"].as_str().unwrap()));
    assert!(log.get("cursor").is_none());

    let files = |sha: &str| format!("/api/projects/{id}/git/commit/{sha}");
    let expected = json!([{ "path": "side.txt", "kind": "added" }]);
    assert_eq!(get(server.port, &files(&merge)).await, expected);
    let expected = json!([
        { "path": "a.txt", "kind": "modified" },
        { "path": "c.txt", "kind": "renamed", "from": "b.txt" },
        { "path": "gone.txt", "kind": "deleted" },
    ]);
    assert_eq!(get(server.port, &files(&main)).await, expected);
    let expected = json!([
        { "path": "a.txt", "kind": "added" },
        { "path": "b.txt", "kind": "added" },
        { "path": "gone.txt", "kind": "added" },
    ]);
    assert_eq!(get(server.port, &files(&root)).await, expected);

    let port = server.port;
    assert_eq!(sides(port, &id, &merge, "side.txt", "side.txt").await, ["", "side\n"]);
    assert_eq!(sides(port, &id, &merge, "a.txt", "a.txt").await, ["two\n", "two\n"]);
    assert_eq!(sides(port, &id, &main, "b.txt", "c.txt").await, ["b\nb\nb\n", "b\nb\nb\n"]);
    assert_eq!(sides(port, &id, &main, "gone.txt", "gone.txt").await, ["gone\n", ""]);
    assert_eq!(sides(port, &id, &root, "a.txt", "a.txt").await, ["", "one\n"]);
}

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=oikonomos", "-c", "user.email=test@oikonomos.invalid", "-C"])
        .arg(repo)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

async fn open(port: u16, dir: &Path) -> String {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": dir })).await;
    next_control(&mut socket, "project.opened").await["projectId"].as_str().unwrap().to_owned()
}

fn rev_parse(repo: &Path, rev: &str) -> String {
    let output = Command::new("git").arg("-C").arg(repo).args(["rev-parse", rev]).output().unwrap();
    assert!(output.status.success(), "git rev-parse {rev}");
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}

async fn sides(port: u16, id: &str, sha: &str, from: &str, path: &str) -> [String; 2] {
    let show = |rev: &str, path: &str| text(port, format!("/api/projects/{id}/git/show?sha={rev}&path={path}"));
    [show(&format!("{sha}%5E"), from).await, show(sha, path).await]
}

async fn get(port: u16, path: &str) -> Value {
    serde_json::from_str(&text(port, path.to_owned()).await).unwrap()
}

async fn text(port: u16, path: String) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    body.to_owned()
}
