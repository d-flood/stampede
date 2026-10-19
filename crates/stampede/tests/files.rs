mod common;

use std::{fs, os::unix::fs::{PermissionsExt, symlink}};

use common::{Server, control, next_control};
use serde_json::json;
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn copying_preserves_contents_without_overwriting_or_following_symlinks() {
    let server = Server::start("copy-files-").await;
    let root = server.root.path().join("project");
    let outside = server.root.path().join("outside");
    fs::create_dir_all(root.join("folder/nested")).unwrap();
    fs::create_dir(root.join("destination")).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(root.join("folder/nested/binary.dat"), [0, 255, 128]).unwrap();
    fs::write(root.join("run.sh"), "echo copied").unwrap();
    fs::set_permissions(root.join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.join(".hidden"), "hidden").unwrap();
    fs::write(outside.join("keep.txt"), "keep").unwrap();
    symlink(&outside, root.join("folder/external")).unwrap();
    symlink("missing", root.join("folder/dangling")).unwrap();
    symlink(&outside, root.join("external")).unwrap();
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "copy-files" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": root })).await;
    let opened = next_control(&mut socket, "project.opened").await;
    let id = opened["projectId"].as_str().unwrap();
    let api = format!("http://127.0.0.1:{}/api/projects/{id}/file/copy", server.port);
    let client = reqwest::Client::new();
    for (path, destination, expected) in [
        ("run.sh", "destination", "destination/run.sh"),
        ("run.sh", "destination", "destination/run copy.sh"),
        ("run.sh", "", "run copy.sh"),
        ("run.sh", "", "run copy 2.sh"),
        (".hidden", "", ".hidden copy"),
        ("folder", "", "folder copy"),
        ("folder", "destination", "destination/folder"),
    ] {
        let response = client.post(&api)
            .header("content-type", "application/json")
            .body(json!({ "path": path, "destination": destination }).to_string())
            .send().await.unwrap();
        assert_eq!(response.status(), 201, "{path} -> {destination}");
        let body: serde_json::Value = serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
        assert_eq!(body["path"], expected);
    }
    for path in ["run.sh", "run copy.sh", "run copy 2.sh", "destination/run.sh", "destination/run copy.sh"] {
        assert_eq!(fs::read_to_string(root.join(path)).unwrap(), "echo copied");
        assert_eq!(fs::metadata(root.join(path)).unwrap().permissions().mode() & 0o777, 0o755);
    }
    for path in ["folder copy", "destination/folder"] {
        assert_eq!(fs::read(root.join(path).join("nested/binary.dat")).unwrap(), [0, 255, 128]);
        assert_eq!(fs::read_link(root.join(path).join("external")).unwrap(), outside);
        assert_eq!(fs::read_link(root.join(path).join("dangling")).unwrap().to_str(), Some("missing"));
    }
    for (path, destination, status) in [
        ("", "", 400),
        ("../outside/keep.txt", "", 403),
        ("external/keep.txt", "", 403),
        ("run.sh", "../outside", 403),
        ("run.sh", "external", 403),
        ("folder", "folder", 400),
        ("folder", "folder/nested", 400),
        ("missing", "", 404),
        ("run.sh", "missing", 404),
        ("run.sh", "run.sh", 400),
    ] {
        let response = client.post(&api)
            .header("content-type", "application/json")
            .body(json!({ "path": path, "destination": destination }).to_string())
            .send().await.unwrap();
        assert_eq!(response.status().as_u16(), status, "{path} -> {destination}");
    }
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    assert_eq!(fs::read_to_string(outside.join("keep.txt")).unwrap(), "keep");
    assert_eq!(fs::read_dir(root.join("folder/nested")).unwrap().count(), 1);
}

#[tokio::test]
async fn image_previews_return_bytes_with_image_types_and_stay_within_the_project() {
    let server = Server::start("image-files-").await;
    let root = server.root.path().join("project");
    fs::create_dir(&root).unwrap();
    let bytes = [0, 255, 128, 1];
    for file in ["photo.JPEG", "space & hash #.webp", "image.png"] {
        fs::write(root.join(file), bytes).unwrap();
    }
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>";
    fs::write(root.join("drawing.svg"), svg).unwrap();
    fs::write(root.join("page.html"), "<script>alert(1)</script>").unwrap();
    fs::write(server.root.path().join("outside.png"), bytes).unwrap();
    symlink(server.root.path().join("outside.png"), root.join("external.png")).unwrap();
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "image-files" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": root })).await;
    let opened = next_control(&mut socket, "project.opened").await;
    let id = opened["projectId"].as_str().unwrap();
    let api = format!("http://127.0.0.1:{}/api/projects/{id}", server.port);
    let client = reqwest::Client::new();
    for (file, mime) in [
        ("photo.JPEG", "image/jpeg"),
        ("space & hash #.webp", "image/webp"),
        ("image.png", "image/png"),
        ("drawing.svg", "image/svg+xml"),
    ] {
        let mut url = reqwest::Url::parse(&format!("{api}/image")).unwrap();
        url.query_pairs_mut().append_pair("path", file);
        let response = client.get(url).send().await.unwrap();
        assert_eq!(response.status(), 200, "{file}");
        assert_eq!(response.headers()["content-type"], mime);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert_eq!(response.headers()["content-security-policy"], "sandbox; default-src 'none'");
        let expected: &[u8] = if file.ends_with("svg") { svg.as_bytes() } else { &bytes };
        assert_eq!(response.bytes().await.unwrap().as_ref(), expected);
    }
    for (path, status) in [
        ("page.html", 415),
        ("missing.png", 404),
        ("../outside.png", 403),
        ("external.png", 403),
    ] {
        let mut url = reqwest::Url::parse(&format!("{api}/image")).unwrap();
        url.query_pairs_mut().append_pair("path", path);
        let response = client.get(url).send().await.unwrap();
        assert_eq!(response.status().as_u16(), status, "{path}");
    }
    let response = client.get(format!("{api}/file?path=drawing.svg")).send().await.unwrap();
    let contents: serde_json::Value = serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
    assert_eq!(contents["content"], svg);
}

#[tokio::test]
async fn creation_is_confined_to_the_project_and_never_overwrites_entries() {
    let server = Server::start("create-files-").await;
    let root = server.root.path().join("project");
    let outside = server.root.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(root.join("keep.txt"), "keep").unwrap();
    symlink(&outside, root.join("external")).unwrap();
    symlink(outside.join("missing.txt"), root.join("dangling")).unwrap();
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "create-files" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": root })).await;
    let opened = next_control(&mut socket, "project.opened").await;
    let id = opened["projectId"].as_str().unwrap();
    let api = format!("http://127.0.0.1:{}/api/projects/{id}/file", server.port);
    let client = reqwest::Client::new();
    for (path, dir, status) in [
        ("notes.txt", false, 201),
        ("docs", true, 201),
        ("docs/child.txt", false, 201),
        ("docs/nested", true, 201),
        ("keep.txt", false, 409),
        ("keep.txt", true, 409),
        ("docs", false, 409),
        ("docs", true, 409),
        ("dangling", false, 409),
        ("dangling", true, 409),
        ("", false, 400),
        (".", true, 403),
        ("../escaped.txt", false, 403),
        ("/escaped", true, 403),
        ("external/escaped.txt", false, 403),
        ("external/escaped", true, 403),
        ("docs/../escaped.txt", false, 403),
    ] {
        let response = client.post(&api)
            .header("content-type", "application/json")
            .body(json!({ "path": path, "dir": dir }).to_string())
            .send().await.unwrap();
        assert_eq!(response.status().as_u16(), status, "{path}, dir={dir}");
    }
    assert_eq!(fs::read_to_string(root.join("keep.txt")).unwrap(), "keep");
    assert_eq!(fs::read(root.join("notes.txt")).unwrap(), Vec::<u8>::new());
    assert!(root.join("docs/child.txt").is_file());
    assert!(root.join("docs/nested").is_dir());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    assert!(!server.root.path().join("escaped.txt").exists());
}

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
