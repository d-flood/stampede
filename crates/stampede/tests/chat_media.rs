mod common;

use std::{fs, io::Write, path::Path, process::{Command, Stdio}};

use common::Server;
use serde_json::{Value, json};

fn call(data_dir: &Path, arguments: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stampede"))
        .arg("chat-tools").arg("--data-dir").arg(data_dir)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    writeln!(child.stdin.take().unwrap(), "{}", json!({ "id": 1, "method": "tools/call", "params": {
        "name": "render_media", "arguments": arguments,
    } })).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"].clone()
}

#[tokio::test]
async fn chat_media_copies_files_and_serves_seekable_assets_after_restart() {
    let mut server = Server::start("stampede-media-").await;
    let data_dir = server.root.path().join("data");
    let client = reqwest::Client::new();
    for (extension, bytes, mime) in [
        ("png", &include_bytes!("../../../web/static/icons/icon-192.png")[..], "image/png"),
        ("jpg", &b"\xff\xd8\xff\xe0test"[..], "image/jpeg"),
        ("gif", &b"GIF89a0123456789"[..], "image/gif"),
        ("webp", &b"RIFFxxxxWEBPxxxx"[..], "image/webp"),
        ("mp4", &b"\0\0\0\x18ftypisom\0\0\0\0isommp42"[..], "video/mp4"),
        ("webm", &b"\x1a\x45\xdf\xa3webmtestdata"[..], "video/webm"),
    ] {
        let source = server.root.path().join("screenshot.tmp");
        fs::write(&source, bytes).unwrap();
        let result = call(&data_dir, json!({ "path": source, "title": "Screenshot or recording" }));
        assert_eq!(result["isError"], false, "{result}");
        let asset = result["structuredContent"]["asset"].as_str().unwrap();
        assert!(asset.ends_with(&format!(".{extension}")));
        assert_eq!(serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(), result["structuredContent"]);
        fs::remove_file(source).unwrap();
        server.restart().await;
        let url = format!("http://127.0.0.1:{}/api/media/{asset}", server.port);
        let response = client.get(&url).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["content-type"], mime);
        assert_eq!(response.headers()["accept-ranges"], "bytes");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert_eq!(response.bytes().await.unwrap().as_ref(), bytes);
        for (range, start, end) in [("bytes=2-5", 2, 5), ("bytes=4-", 4, bytes.len() - 1), ("bytes=-4", bytes.len() - 4, bytes.len() - 1), ("bytes=0-999999", 0, bytes.len() - 1)] {
            let response = client.get(&url).header("range", range).send().await.unwrap();
            assert_eq!(response.status(), 206);
            assert_eq!(response.headers()["content-range"], format!("bytes {start}-{end}/{}", bytes.len()));
            assert_eq!(response.bytes().await.unwrap().as_ref(), &bytes[start..=end]);
        }
        for range in ["bytes=999999-", "bytes=5-2", "bytes=-0"] {
            let response = client.get(&url).header("range", range).send().await.unwrap();
            assert_eq!(response.status(), 416);
            assert_eq!(response.headers()["content-range"], format!("bytes */{}", bytes.len()));
        }
        let response = client.head(&url).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["content-length"], bytes.len().to_string());
        assert!(response.bytes().await.unwrap().is_empty());
    }
    for asset in ["not-an-asset.png", "00000000-0000-0000-0000-000000000000.svg", "00000000-0000-0000-0000-000000000000.png", "%2e%2e%2fconfig.toml"] {
        let response = client.get(format!("http://127.0.0.1:{}/api/media/{asset}", server.port)).send().await.unwrap();
        assert_eq!(response.status(), 404);
    }
}

#[test]
fn chat_media_rejects_invalid_files_and_arguments_without_publishing() {
    let root = tempfile::tempdir().unwrap();
    let data_dir = root.path().join("data");
    let source = root.path().join("image.png");
    fs::write(&source, "<svg onload='alert(1)'></svg>").unwrap();
    for arguments in [
        json!({ "path": source, "title": "Untrusted image" }),
        json!({ "path": root.path().join("missing.png"), "title": "Missing" }),
        json!({ "path": root.path(), "title": "Directory" }),
        json!({ "path": "relative.png", "title": "Relative" }),
        json!({ "path": source, "title": " " }),
        json!({ "path": source, "title": "x".repeat(201) }),
        json!({ "path": source, "title": "Extra", "extra": true }),
    ] {
        assert_eq!(call(&data_dir, arguments)["isError"], true);
    }
    fs::write(&source, "").unwrap();
    assert_eq!(call(&data_dir, json!({ "path": source, "title": "Empty" }))["isError"], true);
    fs::File::create(&source).unwrap().set_len(100 * 1024 * 1024 + 1).unwrap();
    assert_eq!(call(&data_dir, json!({ "path": source, "title": "Too large" }))["isError"], true);
    assert!(!data_dir.join("media").exists());
}
