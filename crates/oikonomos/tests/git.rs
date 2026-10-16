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

async fn get(port: u16, path: &str) -> Value {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    serde_json::from_str(body).unwrap()
}
