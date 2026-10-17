mod common;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    time::Duration,
};

use common::{Cli, Server, control, git, init, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn device(port: u16) -> Socket {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "loop-epics" })).await;
    next_control(&mut socket, "hello").await;
    socket
}

async fn received(socket: &mut Socket, kind: &str) -> Value {
    timeout(Duration::from_secs(10), next_control(socket, kind)).await.unwrap_or_else(|_| panic!("no {kind} arrived"))
}

async fn open(socket: &mut Socket, path: &Path) -> String {
    control(socket, json!({ "type": "project.open", "path": path })).await;
    received(socket, "project.opened").await["projectId"].as_str().unwrap().to_owned()
}

async fn subscribe(socket: &mut Socket, project_id: &str) -> Value {
    control(socket, json!({ "type": "epics.subscribe", "projectId": project_id })).await;
    let listed = received(socket, "epics.changed").await;
    assert_eq!(listed["projectId"], project_id);
    listed["epics"].clone()
}

async fn changed(socket: &mut Socket, projects: usize) -> HashMap<String, Value> {
    let mut changes = HashMap::new();
    while changes.len() < projects {
        let changed = received(socket, "epic.changed").await;
        changes.insert(changed["projectId"].as_str().unwrap().to_owned(), changed["epic"].clone());
    }
    changes
}

fn keys(changes: &HashMap<String, Value>) -> HashSet<&str> {
    changes.keys().map(String::as_str).collect()
}

#[tokio::test]
async fn a_cli_change_reaches_every_device_subscribed_to_the_epics_project() {
    let mut server = Server::start("loop-epics").await;
    let root = server.root.path().to_owned();
    let cli = Cli { data_dir: root.join("data") };
    let repo = init(&root, "repo");
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "Initial commit"]);
    let worktree = root.join("worktree");
    git(&repo, &["worktree", "add", "-q", worktree.to_str().unwrap()]);
    let other = init(&root, "other");
    let plain = root.join("plain");
    fs::create_dir(&plain).unwrap();

    let mut socket = device(server.port).await;
    let repo_id = open(&mut socket, &repo).await;
    let worktree_id = open(&mut socket, &worktree).await;
    let other_id = open(&mut socket, &other).await;
    let plain_id = open(&mut socket, &plain).await;
    for project_id in [&repo_id, &worktree_id, &other_id, &plain_id] {
        assert_eq!(subscribe(&mut socket, project_id).await, json!([]));
    }
    let checkouts = HashSet::from([repo_id.as_str(), worktree_id.as_str()]);

    cli.ok(&repo, &["epic", "add", "loop", "--title", "The loop", "--brief", "-"], "# The loop\n");
    let added = changed(&mut socket, 2).await;
    assert_eq!(keys(&added), checkouts);
    let expected = json!({ "slug": "loop", "title": "The loop", "brief": "# The loop\n", "setup": null, "archived": false, "tickets": [] });
    assert_eq!(added[&repo_id], expected);

    cli.ok(&worktree, &["ticket", "add", "loop", "1", "--title", "The store", "--body", "-"], "Build the store.\n");
    assert_eq!(keys(&changed(&mut socket, 2).await), checkouts);
    cli.ok(&repo, &["ticket", "add", "loop", "2", "--title", "The tab", "--blocked-by", "1", "--body", "-"], "Show it.\n");
    assert_eq!(keys(&changed(&mut socket, 2).await), checkouts);
    cli.ok(&repo, &["note", "loop/2", "Decided on one socket."], "");
    assert_eq!(keys(&changed(&mut socket, 2).await), checkouts);
    cli.ok(&repo, &["epic", "set-setup", "loop", "pnpm install"], "");
    let set = changed(&mut socket, 2).await;
    assert_eq!(keys(&set), checkouts);
    let epic = &set[&worktree_id];
    let at = epic["tickets"][1]["notes"][0]["at"].as_u64().expect("a Note's time in milliseconds");
    let expected = json!({
        "slug": "loop",
        "title": "The loop",
        "brief": "# The loop\n",
        "setup": "pnpm install",
        "archived": false,
        "tickets": [
            { "number": 1, "title": "The store", "body": "Build the store.\n", "blockers": [], "status": "not-started", "notes": [] },
            {
                "number": 2,
                "title": "The tab",
                "body": "Show it.\n",
                "blockers": [1],
                "status": "not-started",
                "notes": [{ "author": "agent", "text": "Decided on one socket.", "at": at }],
            },
        ],
    });
    assert_eq!(*epic, expected);
    assert_eq!(set[&repo_id], expected);

    cli.ok(&other, &["epic", "add", "other", "--title", "Other", "--brief", "-"], "Other brief\n");
    let elsewhere = changed(&mut socket, 1).await;
    assert_eq!(keys(&elsewhere), HashSet::from([other_id.as_str()]));
    assert_eq!(elsewhere[&other_id]["slug"], "other");

    server.restart().await;
    let mut socket = device(server.port).await;
    assert_eq!(open(&mut socket, &worktree).await, worktree_id);
    assert_eq!(subscribe(&mut socket, &worktree_id).await, json!([expected]));
    assert_eq!(subscribe(&mut socket, &plain_id).await, json!([]));
    cli.ok(&repo, &["ticket", "set-body", "loop/1", "-"], "Build the store first.\n");
    let edited = changed(&mut socket, 1).await;
    assert_eq!(edited[&worktree_id]["tickets"][0]["body"], "Build the store first.\n");
}
