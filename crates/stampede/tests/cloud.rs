use std::{env, fs, path::PathBuf, time::Duration};

use serde_json::{Value, json};
use stampede::cloud::{self, Availability};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, process::Command, time::{sleep, timeout}};

use crate::common::{Server, git, init};

#[tokio::test]
async fn cli_seam() {
    if env::var_os("STAMPEDE_CLOUD_TEST").is_none() {
        let server = Server::start("cloud-").await;
        let url = format!("http://127.0.0.1:{}/api/cloud", server.port);
        let status = || async { serde_json::from_str::<Value>(&reqwest::get(&url).await.unwrap().text().await.unwrap()).unwrap() };
        assert_eq!(status().await, json!({"cli": "signed-out"}));
        fs::write(server.root.path().join("sprite.json"), r#"{"organization":"test-org"}"#).unwrap();
        assert_eq!(status().await, json!({"cli": "ready", "organization": "test-org"}));

        // Isolate PATH and HOME from the other integration tests running concurrently.
        let result = Command::new(env::current_exe().unwrap())
            .args(["--exact", "cloud::cli_seam", "--nocapture"])
            .env("STAMPEDE_CLOUD_TEST", "1")
            .env("STAMPEDE_TEST_ROOT", server.root.path())
            .env("HOME", server.root.path().join("home"))
            .env("PATH", format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap()))
            .status().await.unwrap();
        assert!(result.success());
        return;
    }

    let root = PathBuf::from(env::var_os("STAMPEDE_TEST_ROOT").unwrap());
    let name = "stampede-test";
    assert_eq!(cloud::availability().await, Availability::Ready { organization: "test-org".into() });
    cloud::create(name).await.unwrap();
    let listed = cloud::list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, name);
    assert!(!listed[0].created.is_empty());

    let input = vec![0, 255, 128, 10].repeat(1024 * 1024);
    let output = cloud::exec(name, &["sh", "-c", "cat; printf '%s' \"$TEST_ENV\"; exit 7"], &[("TEST_ENV", "hello")], Some(&input)).await.unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, [input.as_slice(), b"hello"].concat());

    let output = cloud::exec(name, &["sh", "-c", "printf '%s' \"$HOME\""], &[], None).await.unwrap();
    let home = String::from_utf8(output.stdout).unwrap();
    let home = PathBuf::from(home);
    assert_eq!(home, root.join("sprites").join(name).join("home"));
    let local = root.join("payload");
    fs::write(&local, b"pushed").unwrap();
    let remote = home.join("nested/payload");
    cloud::push(name, &local, &remote).await.unwrap();
    assert_eq!(cloud::exec(name, &["cat", remote.to_str().unwrap()], &[], None).await.unwrap().stdout, b"pushed");

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let remote_port = listener.local_addr().unwrap().port();
    let echo = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            socket.read_to_end(&mut bytes).await.unwrap();
            if bytes.is_empty() { continue; }
            socket.write_all(&bytes).await.unwrap();
            break;
        }
    });
    let forward = cloud::forward(name, remote_port).await.unwrap();
    let port = forward.port;
    let mut socket = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    socket.write_all(b"round trip").await.unwrap();
    socket.shutdown().await.unwrap();
    let mut bytes = Vec::new();
    timeout(Duration::from_secs(5), socket.read_to_end(&mut bytes)).await.unwrap().unwrap();
    assert_eq!(bytes, b"round trip");
    echo.await.unwrap();
    drop(forward);
    timeout(Duration::from_secs(5), async {
        while TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();

    let bare = home.join("bare repo");
    let bare = bare.to_str().unwrap();
    let output = cloud::exec(name, &["git", "init", "--bare", bare], &[], None).await.unwrap();
    assert!(output.status.success());
    let repo = init(&root, "repo");
    git(&repo, &["commit", "--allow-empty", "-m", "Initial commit"]);
    git(&repo, &["-c", "protocol.ext.allow=always", "push", &cloud::git_remote(name, bare), "HEAD:refs/heads/main"]);
    let output = cloud::exec(name, &["git", "--git-dir", bare, "rev-parse", "refs/heads/main"], &[], None).await.unwrap();
    assert!(output.status.success());
    let tip = Command::new("git").arg("-C").arg(&repo).args(["rev-parse", "HEAD"]).output().await.unwrap();
    assert_eq!(output.stdout, tip.stdout);

    cloud::destroy(name).await.unwrap();
    assert!(cloud::list().await.unwrap().is_empty());
    assert!(!home.exists());
}

#[tokio::test]
async fn a_cloud_worktree_is_created_and_served_on_its_own_host() {
    use crate::common::{View, chat_turn, control, next_control};
    use tokio_tungstenite::connect_async;

    let mut server = Server::with_env("cloud-worktree-", "", vec![("STAMPEDE_FAKE_FIXTURE", "simple-turn".into())]).await;
    let root = server.root.path().to_owned();
    fs::write(root.join("sprite.json"), r#"{"organization":"test-org"}"#).unwrap();
    let repo = init(&root, "repo with spaces");
    git(&repo, &["config", "user.name", "Cloud Author"]);
    git(&repo, &["config", "user.email", "cloud@example.invalid"]);
    fs::write(repo.join("committed"), "at the starting commit").unwrap();
    git(&repo, &["add", "committed"]);
    git(&repo, &["commit", "-m", "Starting commit"]);
    let tip = Command::new("git").arg("-C").arg(&repo).args(["rev-parse", "HEAD"]).output().await.unwrap().stdout;
    fs::write(repo.join("uncommitted"), "not carried").unwrap();
    let home_ws = format!("ws://127.0.0.1:{}/ws", server.port);
    let (mut socket, _) = connect_async(&home_ws).await.unwrap();
    control(&mut socket, json!({ "type": "project.open", "path": repo })).await;
    let parent = next_control(&mut socket, "project.opened").await["projectId"].clone();
    next_control(&mut socket, "projects.changed").await;
    let home_http = format!("http://127.0.0.1:{}", server.port);
    let create = |branch: &str| {
        let url = format!("{home_http}/api/projects/{}/worktrees", parent.as_str().unwrap());
        let body = json!({ "cloud": "sprite", "branch": branch, "from": "HEAD" }).to_string();
        async move {
            let response = reqwest::Client::new().post(url).header("content-type", "application/json").body(body).send().await.unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(response.headers()["content-type"], "application/x-ndjson");
            response.text().await.unwrap().lines().map(|line| serde_json::from_str::<Value>(line).unwrap()).collect::<Vec<_>>()
        }
    };
    let sprite = |args: &[&str]| {
        let mut command = Command::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake/sprite"));
        command.env("STAMPEDE_TEST_ROOT", &root).args(args);
        command
    };
    fs::write(root.join("sprite-fail"), "push\nFly refused the upload\n").unwrap();
    assert_eq!(create("refused").await, [json!({"step":"create"}), json!({"step":"install"}), json!({"error":"Fly refused the upload"})]);
    let listed: Value = serde_json::from_slice(&sprite(&["list", "--json"]).output().await.unwrap().stdout).unwrap();
    assert_eq!(listed["sprites"], json!([]));
    control(&mut socket, json!({ "type": "projects.list" })).await;
    assert_eq!(next_control(&mut socket, "projects.changed").await["projects"].as_array().unwrap().len(), 1);
    assert!(!root.join("data/cloud-hosts.json").exists());
    fs::remove_file(root.join("sprite-fail")).unwrap();

    let lines = create("feat/cloud").await;
    assert_eq!(&lines[..3], [json!({"step":"create"}), json!({"step":"install"}), json!({"step":"repository"})]);
    assert_eq!(lines.len(), 4, "{lines:?}");
    let host = lines[3]["host"].as_str().unwrap_or_else(|| panic!("{lines:?}"));
    let project = &lines[3]["projectId"];
    let home_id = fs::read_to_string(root.join("data/host-id")).unwrap();
    assert!(host.starts_with(&format!("stampede-{}-", &home_id[..8])));
    let remote_home = root.join("sprites").join(host).join("home");
    let path = remote_home.join("repo with spaces");
    let entry = json!({ "id": project, "host": host, "path": path, "worktree": { "parent": parent, "branch": "feat/cloud" },
        "cloud": { "setup": "ready", "state": "awake", "reason": null, "inSync": true, "setupTerminal": null } });
    let changed = next_control(&mut socket, "projects.changed").await;
    assert!(changed["projects"].as_array().unwrap().contains(&entry), "{changed}");
    assert_eq!(fs::read_to_string(path.join("committed")).unwrap(), "at the starting commit");
    assert!(!path.join("uncommitted").exists());
    let identity: Value = serde_json::from_slice(&fs::read(remote_home.join(".stampede-cloud.json")).unwrap()).unwrap();
    assert_eq!(identity, json!({ "homeId": home_id, "homeName": "Stampede", "project": "repo with spaces", "branch": "feat/cloud" }));
    let git_output = |args: &[&str]| {
        let mut command = Command::new("git");
        command.arg("-C").arg(&path).args(args);
        command
    };
    assert_eq!(git_output(&["rev-parse", "HEAD"]).output().await.unwrap().stdout, tip);
    assert_eq!(git_output(&["config", "user.name"]).output().await.unwrap().stdout, b"Cloud Author\n");
    assert_eq!(git_output(&["config", "user.email"]).output().await.unwrap().stdout, b"cloud@example.invalid\n");
    assert_eq!(git_output(&["status", "--porcelain"]).output().await.unwrap().stdout, b"");
    assert_eq!(git_output(&["remote"]).output().await.unwrap().stdout, b"");

    let cloud_ws = format!("ws://127.0.0.1:{}/cloud/{host}/ws", server.port);
    let (mut cloud_socket, _) = connect_async(&cloud_ws).await.unwrap();
    control(&mut cloud_socket, json!({ "type": "projects.list" })).await;
    let remote_projects = next_control(&mut cloud_socket, "projects.changed").await;
    assert_eq!(remote_projects["projects"], json!([{ "id": project, "path": path }]));
    control(&mut cloud_socket, json!({ "type": "terminal.create", "projectId": project })).await;
    let terminal = next_control(&mut cloud_socket, "terminal.created").await;
    let mut view = View::open_url(&cloud_ws, "device", terminal["terminal"]["id"].as_str()).await;
    view.send(&format!("test \"$HOME\" = '{}' && printf 'remote-%s\\n' yes\r", remote_home.display())).await;
    view.settle(|text| text.contains("remote-yes")).await;
    view.close().await;

    control(&mut cloud_socket, json!({ "type": "chat.create", "projectId": project, "rig": "Claude Code" })).await;
    let chat = next_control(&mut cloud_socket, "chat.created").await["chat"].clone();
    let chat_id = chat["id"].as_str().unwrap();
    control(&mut cloud_socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    next_control(&mut cloud_socket, "chat.items").await;
    let (_, items) = chat_turn(&mut cloud_socket, chat_id, &chat["status"], "Greet the reader in one short sentence with one word in **bold**. Use no names and no tools.").await;
    assert!(items.iter().any(|item| item["text"] == "Hello and **welcome**! 👋"), "{items:?}");
    assert!(remote_home.join(format!(".local/share/stampede/chats/{chat_id}.json")).exists());
    control(&mut cloud_socket, json!({ "type": "chat.close", "chatId": chat_id })).await;

    let url = format!("{home_http}/cloud/{host}/api/projects/{}/git", project.as_str().unwrap());
    let get = || async { reqwest::get(&url).await.unwrap() };
    assert_eq!(serde_json::from_str::<Value>(&get().await.text().await.unwrap()).unwrap()["branch"], "feat/cloud");
    assert_eq!(reqwest::get(format!("{home_http}/cloud/unknown/api/version")).await.unwrap().status(), 404);
    drop(cloud_socket);
    drop(view);
    drop(socket);
    server.restart().await;
    let (mut socket, _) = connect_async(&home_ws).await.unwrap();
    control(&mut socket, json!({ "type": "projects.list" })).await;
    assert!(next_control(&mut socket, "projects.changed").await["projects"].as_array().unwrap().contains(&entry));
    fs::write(root.join("sprite-fail"), "proxy\nFly refused the forward\n").unwrap();
    let failure = get().await;
    assert_eq!(failure.status(), 502);
    assert_eq!(failure.text().await.unwrap(), "Fly refused the forward");
    fs::remove_file(root.join("sprite-fail")).unwrap();
    assert_eq!(get().await.status(), 200);
    assert!(sprite(&["destroy", "--force", host]).status().await.unwrap().success());
}
