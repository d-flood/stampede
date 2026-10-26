mod common;

use std::{fs, path::Path, process::Command, time::Duration};

use common::{Server, control, next_control, watching};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn open(server: &Server, root: &Path) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "watcher" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": root })).await;
    next_control(&mut socket, "project.opened").await;
    watching(&mut socket, root).await;
    socket
}

fn paths(changed: &Value) -> impl Iterator<Item = &str> {
    changed["paths"].as_array().unwrap().iter().map(|path| path.as_str().unwrap())
}

/// Every path reported until `path` is.
async fn until(socket: &mut Socket, path: &str) -> Vec<String> {
    let mut reported = Vec::new();
    let arrived = timeout(Duration::from_secs(10), async {
        while !reported.iter().any(|listed| listed == path) {
            reported.extend(paths(&next_control(socket, "fs.changed").await).map(str::to_owned));
        }
    })
    .await;
    assert!(arrived.is_ok(), "{path} was never reported, only {reported:?}");
    reported
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git").arg("-C").arg(dir).args(args).status().unwrap().success());
}

#[tokio::test]
async fn the_watcher_follows_nested_new_renamed_and_removed_directories() {
    let server = Server::start("watcher-").await;
    let root = server.root.path().join("project");
    fs::create_dir_all(root.join("a/b")).unwrap();
    let mut socket = open(&server, &root).await;

    fs::write(root.join("a/b/nested.txt"), "nested").unwrap();
    until(&mut socket, "a/b/nested.txt").await;

    fs::create_dir_all(root.join("new/inner")).unwrap();
    fs::write(root.join("new/inner/first.txt"), "first").unwrap();
    assert!(until(&mut socket, "new/inner/first.txt").await.iter().any(|path| path == "new"));
    fs::write(root.join("new/inner/second.txt"), "second").unwrap();
    until(&mut socket, "new/inner/second.txt").await;

    fs::rename(root.join("new"), root.join("moved")).unwrap();
    assert!(until(&mut socket, "moved").await.iter().any(|path| path == "new"));
    fs::write(root.join("moved/inner/third.txt"), "third").unwrap();
    until(&mut socket, "moved/inner/third.txt").await;

    fs::remove_dir_all(root.join("moved")).unwrap();
    until(&mut socket, "moved").await;
    fs::create_dir_all(root.join("moved/inner")).unwrap();
    fs::write(root.join("moved/inner/fourth.txt"), "fourth").unwrap();
    until(&mut socket, "moved/inner/fourth.txt").await;
    fs::write(root.join("moved/inner/fifth.txt"), "fifth").unwrap();
    until(&mut socket, "moved/inner/fifth.txt").await;
}

#[tokio::test]
async fn the_watcher_skips_ignored_paths_and_reports_git_changes() {
    let server = Server::start("watcher-git-").await;
    let root = server.root.path().join("project");
    fs::create_dir_all(root.join("build")).unwrap();
    fs::write(root.join(".gitignore"), "build/\n").unwrap();
    git(&root, &["init", "-q"]);
    let mut socket = open(&server, &root).await;

    fs::write(root.join("build/output.txt"), "ignored").unwrap();
    fs::write(root.join("kept.txt"), "kept").unwrap();
    let reported = until(&mut socket, "kept.txt").await;
    assert!(!reported.iter().any(|path| path.starts_with("build")), "ignored paths were reported: {reported:?}");

    git(&root, &["add", "kept.txt"]);
    let signalled = timeout(Duration::from_secs(10), async {
        while next_control(&mut socket, "fs.changed").await["git"] != true {}
    });
    signalled.await.expect("staging a file never signalled git");
}
