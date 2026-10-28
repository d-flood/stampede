use std::{fs, io::{self, BufRead, Write}, os::unix::net::UnixStream, path::{Path, PathBuf}, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, net::UnixListener, sync::{broadcast, watch}};

use super::{ChatToolsArgs, Chats, Entry, Item, Notice, Outcome, Phase, RequestKind, Status, Totals};
use crate::config::Config;

pub const LIST: &str = "list_rigs";
pub const TOOL: &str = "delegate_to_rig";
pub const CLAUDE_LIST: &str = "mcp__stampede_chat__list_rigs";
pub const CLAUDE_TOOL: &str = "mcp__stampede_chat__delegate_to_rig";
const MAX_REPORT: usize = 8 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Task {
    rig: String,
    task: String,
}

#[derive(Deserialize)]
struct Request {
    chat_id: Option<String>,
    name: String,
    arguments: Value,
}

#[derive(Serialize, Deserialize)]
struct Report {
    id: String,
    rig: String,
    task: String,
    model: Option<String>,
    started_at: u64,
    outcome: Option<Outcome>,
    report: Option<String>,
    totals: Totals,
}

impl Report {
    fn reply(&self, data_dir: &Path, parent_id: &str) -> io::Result<Value> {
        let directory = data_dir.join("delegations");
        fs::create_dir_all(&directory)?;
        fs::write(directory.join(format!("{}.json", self.id)), serde_json::to_vec(self)?)?;
        let mut value = json!({ "id": self.id, "rig": self.rig, "outcome": self.outcome, "report": self.report });
        if let Some(report) = &self.report && report.len() > MAX_REPORT {
            let directory = crate::canvas::dir(data_dir, parent_id).join("reports");
            fs::create_dir_all(&directory)?;
            let path = directory.join(format!("{}.txt", self.id));
            fs::write(&path, report)?;
            value["report"] = json!(&report[..report.floor_char_boundary(MAX_REPORT)]);
            value["truncated"] = json!(true);
            value["report_path"] = json!(path);
        }
        Ok(json!({ "content": [{ "type": "text", "text": value.to_string() }], "structuredContent": value,
            "isError": self.outcome != Some(Outcome::Completed) }))
    }

    fn item(&self, status: Status, activity: Option<String>, items: Vec<Item>) -> Item {
        Item::Subagent {
            id: self.id.clone(), origin: super::SubagentOrigin::Stampede, name: Some(self.rig.clone()), task: self.task.lines().next().unwrap_or_default().chars().take(200).collect(),
            status, outcome: self.outcome, prompt: Some(self.task.clone()), report: self.report.clone(), model: self.model.clone(),
            started_at: self.started_at, totals: Some(self.totals), activity, items,
        }
    }
}

pub fn item(result: &Value, data_dir: Option<&Path>) -> Option<Item> {
    let value: Value = result.get("structuredContent").filter(|value| value.is_object()).cloned().or_else(|| {
        let content = &result["content"];
        if let Some(text) = content.as_str() { return serde_json::from_str(text).ok(); }
        content.as_array()?.iter().filter(|block| block["type"] == "text")
            .find_map(|block| serde_json::from_str(block["text"].as_str()?).ok())
    })?;
    let report = data_dir.and_then(|data_dir| {
        let id = value["id"].as_str().filter(|id| id.len() == 16 && id.bytes().all(|byte| byte.is_ascii_hexdigit()))?;
        let saved: Report = serde_json::from_slice(&fs::read(data_dir.join("delegations").join(format!("{id}.json"))).ok()?).ok()?;
        (saved.id == id && value["rig"] == saved.rig).then_some(saved)
    }).or_else(|| serde_json::from_value::<Report>(value.clone()).ok()).or_else(|| {
        Some(Report {
            id: value["id"].as_str()?.to_owned(), rig: value["rig"].as_str()?.to_owned(), task: "Delegated task".to_owned(),
            model: None, started_at: 0, outcome: serde_json::from_value(value["outcome"].clone()).ok(),
            report: value["report"].as_str().map(str::to_owned), totals: Totals { duration_ms: 0, tools: 0, tokens: None },
        })
    })?;
    let items = report.report.iter().map(|text| Item::Text { text: text.clone(), streaming: false, phase: Phase::Final }).collect();
    Some(report.item(Status::Done, None, items))
}

pub fn list_tool() -> Value {
    json!({
        "name": LIST,
        "description": "List configured Stampede rigs and their harness/model settings. Use the returned names for delegate_to_rig.",
        "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
        "annotations": { "readOnlyHint": true, "destructiveHint": false, "openWorldHint": false },
    })
}

pub fn tool() -> Value {
    json!({
        "name": TOOL,
        "description": "Delegate to another rig when the user requests it, and wait for its result. Use list_rigs for names. The child shares this working directory and files but starts a fresh conversation with its own model and permissions. Supply a self-contained task, relevant paths and expected output. For review-and-fix requests, request review only, then assess and apply fixes yourself. Treat failed or stopped outcomes as incomplete. Oversized reports return an excerpt and a path to the full report.",
        "inputSchema": {
            "type": "object", "properties": {
                "rig": { "type": "string", "minLength": 1, "description": "Exact configured rig name from list_rigs." },
                "task": { "type": "string", "minLength": 1, "maxLength": 65536, "description": "Self-contained instructions for the delegated agent." },
            }, "required": ["rig", "task"], "additionalProperties": false,
        },
        "annotations": { "readOnlyHint": false, "destructiveHint": false, "openWorldHint": false },
    })
}

fn socket_path(data_dir: &Path) -> PathBuf {
    data_dir.join("delegation.sock")
}

fn error(message: impl ToString) -> Value {
    json!({ "content": [{ "type": "text", "text": message.to_string() }], "isError": true })
}

pub fn call(args: &ChatToolsArgs, params: Value) -> Value {
    let exchange = || -> io::Result<Value> {
        let address = crate::unix::path(&socket_path(&args.data_dir))?;
        let mut stream = UnixStream::connect(&address)?;
        let request = json!({ "chat_id": args.chat_id, "name": params["name"], "arguments": params["arguments"] });
        writeln!(stream, "{request}")?;
        let mut reply = String::new();
        std::io::BufReader::new(stream).read_line(&mut reply)?;
        serde_json::from_str(&reply).map_err(io::Error::other)
    };
    exchange().unwrap_or_else(|cause| error(format!("Could not reach Stampede chat tools: {cause}")))
}

pub fn listen(data_dir: &Path, chats: Arc<Chats>, terminals: Arc<crate::terminals::Terminals>, config: watch::Receiver<Arc<Config>>) -> io::Result<()> {
    let path = socket_path(data_dir);
    match fs::remove_file(&path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let address = crate::unix::path(&path)?;
    let listener = UnixListener::bind(&address)?;
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let (chats, terminals, config) = (Arc::clone(&chats), Arc::clone(&terminals), config.clone());
                    tokio::spawn(async move {
                        let (reader, mut writer) = stream.into_split();
                        let mut reader = BufReader::new(reader);
                        let mut line = String::new();
                        if (&mut reader).take(1024 * 1024).read_line(&mut line).await.is_err() { return; }
                        let result = match serde_json::from_str::<Request>(&line) {
                            Ok(request) if request.name == LIST => {
                                let rigs: Vec<_> = config.borrow().rigs.iter().map(|rig| json!({
                                    "name": rig.name, "agent": rig.agent, "model": rig.model, "effort": rig.effort,
                                })).collect();
                                json!({ "content": [{ "type": "text", "text": serde_json::to_string(&rigs).unwrap() }], "isError": false })
                            }
                            Ok(request) if request.name == super::questions::TOOL => {
                                super::questions::ask(&chats, request.chat_id, request.arguments, &mut reader).await
                            }
                            Ok(request) if matches!(request.name.as_str(), super::terminal_tools::STAGE | super::terminal_tools::READ) => {
                                let scrollback = config.borrow().scrollback;
                                super::terminal_tools::call(&chats, &terminals, request.chat_id, &request.name, request.arguments, scrollback).await
                            }
                            Ok(request) if request.name == TOOL => match serde_json::from_value::<Task>(request.arguments) {
                                Ok(task) if !task.rig.trim().is_empty() && !task.task.trim().is_empty() && task.task.len() <= 65536 => {
                                    let rig = config.borrow().rigs.iter().find(|rig| rig.name == task.rig).cloned();
                                    match (request.chat_id, rig) {
                                        (Some(parent), Some(rig)) => {
                                            match chats.delegate(&parent, &rig, task.task, &mut reader).await {
                                                Ok(report) => report.reply(&chats.data_dir, &parent).unwrap_or_else(|cause| error(format!("Could not save delegation report: {cause}"))),
                                                Err(cause) => error(cause),
                                            }
                                        }
                                        (None, _) => error("Delegation requires a calling Stampede chat."),
                                        (_, None) => error("That rig is no longer configured. Use list_rigs to choose a rig."),
                                    }
                                }
                                _ => error("Provide only a rig name and a nonempty task up to 64 KiB."),
                            },
                            _ => error("Unknown delegation request."),
                        };
                        let _ = writer.write_all(format!("{result}\n").as_bytes()).await;
                    });
                }
                Err(cause) => eprintln!("cannot accept a delegation: {cause}"),
            }
        }
    });
    Ok(())
}

impl Chats {
    async fn delegate(self: &Arc<Self>, parent_id: &str, rig: &crate::config::Rig, task: String, reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>) -> io::Result<Report> {
        let parent = self.get(parent_id).ok_or_else(|| io::Error::other("The calling chat is no longer open."))?;
        let source = parent.chat.lock().unwrap().clone();
        if source.parent.is_some() {
            return Err(io::Error::other("Return your result to the calling agent instead of delegating again."));
        }
        let mut changes = self.subscribe();
        let child = self.start(&source.project_id, &source.cwd, rig, None, None, Some(source.id.clone()), None).await?;
        let entry = self.get(&child.id).ok_or_else(|| io::Error::other("The delegated agent did not start."))?;
        let mut report = Report { id: child.id.clone(), rig: child.rig.clone(), task, model: child.settings.chosen.model.clone(),
            started_at: super::now(), outcome: None, report: None, totals: Totals { duration_ms: 0, tools: 0, tokens: None } };
        self.send(&child.id, &report.task, false, &[]);
        let mut disconnected = [0];
        loop {
            let (status, activity, items, finished) = {
                let transcript = entry.transcript.lock().unwrap();
                let chat = entry.chat.lock().unwrap();
                report.rig = chat.rig.clone();
                report.model = chat.settings.chosen.model.clone();
                report.totals.duration_ms = super::now().saturating_sub(report.started_at);
                report.totals.tokens = chat.tokens.map(|tokens| tokens.input + tokens.output);
                report.totals.tools = transcript.items.iter().map(|item| match item {
                    Item::Tools { calls } => calls.len(), Item::Command { .. } | Item::Edit { .. } | Item::Canvas { .. } => 1, _ => 0,
                }).sum();
                let begun = transcript.items.iter().any(|item| matches!(item, Item::User { .. } | Item::Notice { notice: Notice::Error, .. }));
                (chat.status, chat.activity.clone(), transcript.items.clone(), begun && transcript.reported == Status::Done)
            };
            self.delegation_requests(&parent, &entry, &child.id);
            if finished {
                report.outcome = Some(if items.iter().any(|item| matches!(item, Item::Notice { notice: Notice::Interrupted, .. })) { Outcome::Stopped }
                    else if items.iter().any(|item| matches!(item, Item::Notice { notice: Notice::Error | Notice::Limit, .. })) { Outcome::Failed }
                    else { Outcome::Completed });
                report.report = Some(items.iter().filter_map(|item| match item {
                    Item::Text { text, streaming: false, phase: Phase::Final } => Some(text.as_str()),
                    Item::Notice { notice: Notice::Error | Notice::Limit | Notice::Interrupted, text } => Some(text.as_str()), _ => None,
                }).collect::<Vec<_>>().join("\n\n"));
                if report.report.as_ref().is_none_or(|text| text.trim().is_empty()) {
                    report.outcome = Some(Outcome::Failed);
                    report.report = Some("The delegated agent ended without a final response.".to_owned());
                }
                parent.driver.delegated(report.item(Status::Done, None, items));
                break;
            }
            parent.driver.delegated(report.item(if status == Status::Done { Status::Working } else { status }, activity, items));
            let stopped = tokio::select! {
                stopped = async {
                    loop {
                        let changed = changes.recv().await;
                        if self.get(&child.id).is_none() || self.get(parent_id).is_none() { break true; }
                        match changed {
                            Ok(super::Change::Status(id, Status::Done)) if id == source.id && source.status != Status::Done => break true,
                            Ok(super::Change::Status(id, _) | super::Change::Activity(id, _) | super::Change::Settings(id, _)) if id == child.id => break false,
                            Ok(super::Change::Item { chat_id, .. } | super::Change::Request { chat_id, .. }
                                | super::Change::Resolved { chat_id, .. } | super::Change::Meters { chat_id, .. }) if chat_id == child.id => break false,
                            Err(broadcast::error::RecvError::Closed) => break true,
                            Err(broadcast::error::RecvError::Lagged(_)) => break false,
                            _ => {},
                        }
                    }
                } => stopped,
                _ = reader.read(&mut disconnected) => true,
            };
            if stopped {
                report.outcome = Some(Outcome::Stopped);
                report.report = Some("Delegation stopped before the agent finished.".to_owned());
                let items = entry.transcript.lock().unwrap().items.clone();
                parent.driver.delegated(report.item(Status::Done, None, items));
                break;
            }
        }
        self.clear_delegation_requests(&parent, &child.id);
        self.close(&child.id);
        Ok(report)
    }

    fn delegation_requests(&self, parent: &Entry, child: &Entry, child_id: &str) {
        let requests = child.transcript.lock().unwrap().requests.clone();
        let mut transcript = parent.transcript.lock().unwrap();
        let parent_id = parent.chat.lock().unwrap().id.clone();
        let prefix = format!("{child_id}/");
        let stale: Vec<_> = transcript.requests.iter().filter(|request| request.id().starts_with(&prefix)
            && !requests.iter().any(|child| request.id() == format!("{prefix}{}", child.id()))).map(|request| request.id().to_owned()).collect();
        for id in stale { self.resolve(&mut transcript, &parent_id, &id); }
        for mut request in requests {
            let id = match &mut request.kind {
                RequestKind::Command { id, .. } | RequestKind::Edit { id, .. } | RequestKind::Other { id, .. }
                | RequestKind::Question { id, .. } | RequestKind::Plan { id, .. } => id,
            };
            *id = format!("{prefix}{id}");
            request.subagent.get_or_insert_with(|| child_id.to_owned());
            if transcript.requests.iter().any(|pending| *pending == request) { continue; }
            transcript.requests.push(request.clone());
            transcript.seq += 1;
            let _ = self.changed.send(super::Change::Request { chat_id: parent_id.clone(), seq: transcript.seq, request });
        }
        self.settle(parent, &transcript);
    }

    fn clear_delegation_requests(&self, parent: &Entry, child_id: &str) {
        let mut transcript = parent.transcript.lock().unwrap();
        let parent_id = parent.chat.lock().unwrap().id.clone();
        let prefix = format!("{child_id}/");
        let requests: Vec<_> = transcript.requests.iter().filter(|request| request.id().starts_with(&prefix)).map(|request| request.id().to_owned()).collect();
        for id in requests { self.resolve(&mut transcript, &parent_id, &id); }
        self.settle(parent, &transcript);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_results_preserve_full_reports_and_legacy_replay() {
        let data = tempfile::tempdir().unwrap();
        let mut report = Report {
            id: "0123456789abcdef".to_owned(), rig: "Reviewer".to_owned(), task: "Review this task".to_owned(),
            model: Some("model".to_owned()), started_at: 42, outcome: Some(Outcome::Completed),
            report: Some("界".repeat(MAX_REPORT)), totals: Totals { duration_ms: 1000, tools: 3, tokens: Some(100) },
        };
        for outcome in [Outcome::Completed, Outcome::Failed, Outcome::Stopped] {
            report.outcome = Some(outcome);
            let result = report.reply(data.path(), "parent").unwrap();
            assert_eq!(result["isError"], outcome != Outcome::Completed);
            let reply = &result["structuredContent"];
            for field in ["task", "model", "started_at", "totals"] { assert!(reply.get(field).is_none()); }
            assert_eq!(reply["truncated"], true);
            assert!(reply["report"].as_str().unwrap().len() <= MAX_REPORT);
            assert_eq!(fs::read_to_string(reply["report_path"].as_str().unwrap()).unwrap(), report.report.as_deref().unwrap());
            let expected = report.item(Status::Done, None, vec![Item::Text { text: report.report.clone().unwrap(), streaming: false, phase: Phase::Final }]);
            assert_eq!(item(&json!({ "content": serde_json::to_string(&report).unwrap() }), None), Some(expected.clone()));
            for encoded in [result.clone(), json!({ "content": result["content"] }), json!({ "content": [{ "type": "text", "text": serde_json::to_string(&report).unwrap() }] })] {
                assert_eq!(item(&encoded, Some(data.path())), Some(expected.clone()));
            }
            assert!(item(&result, None).is_some());
        }
    }
}
