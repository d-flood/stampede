mod common;

use std::{env, time::Duration};

use common::{Server, control, next, next_control};
use serde_json::{Value, json};
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

const CLAUDE_PROMPT: &str = "Think it over briefly first. Then, in order: read notes.txt and todo.txt, search the web for \"IANA example domain\", run `wc -l notes.txt` with Bash, run `wc -l missing.txt` with Bash (it will fail), and read notes.txt once more. Finally answer in one short sentence.";
const CLAUDE_ANSWER: &str = "All steps are done: notes.txt has 3 lines and \"needle\" is on line 2, todo.txt also mentions the needle, `wc` failed on missing.txt because the file doesn't exist, and IANA keeps example.com/.org/.net for documentation.\n\nSources: [IANA Reserved Domains](https://res-dom.iana.org), [.example – Wikipedia](https://en.wikipedia.org/wiki/.example)";
const CODEX_PROMPT: &str = "Think it over briefly first. Then, in order: run `wc -l notes.txt`, run `wc -l missing.txt` (it will fail), and search the web for \"IANA example domain\". Finally answer in one short sentence.";
const CODEX_ANSWER: &str = "`notes.txt` has 3 lines, `missing.txt` was absent, and the search found [IANA’s Example Domains page](https://www.iana.org/help/example-domains).";
const DONE: &str = "Say done in one word.";

#[derive(Default)]
struct View {
    items: Vec<Value>,
    statuses: Vec<Value>,
}

#[tokio::test]
async fn claude_tools_thinking_and_commands_normalize_alike_live_and_from_the_session_file() {
    let (mut server, mut socket, chat_id) = launch("claude-code", "tools").await;
    let mut view = View::default();
    for prompt in [CLAUDE_PROMPT, "/compact"] {
        view.turn(&mut socket, &chat_id, prompt).await;
    }
    let search = json!({ "query": "select:WebSearch", "max_results": 1 }).to_string();
    let expected = [
        user(CLAUDE_PROMPT),
        json!({ "kind": "thinking", "seconds": 2 }),
        json!({ "kind": "tools", "calls": [
            { "kind": "read", "name": "Read", "detail": "notes.txt" },
            { "kind": "read", "name": "Read", "detail": "todo.txt" },
            { "kind": "other", "name": "ToolSearch", "detail": search },
            { "kind": "web", "name": "WebSearch", "detail": "IANA example domain" },
        ] }),
        json!({ "kind": "command", "command": "wc -l notes.txt", "status": "ok", "output": "3 notes.txt" }),
        json!({
            "kind": "command",
            "command": "wc -l missing.txt",
            "status": "failed",
            "output": "Exit code 1\nwc: missing.txt: No such file or directory",
        }),
        json!({ "kind": "tools", "calls": [{ "kind": "read", "name": "Read", "detail": "notes.txt" }] }),
        json!({ "kind": "thinking", "seconds": 2 }),
        text(CLAUDE_ANSWER),
        user("/compact"),
        json!({ "kind": "notice", "notice": "compacted", "text": "Conversation compacted" }),
        user(DONE),
        text("Done."),
    ];
    assert_eq!(view.items, expected[..10]);

    server.restart().await;
    let (mut socket, mut view) = reattach(&server, &chat_id, 10).await;
    assert_eq!(view.items, expected[..10], "a compaction replayed by the holder");
    view.turn(&mut socket, &chat_id, DONE).await;
    assert_eq!(view.items, expected);

    server.restart().await;
    let (_, view) = reattach(&server, &chat_id, expected.len()).await;
    assert_eq!(view.items, expected, "a compaction read from the session file");
}

#[tokio::test]
async fn codex_tools_reasoning_and_commands_normalize_to_blocks_live_and_from_the_thread() {
    let (mut server, mut socket, chat_id) = launch("codex", "tools").await;
    let mut view = View::default();
    for prompt in [CODEX_PROMPT, DONE] {
        view.turn(&mut socket, &chat_id, prompt).await;
    }
    let mut expected = [
        user(CODEX_PROMPT),
        json!({ "kind": "thinking", "seconds": 3 }),
        text("I’ll run both counts in order, then search the web.\n"),
        json!({ "kind": "command", "command": "wc -l notes.txt", "status": "ok", "output": "3 notes.txt\n" }),
        json!({
            "kind": "command",
            "command": "wc -l missing.txt",
            "status": "failed",
            "output": "wc: missing.txt: No such file or directory\n",
        }),
        json!({ "kind": "tools", "calls": [{ "kind": "web", "name": "webSearch", "detail": "IANA example domain" }] }),
        text(CODEX_ANSWER),
        json!({ "kind": "notice", "notice": "compacted", "text": "Conversation compacted" }),
        user(DONE),
        text("Done"),
    ];
    assert_eq!(view.items, expected);

    server.restart().await;
    let (_, view) = reattach(&server, &chat_id, expected.len()).await;
    expected[1] = json!({ "kind": "thinking", "seconds": 0 });
    assert_eq!(view.items, expected, "thread/resume carries no item times");
}

#[tokio::test]
async fn api_errors_become_error_notices() {
    for (agent, error) in [
        ("claude-code", "There's an issue with the selected model (claude-nonexistent-9). It may not exist or you may not have access to it."),
        ("codex", "The 'gpt-nonexistent-9' model is not supported when using Codex with a ChatGPT account."),
    ] {
        let (_server, mut socket, chat_id) = launch(agent, "error").await;
        let mut view = View::default();
        view.turn(&mut socket, &chat_id, "Say hi.").await;
        assert_eq!(view.items, [user("Say hi."), json!({ "kind": "notice", "notice": "error", "text": error })], "{agent}");
    }
}

fn user(text: &str) -> Value {
    json!({ "kind": "user", "text": text, "images": 0 })
}

fn text(text: &str) -> Value {
    json!({ "kind": "text", "text": text, "streaming": false })
}

async fn launch(agent: &str, fixture: &str) -> (Server, Socket, String) {
    let config = format!(
        r#"
[[rig]]
name = "rig"
agent = "{agent}"
command = "{agent}"
env = {{ OIKONOMOS_FAKE_FIXTURE = "{fixture}" }}
"#
    );
    let path = format!("{}/tests/fake:{}", env!("CARGO_MANIFEST_DIR"), env::var("PATH").unwrap());
    let server = Server::with_env("chat-transcript", &config, vec![("PATH", path.into())]).await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "project.open", "path": "~" })).await;
    let project_id = next_control(&mut socket, "project.opened").await["projectId"].clone();
    control(&mut socket, json!({ "type": "chat.create", "projectId": project_id, "rig": "rig" })).await;
    let created = timeout(Duration::from_secs(10), next_control(&mut socket, "chat.created"));
    let chat_id = created.await.expect("the chat was never created")["chat"]["id"].as_str().unwrap().to_owned();
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    assert_eq!(next_control(&mut socket, "chat.items").await["items"], json!([]));
    (server, socket, chat_id)
}

async fn reattach(server: &Server, chat_id: &str, len: usize) -> (Socket, View) {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "hello", "deviceId": "device" })).await;
    next_control(&mut socket, "hello").await;
    control(&mut socket, json!({ "type": "chat.attach", "chatId": chat_id })).await;
    let items = next_control(&mut socket, "chat.items").await["items"].as_array().unwrap().clone();
    let mut view = View { items, statuses: Vec::new() };
    view.follow(&mut socket, chat_id, |view| view.items.len() >= len).await;
    (socket, view)
}

impl View {
    async fn turn(&mut self, socket: &mut Socket, chat_id: &str, text: &str) {
        control(socket, json!({ "type": "chat.send", "chatId": chat_id, "text": text })).await;
        let ended = self.statuses.len() + 2;
        self.follow(socket, chat_id, |view| view.statuses.len() == ended).await;
        assert_eq!(self.statuses[ended - 2..], ["working", "done"]);
    }

    async fn follow(&mut self, socket: &mut Socket, chat_id: &str, until: impl Fn(&Self) -> bool) {
        timeout(Duration::from_secs(20), async {
            while !until(self) {
                let message = next(socket).await;
                if message["chatId"] != chat_id {
                    continue;
                }
                match message["type"].as_str().unwrap() {
                    "chat.status" => self.statuses.push(message["status"].clone()),
                    "chat.item" => {
                        let index = message["index"].as_u64().unwrap() as usize;
                        assert!(index <= self.items.len(), "{message}");
                        match self.items.get_mut(index) {
                            Some(item) => *item = message["item"].clone(),
                            None => self.items.push(message["item"].clone()),
                        }
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out with {:?} {:?}", self.statuses, self.items));
    }
}
