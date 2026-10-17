mod common;

use std::{fs, os::unix::fs::symlink};

use common::{Server, control, next_control};
use serde_json::json;
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn deletion_removes_entries_without_following_symlinks_or_leaving_the_project() {
    let server = Server::start("files-").await;
    let root = server.root.path().join("project");
    let outside = server.root.path().join("outside");
    fs::create_dir_all(root.join("folder/nested")).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(root.join("file.txt"), "file").unwrap();
    fs::write(root.join("folder/nested/child.txt"), "child").unwrap();
    fs::write(outside.join("keep.txt"), "keep").unwrap();
    symlink(&outside, root.join("link")).unwrap();
    symlink(outside.join("missing.txt"), root.join("dangling")).unwrap();
    symlink(&outside, root.join("folder/external")).unwrap();
    symlink(&root, root.join("self")).unwrap();

    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "files" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": root })).await;
    let opened = next_control(&mut socket, "project.opened").await;
    let id = opened["projectId"].as_str().unwrap();
    let api = format!("http://127.0.0.1:{}/api/projects/{id}/file", server.port);
    let client = reqwest::Client::new();

    for (path, status) in [
        ("", 400),
        (".", 400),
        ("./", 400),
        ("folder/..", 403),
        ("../outside/keep.txt", 403),
        ("/", 403),
        ("link/keep.txt", 403),
        ("link/../file.txt", 403),
        ("missing.txt", 404),
    ] {
        let mut url = reqwest::Url::parse(&api).unwrap();
        url.query_pairs_mut().append_pair("path", path);
        let response = client.delete(url).send().await.unwrap();
        assert_eq!(response.status().as_u16(), status, "{path}");
    }

    for path in ["file.txt", "folder", "link", "dangling", "self"] {
        let mut url = reqwest::Url::parse(&api).unwrap();
        url.query_pairs_mut().append_pair("path", path);
        let response = client.delete(url).send().await.unwrap();
        assert_eq!(response.status().as_u16(), 204, "{path}");
        assert!(fs::symlink_metadata(root.join(path)).is_err(), "{path}");
    }
    assert!(root.is_dir());
    assert_eq!(fs::read_to_string(outside.join("keep.txt")).unwrap(), "keep");
}
