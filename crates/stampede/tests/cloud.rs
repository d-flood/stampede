use std::{env, fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

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
        assert_eq!(status().await, json!({"cli": "signed-out", "claudeToken": false}));
        fs::write(server.root.path().join("sprite.json"), r#"{"organization":"test-org"}"#).unwrap();
        assert_eq!(status().await, json!({"cli": "ready", "organization": "test-org", "claudeToken": false}));

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
    git(&repo, &["commit", "--allow-empty", "-m", "Without a Cloud Setup"]);
    let setup = |run: &str| {
        fs::create_dir_all(repo.join(".stampede")).unwrap();
        fs::write(repo.join(".stampede/cloud-setup.toml"), format!("carry = [\".env\", \"absent.toml\"]\nrun = {run:?}\n")).unwrap();
        git(&repo, &["add", ".stampede"]);
        git(&repo, &["commit", "-m", "Cloud Setup"]);
    };
    git(&repo, &["switch", "-c", "broken"]);
    setup("test -e fixed || exit 3");
    git(&repo, &["switch", "-"]);
    fs::write(repo.join("committed"), "at the starting commit").unwrap();
    fs::write(repo.join(".gitignore"), ".env\nmarker\nfixed\n").unwrap();
    git(&repo, &["add", "committed", ".gitignore"]);
    setup("test -f .env && printf marked > marker");
    let tip = Command::new("git").arg("-C").arg(&repo).args(["rev-parse", "HEAD"]).output().await.unwrap().stdout;
    fs::write(repo.join("uncommitted"), "not carried").unwrap();
    fs::write(repo.join(".env"), "carried").unwrap();
    let home_ws = format!("ws://127.0.0.1:{}/ws", server.port);
    let (mut socket, _) = connect_async(&home_ws).await.unwrap();
    control(&mut socket, json!({ "type": "project.open", "path": repo })).await;
    let parent = next_control(&mut socket, "project.opened").await["projectId"].clone();
    next_control(&mut socket, "projects.changed").await;
    let home_http = format!("http://127.0.0.1:{}", server.port);
    let request = |branch: &str, from: &str| {
        let url = format!("{home_http}/api/projects/{}/worktrees", parent.as_str().unwrap());
        let body = json!({ "cloud": "sprite", "branch": branch, "from": from }).to_string();
        reqwest::Client::new().post(url).header("content-type", "application/json").body(body).send()
    };
    let create_from = |branch: &str, from: &str| {
        let response = request(branch, from);
        async move {
            let response = response.await.unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(response.headers()["content-type"], "application/x-ndjson");
            response.text().await.unwrap().lines().map(|line| serde_json::from_str::<Value>(line).unwrap()).collect::<Vec<_>>()
        }
    };
    let create = |branch: &str| create_from(branch, "HEAD");
    let sprite = |args: &[&str]| {
        let mut command = Command::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake/sprite"));
        command.env("STAMPEDE_TEST_ROOT", &root).args(args);
        command
    };
    let refused = request("bare", "HEAD~1").await.unwrap();
    assert_eq!((refused.status().as_u16(), refused.text().await.unwrap().as_str()), (400, "HEAD~1 has no Cloud Setup. Type /cloud-setup in a Chat to draft one."));
    let listed: Value = serde_json::from_slice(&sprite(&["list", "--json"]).output().await.unwrap().stdout).unwrap();
    assert_eq!(listed["sprites"], json!([]));
    fs::write(root.join("sprite-fail"), "push\nFly refused the upload\n").unwrap();
    assert_eq!(create("refused").await, [json!({"step":"create"}), json!({"step":"install"}), json!({"error":"Fly refused the upload"})]);
    let listed: Value = serde_json::from_slice(&sprite(&["list", "--json"]).output().await.unwrap().stdout).unwrap();
    assert_eq!(listed["sprites"], json!([]));
    control(&mut socket, json!({ "type": "projects.list" })).await;
    assert_eq!(next_control(&mut socket, "projects.changed").await["projects"].as_array().unwrap().len(), 1);
    assert!(!root.join("data/cloud-hosts.json").exists());
    fs::remove_file(root.join("sprite-fail")).unwrap();

    let client = reqwest::Client::new();
    let token_route = format!("{home_http}/api/cloud/claude-token");
    let token = "fake-claude-setup-token";
    assert_eq!(client.put(&token_route).header("content-type", "application/json").body(json!(token).to_string()).send().await.unwrap().status(), 204);
    let cloud_status = reqwest::get(format!("{home_http}/api/cloud")).await.unwrap().text().await.unwrap();
    assert_eq!(serde_json::from_str::<Value>(&cloud_status).unwrap()["claudeToken"], true);
    assert!(!cloud_status.contains(token));
    assert!(!reqwest::get(format!("{home_http}/api/config/file")).await.unwrap().text().await.unwrap().contains(token));
    assert!(!fs::read_to_string(root.join("config.toml")).unwrap().contains(token));
    assert_eq!(fs::metadata(root.join("data/cloud-logins.json")).unwrap().permissions().mode() & 0o777, 0o600);
    fs::write(root.join("gh.json"), r#"{"token":"fake-github-token","user":"cloud-test","repo":{},"prs":[]}"#).unwrap();
    fs::create_dir_all(root.join("home/.codex")).unwrap();
    fs::write(root.join("home/.codex/auth.json"), r#"{"fake":true}"#).unwrap();
    let lines = create("feat/cloud").await;
    assert_eq!(&lines[..5], [json!({"step":"create"}), json!({"step":"install"}), json!({"step":"repository"}), json!({"step":"carry"}), json!({"step":"toolkit"})]);
    assert_eq!(lines.len(), 6, "{lines:?}");
    let host = lines[5]["host"].as_str().unwrap_or_else(|| panic!("{lines:?}"));
    let project = &lines[5]["projectId"];
    let home_id = fs::read_to_string(root.join("data/host-id")).unwrap();
    assert!(host.starts_with(&format!("stampede-{}-", &home_id[..8])));
    let remote_home = root.join("sprites").join(host).join("home");
    let path = remote_home.join("repo with spaces");
    let auth = remote_home.join(".codex/auth.json");
    assert_eq!(fs::read_to_string(&auth).unwrap(), r#"{"fake":true}"#);
    assert_eq!(fs::metadata(auth).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(fs::metadata(remote_home.join(".local/share/stampede/cloud-logins.json")).unwrap().permissions().mode() & 0o777, 0o600);
    let entry = setup_reaches(&mut socket, project, "ready").await;
    let setup_terminal = entry["cloud"]["setupTerminal"].as_str().unwrap().to_owned();
    assert_eq!(entry, json!({ "id": project, "host": host, "path": path, "worktree": { "parent": parent, "branch": "feat/cloud" },
        "cloud": { "setup": "ready", "state": "awake", "reason": null, "inSync": true, "setupTerminal": setup_terminal } }));
    assert_eq!(fs::read_to_string(path.join("committed")).unwrap(), "at the starting commit");
    assert_eq!(fs::read_to_string(path.join(".env")).unwrap(), "carried");
    assert_eq!(fs::read_to_string(path.join("marker")).unwrap(), "marked");
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
    let mut view = View::open_url(&cloud_ws, "device", Some(&setup_terminal)).await;
    view.settle(|text| text.contains("Skipped absent.toml: not on the Home Host")).await;
    assert!(!view.text().contains("Skipped .env"), "{}", view.text());
    view.send("printf 'login:%s:%s\\n' \"$CLAUDE_CODE_OAUTH_TOKEN\" \"$GH_TOKEN\"\r").await;
    view.settle(|text| text.contains("login:fake-claude-setup-token:fake-github-token")).await;
    drop(view);

    let lines = create_from("feat/broken", "broken").await;
    let broken = lines.last().unwrap()["host"].as_str().unwrap_or_else(|| panic!("{lines:?}")).to_owned();
    let broken_project = &lines.last().unwrap()["projectId"];
    setup_reaches(&mut socket, broken_project, "failed").await;
    let (mut broken_socket, _) = connect_async(format!("ws://127.0.0.1:{}/cloud/{broken}/ws", server.port)).await.unwrap();
    control(&mut broken_socket, json!({ "type": "hello", "deviceId": "device" })).await;
    let failed = next_control(&mut broken_socket, "cloud.setup").await;
    assert_eq!((&failed["state"], &failed["status"], &failed["command"]), (&json!("failed"), &json!(3), &json!("test -e fixed || exit 3")));
    let listed: Value = serde_json::from_slice(&sprite(&["list", "--json"]).output().await.unwrap().stdout).unwrap();
    assert!(listed["sprites"].as_array().unwrap().iter().any(|listed| listed["name"] == *broken), "{listed}");
    fs::write(root.join("sprites").join(&broken).join("home/repo with spaces/fixed"), "").unwrap();
    let again = reqwest::Client::new().post(format!("{home_http}/cloud/{broken}/api/cloud-setup/again")).send().await.unwrap();
    assert_eq!(again.status(), 200);
    assert_eq!(setup_reaches(&mut socket, broken_project, "ready").await["cloud"]["setupTerminal"], failed["terminalId"]);
    drop(broken_socket);
    assert!(sprite(&["destroy", "--force", &broken]).status().await.unwrap().success());

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
    assert_eq!(client.put(&token_route).header("content-type", "application/json").body("null").send().await.unwrap().status(), 204);

    let refusal = "A Cloud Host holds one worktree and nothing else.";
    let cloud_http = format!("{home_http}/cloud/{host}/api");
    let projects = format!("{cloud_http}/projects/{}", project.as_str().unwrap());
    let client = reqwest::Client::new();
    for request in [
        client.post(format!("{projects}/worktrees")).body(r#"{"branch":"nested","from":"HEAD"}"#),
        client.get(format!("{projects}/prs")),
        client.get(format!("{projects}/pr-reviews")),
        client.post(format!("{projects}/pr-reviews/1/worktree")),
        client.post(format!("{projects}/pr-reviews/1/drafts")).body("{}"),
    ] {
        let response = request.header("content-type", "application/json").send().await.unwrap();
        assert_eq!((response.status().as_u16(), response.text().await.unwrap().as_str()), (403, refusal));
    }
    let recorded = client.post(format!("{cloud_http}/epics/none")).header("content-type", "application/json").body(r#"{"kind":"epic.archived"}"#).send().await.unwrap();
    assert_eq!((recorded.status().as_u16(), recorded.text().await.unwrap().as_str()), (400, refusal));
    let mut add = Command::new(env!("CARGO_BIN_EXE_stampede"));
    add.args(["epic", "add", "none", "--title", "None", "--brief", "committed", "--data-dir"]).arg(remote_home.join(".local/share/stampede")).current_dir(&path);
    let added = add.output().await.unwrap();
    assert!(!added.status.success() && String::from_utf8_lossy(&added.stderr).contains(refusal), "{added:?}");

    control(&mut cloud_socket, json!({ "type": "chat.resume", "projectId": project, "agent": "antigravity", "sessionId": "session" })).await;
    control(&mut cloud_socket, json!({ "type": "chat.create", "projectId": project, "rig": "Claude Code" })).await;
    let chat = next_control(&mut cloud_socket, "chat.created").await["chat"].clone();
    assert_eq!(chat["agent"], "claude-code");
    control(&mut cloud_socket, json!({ "type": "chat.move", "chatId": chat["id"], "branch": "nested" })).await;
    assert_eq!(next_control(&mut cloud_socket, "chat.move.error").await["error"], refusal);
    let url = format!("{home_http}/cloud/{host}/api/projects/{}/git", project.as_str().unwrap());
    let get = || async { reqwest::get(&url).await.unwrap() };
    assert_eq!(serde_json::from_str::<Value>(&get().await.text().await.unwrap()).unwrap()["branch"], "feat/cloud");
    assert_eq!(reqwest::get(format!("{home_http}/cloud/unknown/api/version")).await.unwrap().status(), 404);
    drop(cloud_socket);
    drop(view);
    drop(socket);
    // Fail the forward before the restarted Home Host links the Cloud Host and holds one.
    fs::write(root.join("sprite-fail"), "proxy\nFly refused the forward\n").unwrap();
    server.restart().await;
    let (mut socket, _) = connect_async(&home_ws).await.unwrap();
    control(&mut socket, json!({ "type": "projects.list" })).await;
    assert!(next_control(&mut socket, "projects.changed").await["projects"].as_array().unwrap().contains(&entry));
    let failure = get().await;
    assert_eq!(failure.status(), 502);
    assert_eq!(failure.text().await.unwrap(), "Fly refused the forward");
    fs::remove_file(root.join("sprite-fail")).unwrap();
    assert_eq!(get().await.status(), 200);
    assert!(sprite(&["destroy", "--force", host]).status().await.unwrap().success());
}

#[tokio::test]
async fn the_home_host_links_each_cloud_host_for_its_settings_attention_and_push() {
    use crate::common::{control, next_control, pushed, subscribe};
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    use futures_util::StreamExt;

    let gates = tempfile::tempdir().unwrap();
    let fixture = format!("{}/tests/fixtures/chats/claude-code/background-shell-idle.synthetic.jsonl", env!("CARGO_MANIFEST_DIR"));
    let config = format!("[[rig]]\nname = \"Gated\"\nagent = \"claude-code\"\nenv = {{ STAMPEDE_FAKE_FIXTURE = {fixture:?}, STAMPEDE_FAKE_GATES = {:?} }}\n", gates.path());
    let mut server = Server::with_config("cloud-link-", &config).await;
    let root = server.root.path().to_owned();
    fs::write(root.join("sprite.json"), r#"{"organization":"test-org"}"#).unwrap();
    let repo = init(&root, "repo");
    fs::create_dir(repo.join(".stampede")).unwrap();
    fs::write(repo.join(".stampede/cloud-setup.toml"), "run = \"true\"\n").unwrap();
    git(&repo, &["add", ".stampede"]);
    git(&repo, &["commit", "-m", "Starting commit"]);
    let mut pushes = subscribe(&server, &["away"]).await;
    let home_ws = format!("ws://127.0.0.1:{}/ws", server.port);
    let home_http = format!("http://127.0.0.1:{}", server.port);
    let (mut socket, _) = connect_async(&home_ws).await.unwrap();
    control(&mut socket, json!({ "type": "project.open", "path": repo })).await;
    let parent = next_control(&mut socket, "project.opened").await["projectId"].as_str().unwrap().to_owned();
    drop(socket);
    let client = reqwest::Client::new();
    let created = client.post(format!("{home_http}/api/projects/{parent}/worktrees")).header("content-type", "application/json")
        .body(json!({ "cloud": "sprite", "branch": "feat/link", "from": "HEAD" }).to_string()).send().await.unwrap().text().await.unwrap();
    let created: Value = serde_json::from_str(created.lines().last().unwrap()).unwrap();
    let (host, project) = (created["host"].as_str().unwrap_or_else(|| panic!("{created}")), &created["projectId"]);
    let cloud_http = format!("{home_http}/cloud/{host}/api");

    let rig = json!({ "name": "Linked", "agent": "codex", "mode": "full-access" });
    let saved = client.put(format!("{home_http}/api/rigs")).header("content-type", "application/json").body(rig.to_string()).send().await.unwrap();
    assert_eq!(saved.status(), 200);
    timeout(Duration::from_secs(20), async {
        loop {
            let rigs: Value = serde_json::from_str(&client.get(format!("{cloud_http}/rigs")).send().await.unwrap().text().await.unwrap()).unwrap();
            let names: Vec<_> = rigs.as_array().unwrap().iter().map(|rig| rig["name"].clone()).collect();
            if names == [json!("Gated"), json!("Linked")] { break; }
            sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("the Home Host's Rigs never reached the Cloud Host");
    let refused = client.put(format!("{cloud_http}/theme")).header("content-type", "application/json").body(r#""Shale""#).send().await.unwrap();
    assert_eq!((refused.status().as_u16(), refused.text().await.unwrap().as_str()), (403, "Settings come from the Home Host."));

    let (mut cloud, _) = connect_async(format!("ws://127.0.0.1:{}/cloud/{host}/ws", server.port)).await.unwrap();
    control(&mut cloud, json!({ "type": "chat.create", "projectId": project, "rig": "Gated" })).await;
    let chat_id = next_control(&mut cloud, "chat.created").await["chat"]["id"].as_str().unwrap().to_owned();
    control(&mut cloud, json!({ "type": "chat.send", "chatId": chat_id, "text": "Greet the reader" })).await;
    let gate = gates.path().join("shell");
    timeout(Duration::from_secs(20), async { while !gate.exists() { sleep(Duration::from_millis(20)).await; } }).await.unwrap();
    drop(cloud);
    sleep(Duration::from_secs(1)).await;
    fs::remove_file(&gate).unwrap();
    let item = json!({ "host": host, "projectId": project, "chatId": chat_id, "label": "Gated", "status": "done" });
    assert_eq!(pushed(&mut pushes).await, ("away".into(), item.clone()));
    let listed = || async {
        let (mut socket, _) = connect_async(&home_ws).await.unwrap();
        control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
        timeout(Duration::from_secs(20), async {
            loop {
                let Message::Text(text) = socket.next().await.unwrap().unwrap() else { continue };
                let message: Value = serde_json::from_str(&text).unwrap();
                if message["type"] == "attention.changed" && message["items"] == json!([item]) { break; }
            }
        }).await.expect("the Home Host never listed the Cloud Host's Chat");
    };
    listed().await;
    sleep(Duration::from_secs(2)).await;
    assert!(pushes.try_recv().is_err(), "the Chat was announced twice");

    server.restart().await;
    listed().await;
}

/// Waits for the Home Host to list `project` with its Cloud Setup in `state`, and returns its entry.
async fn setup_reaches(socket: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>, project: &Value, state: &str) -> Value {
    timeout(Duration::from_secs(30), async {
        loop {
            let changed = crate::common::next_control(socket, "projects.changed").await;
            if let Some(entry) = changed["projects"].as_array().unwrap().iter().find(|entry| entry["id"] == *project && entry["cloud"]["setup"] == state) {
                return entry.clone();
            }
        }
    }).await.unwrap_or_else(|_| panic!("the Cloud Setup of {project} never became {state}"))
}
