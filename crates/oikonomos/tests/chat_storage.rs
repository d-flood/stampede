mod common;

use std::{env, fs, path::{Path, PathBuf}, time::Duration};

use common::{Server, control, git, init, leftovers, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::{sleep, timeout}};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn create(socket: &mut Socket, path: &Path, rig: &str) -> String {
    control(socket, json!({ "type": "project.open", "path": path })).await;
    let project = next_control(socket, "project.opened").await;
    control(socket, json!({ "type": "chat.create", "projectId": project["projectId"], "rig": rig })).await;
    next_control(socket, "chat.created").await["chat"]["id"].as_str().unwrap().to_owned()
}

async fn connect(port: u16) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "storage" })).await;
    next_control(&mut socket, "hello").await;
    socket
}

#[tokio::test]
async fn both_agents_get_owned_scratch_and_shared_worktree_build_settings() {
    for agent in ["claude-code", "codex"] {
        let capture = tempfile::tempdir().unwrap();
        let output = capture.path().join("environment.jsonl");
        let fixture = "simple-turn";
        let config = format!("[[rig]]\nname = \"test\"\nagent = \"{agent}\"\nenv = {{ OIKONOMOS_FAKE_FIXTURE = \"{fixture}\", OIKONOMOS_FAKE_ENVIRONMENT = {:?} }}\n", output);
        let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
        let server = Server::with_env("chat-storage", &config, vec![("PATH", path.into())]).await;
        let repo = init(server.root.path(), "repo");
        fs::write(repo.join("Cargo.toml"), "[workspace]\n").unwrap();
        git(&repo, &["add", "Cargo.toml"]);
        git(&repo, &["-c", "user.name=test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false", "commit", "-qm", "Fixture"]);
        let other = server.root.path().join("other");
        git(&repo, &["worktree", "add", "--detach", other.to_str().unwrap()]);
        let mut socket = connect(server.port).await;
        let first = create(&mut socket, &repo, "test").await;
        let second = create(&mut socket, &other, "test").await;
        let environments = timeout(Duration::from_secs(5), async {
            loop {
                let environments: Vec<Value> = fs::read_to_string(&output).unwrap_or_default().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
                if environments.len() == 2 { break environments }
                sleep(Duration::from_millis(20)).await;
            }
        }).await.unwrap();
        let first_scratch = PathBuf::from(environments[0]["TMPDIR"].as_str().unwrap());
        let second_scratch = PathBuf::from(environments[1]["TMPDIR"].as_str().unwrap());
        assert_ne!(first_scratch, second_scratch);
        assert!(first_scratch.starts_with(server.root.path().join("data/storage/scratch")), "{first_scratch:?} outside {:?}", server.root.path());
        assert!(second_scratch.exists());
        for environment in &environments {
            assert_eq!(environment["CLAUDE_CODE_TMPDIR"], environment["TMPDIR"]);
            assert_eq!(Path::new(environment["CARGO_TARGET_DIR"].as_str().unwrap()), repo.join("target"));
            assert_eq!(environment["NODE_COMPILE_CACHE"], environments[0]["NODE_COMPILE_CACHE"]);
        }
        control(&mut socket, json!({ "type": "chat.close", "chatId": first })).await;
        timeout(Duration::from_secs(5), async {
            while first_scratch.exists() { sleep(Duration::from_millis(20)).await; }
        }).await.expect("closed Chat scratch was retained");
        assert!(second_scratch.exists());
        control(&mut socket, json!({ "type": "chat.close", "chatId": second })).await;
        timeout(Duration::from_secs(5), async {
            while second_scratch.exists() || server.root.path().join(format!("data/chats/{second}.json")).exists() || !leftovers(server.root.path()).is_empty() {
                sleep(Duration::from_millis(20)).await;
            }
        }).await.expect("closed Chats left temporary storage or child processes");
    }
}
