mod common;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use common::{Server, chat_turn, close_chat, control, git, init, next, next_control};
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    time::{Instant, sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[tokio::test]
async fn a_claude_chat_moves_to_a_new_worktree_with_its_uncommitted_changes() {
    moves("claude-code", "simple-turn").await;
}

#[tokio::test]
async fn a_codex_chat_moves_to_a_new_worktree_with_its_uncommitted_changes() {
    moves("codex", &format!("{}/tests/fixtures/chats/codex/move.synthetic.jsonl", env!("CARGO_MANIFEST_DIR"))).await;
}

#[tokio::test]
async fn a_copilot_chat_moves_to_a_new_worktree_with_its_uncommitted_changes() {
    moves("copilot", "simple-turn").await;
}

async fn moves(agent: &str, fixture: &str) {
    let (server, mut socket, repo, project, source) = launch(agent, fixture).await;
    let source_id = source["id"].as_str().unwrap();
    chat_turn(&mut socket, source_id, &source["status"], "Say hello").await;
    let session = record(&server, source_id)["agentSession"].clone();
    assert!(session.is_string());
    fs::write(repo.join("tracked.txt"), "changed\n").unwrap();
    fs::write(repo.join("staged.txt"), "staged\n").unwrap();
    git(&repo, &["add", "staged.txt"]);
    fs::write(repo.join("untracked.txt"), "untracked\n").unwrap();
    let commit = output(&repo, &["rev-parse", "--short", "HEAD"]);

    let chat = moved(&mut socket, source_id, "feat/moved").await;
    let worktree = fs::canonicalize(server.root.path()).unwrap().join(format!("data/worktrees/_branches/{project}/feat-moved"));
    assert_ne!(chat["id"], source["id"]);
    assert_ne!(chat["projectId"], project);
    assert_eq!((&chat["cwd"], &chat["rig"], &chat["agent"]), (&json!(worktree), &source["rig"], &source["agent"]));
    assert_eq!(chat["agentSession"], session);
    let draft = chat["draft"].as_str().unwrap();
    assert!(draft.starts_with("FYI: Stampede moved this Chat to a new git worktree."), "{draft}");
    assert!(draft.contains(&format!("`{}`, on the new branch `feat/moved`, cut from `master` at `{commit}`, and the uncommitted changes moved with it", worktree.display())), "{draft}");
    assert!(draft.contains(&format!("under `{}` are now under `{}`", repo.display(), worktree.display())), "{draft}");
    assert_eq!(record(&server, chat["id"].as_str().unwrap())["draft"], draft);
    assert!(!server.root.path().join(format!("data/chats/{source_id}.json")).exists());
    let git_status = reqwest::get(format!("http://127.0.0.1:{}/api/projects/{}/git", server.port, chat["projectId"].as_str().unwrap())).await.unwrap();
    assert_eq!(serde_json::from_str::<Value>(&git_status.text().await.unwrap()).unwrap()["branch"], "feat/moved");

    assert_eq!(output(&repo, &["status", "--porcelain"]), "");
    assert_eq!(output(&worktree, &["status", "--porcelain"]), "A  staged.txt\n M tracked.txt\n?? untracked.txt");
    assert_eq!(output(&repo, &["stash", "list"]), "");
    let running = running_in(&worktree).await;
    match agent {
        "claude-code" => assert!(running.iter().any(|cmdline| cmdline.contains(&format!(" --resume {}", session.as_str().unwrap()))), "{running:?}"),
        "codex" => {
            let resumed = received(&server, "home/.codex/fake-daemon/received.jsonl", "thread/resume");
            assert_eq!((&resumed["params"]["threadId"], &resumed["params"]["cwd"]), (&session, &json!(worktree)));
        }
        _ => assert_eq!(received(&server, "copilot/received.jsonl", "session.resume")["params"]["sessionId"], session),
    }
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn a_clean_checkout_moves_without_touching_older_stashes() {
    let (server, mut socket, repo, project, source) = launch("claude-code", "simple-turn").await;
    fs::write(repo.join("tracked.txt"), "stashed long ago\n").unwrap();
    git(&repo, &["stash", "push", "-q", "-m", "older"]);

    let chat = moved(&mut socket, source["id"].as_str().unwrap(), "clean").await;
    let worktree = fs::canonicalize(server.root.path()).unwrap().join(format!("data/worktrees/_branches/{project}/clean"));
    assert_eq!(chat["cwd"], json!(worktree));
    assert!(!chat["draft"].as_str().unwrap().contains("uncommitted"));
    assert_eq!(output(&worktree, &["status", "--porcelain"]), "");
    assert_eq!(output(&repo, &["stash", "list", "--format=%gs"]), "On master: older");
    close_chat(&mut socket, server.root.path(), chat["id"].as_str().unwrap()).await;
}

#[tokio::test]
async fn a_refused_move_leaves_the_chat_and_its_changes_in_place() {
    let (server, mut socket, repo, project, source) = launch("claude-code", "simple-turn").await;
    let source_id = source["id"].as_str().unwrap();
    fs::write(repo.join("tracked.txt"), "changed\n").unwrap();
    let taken = fs::canonicalize(server.root.path()).unwrap().join(format!("data/worktrees/_branches/{project}/taken"));
    fs::create_dir_all(&taken).unwrap();

    let refused = [
        ("bad..name", "bad..name is not a valid branch name".to_owned()),
        ("taken", format!("{} already exists", taken.display())),
    ];
    for (branch, error) in refused {
        control(&mut socket, json!({ "type": "chat.move", "chatId": source_id, "branch": branch })).await;
        let message = next_control(&mut socket, "chat.move.error").await;
        assert_eq!((&message["chatId"], &message["error"]), (&source["id"], &json!(error)));
    }
    assert_eq!(record(&server, source_id)["cwd"], json!(repo));
    assert_eq!(output(&repo, &["status", "--porcelain"]), " M tracked.txt");
    chat_turn(&mut socket, source_id, &source["status"], "Say hello").await;
    close_chat(&mut socket, server.root.path(), source_id).await;
}

async fn launch(agent: &str, fixture: &str) -> (Server, Socket, PathBuf, String, Value) {
    let config = format!("[[rig]]\nname = 'rig'\nagent = '{agent}'\nenv = {{ STAMPEDE_FAKE_FIXTURE = '{fixture}' }}\n");
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("chat-move", &config, vec![("PATH", path.into())]).await;
    let repo = init(&fs::canonicalize(server.root.path()).unwrap(), "repo");
    git(&repo, &["config", "user.name", "stampede"]);
    git(&repo, &["config", "user.email", "test@stampede.invalid"]);
    fs::write(repo.join("tracked.txt"), "original\n").unwrap();
    git(&repo, &["add", "tracked.txt"]);
    git(&repo, &["commit", "-q", "-m", "Initial commit"]);
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": repo })).await;
    let project = next_control(&mut socket, "project.opened").await["projectId"].as_str().unwrap().to_owned();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project, "rig": "rig" })).await;
    let source = timeout(Duration::from_secs(20), next_control(&mut socket, "chat.created")).await.expect("the Chat never started")["chat"].clone();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": source["id"] })).await;
    next_control(&mut socket, "chat.items").await;
    (server, socket, repo, project, source)
}

async fn moved(socket: &mut Socket, chat_id: &str, branch: &str) -> Value {
    control(socket, json!({ "type": "chat.move", "chatId": chat_id, "branch": branch })).await;
    timeout(Duration::from_secs(20), async {
        loop {
            let message = next(socket).await;
            assert_ne!(message["type"], "chat.move.error", "{message}");
            if message["type"] == "chat.moved" {
                assert_eq!(message["chatId"], chat_id);
                break message["chat"].clone();
            }
        }
    })
    .await
    .expect("the Chat never moved")
}

fn record(server: &Server, chat_id: &str) -> Value {
    serde_json::from_slice(&fs::read(server.root.path().join(format!("data/chats/{chat_id}.json"))).unwrap()).unwrap()
}

fn output(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git").arg("-C").arg(dir).args(args).env("GIT_CONFIG_GLOBAL", "/dev/null").output().unwrap();
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap().trim_end().to_owned()
}

fn received(server: &Server, log: &str, method: &str) -> Value {
    let log = fs::read_to_string(server.root.path().join(log)).unwrap();
    let messages = log.lines().map(|line| serde_json::from_str::<Value>(line).unwrap());
    messages.filter(|message| message["method"] == method).last().unwrap_or_else(|| panic!("no {method} in {log}"))
}

async fn running_in(dir: &Path) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let running: Vec<String> = fs::read_dir("/proc").unwrap().flatten().filter_map(|entry| {
            entry.file_name().to_str()?.parse::<i32>().ok()?;
            (fs::read_link(entry.path().join("cwd")).ok()? == dir).then_some(())?;
            Some(String::from_utf8_lossy(&fs::read(entry.path().join("cmdline")).ok()?).replace('\0', " "))
        }).collect();
        if !running.is_empty() {
            return running;
        }
        assert!(Instant::now() < deadline, "nothing runs in {}", dir.display());
        sleep(Duration::from_millis(50)).await;
    }
}
