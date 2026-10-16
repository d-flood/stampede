mod claude;
mod holder;
mod protocol;

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc::UnboundedReceiver};
use uuid::Uuid;

use crate::{
    attention::{Attention, Feed, Source, Tab},
    config::{Agent, Rig},
    hooks::Status,
    projects::Project,
};
pub use holder::{HoldChatArgs, hold_chat};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Item {
    User { text: String, images: usize },
    Text { text: String, streaming: bool },
}

pub enum Event {
    Session(String),
    Status(Status),
    /// Appends the Item when its index is the transcript's length, and replaces the one there otherwise.
    Item(usize, Item),
}

/// One Agent's wire protocol. A driver's events end when its Agent has stopped.
pub trait Driver: Send + Sync {
    fn send(&self, text: &str);
    fn close(&self);
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: String,
    pub project_id: String,
    pub rig: String,
    pub agent: Agent,
    pub cwd: PathBuf,
    pub created: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_session: Option<String>,
    pub status: Status,
    pub viewed_since_done: bool,
}

#[derive(Clone)]
pub enum Change {
    Chats(String),
    Status(String, Status),
    Item { chat_id: String, seq: u64, index: usize, item: Item },
}

pub struct Chats {
    data_dir: PathBuf,
    chats: Mutex<Vec<Arc<Entry>>>,
    changed: broadcast::Sender<Change>,
    views: Mutex<HashMap<Uuid, (Option<String>, Vec<String>)>>,
    attention: Arc<Feed>,
}

struct Entry {
    chat: Mutex<Chat>,
    transcript: Mutex<Transcript>,
    driver: Box<dyn Driver>,
}

#[derive(Default)]
struct Transcript {
    seq: u64,
    items: Vec<Item>,
}

impl Chats {
    pub fn new(data_dir: PathBuf, attention: Arc<Feed>) -> Self {
        Self {
            data_dir,
            chats: Mutex::default(),
            changed: broadcast::Sender::new(1024),
            views: Mutex::default(),
            attention,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Change> {
        self.changed.subscribe()
    }

    pub fn list(&self) -> Vec<Chat> {
        self.chats.lock().unwrap().iter().map(|entry| entry.chat.lock().unwrap().clone()).collect()
    }

    pub fn owned(&self, project_id: &str) -> Vec<Chat> {
        self.list().into_iter().filter(|chat| chat.project_id == project_id).collect()
    }

    pub async fn create(self: &Arc<Self>, project: &Project, rig: &Rig) -> io::Result<Chat> {
        let chat = Chat {
            id: format!("{:016x}", Uuid::new_v4().as_u64_pair().1),
            project_id: project.id.clone(),
            rig: rig.name.clone(),
            agent: rig.agent,
            cwd: PathBuf::from(&project.path),
            created: SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos(),
            agent_session: None,
            status: Status::Done,
            viewed_since_done: true,
        };
        fs::create_dir_all(self.data_dir.join("chats"))?;
        write_record(&self.data_dir, &chat)?;
        let started = match rig.agent {
            Agent::ClaudeCode => claude::start(&self.data_dir, &chat.id, &chat.cwd, &rig.env)
                .await
                .map(|(driver, events)| (Box::new(driver) as Box<dyn Driver>, events)),
            Agent::Codex => Err(io::Error::new(io::ErrorKind::Unsupported, "Codex Chats are not supported yet")),
        };
        let (driver, events) = started.inspect_err(|_| {
            let _ = fs::remove_file(record_path(&self.data_dir, &chat.id));
        })?;
        let entry = Arc::new(Entry { chat: Mutex::new(chat.clone()), transcript: Mutex::default(), driver });
        self.chats.lock().unwrap().push(Arc::clone(&entry));
        let _ = self.changed.send(Change::Chats(chat.project_id.clone()));
        tokio::spawn(Arc::clone(self).follow(entry, events));
        Ok(chat)
    }

    async fn follow(self: Arc<Self>, entry: Arc<Entry>, mut events: UnboundedReceiver<Event>) {
        let (id, project_id) = {
            let chat = entry.chat.lock().unwrap();
            (chat.id.clone(), chat.project_id.clone())
        };
        while let Some(event) = events.recv().await {
            match event {
                Event::Session(session) => {
                    let mut chat = entry.chat.lock().unwrap();
                    chat.agent_session = Some(session);
                    self.save(&chat);
                }
                Event::Status(status) => {
                    let mut chat = entry.chat.lock().unwrap();
                    if chat.status == status {
                        continue;
                    }
                    chat.status = status;
                    if status == Status::Done {
                        chat.viewed_since_done = self.shown(&id);
                    }
                    self.save(&chat);
                    let item = chat.attention();
                    drop(chat);
                    let _ = self.changed.send(Change::Status(id.clone(), status));
                    self.refresh_attention();
                    if status != Status::Working {
                        self.attention.call(item, self.showing(&id));
                    }
                }
                Event::Item(index, item) => {
                    let mut transcript = entry.transcript.lock().unwrap();
                    match transcript.items.get_mut(index) {
                        Some(existing) => *existing = item.clone(),
                        None => transcript.items.push(item.clone()),
                    }
                    transcript.seq += 1;
                    let seq = transcript.seq;
                    let _ = self.changed.send(Change::Item { chat_id: id.clone(), seq, index, item });
                }
            }
        }
        self.chats.lock().unwrap().retain(|other| !Arc::ptr_eq(other, &entry));
        let _ = fs::remove_file(record_path(&self.data_dir, &id));
        let _ = self.changed.send(Change::Chats(project_id));
        self.refresh_attention();
    }

    fn save(&self, chat: &Chat) {
        if let Err(error) = write_record(&self.data_dir, chat) {
            eprintln!("cannot save chat {}: {error}", chat.id);
        }
    }

    /// The transcript so far, and the sequence number of the last change it includes.
    pub fn attach(&self, id: &str) -> Option<(u64, Vec<Item>)> {
        let transcript = self.get(id)?;
        let transcript = transcript.transcript.lock().unwrap();
        Some((transcript.seq, transcript.items.clone()))
    }

    pub fn send(&self, id: &str, text: &str) {
        if let Some(entry) = self.get(id) {
            entry.driver.send(text);
        }
    }

    pub fn close(&self, id: &str) {
        if let Some(entry) = self.get(id) {
            entry.driver.close();
        }
    }

    pub fn view(&self, connection: Uuid, browser: Option<String>, chat_ids: Vec<String>) {
        let entries: Vec<_> = chat_ids.iter().filter_map(|id| self.get(id)).collect();
        if chat_ids.is_empty() {
            self.views.lock().unwrap().remove(&connection);
        } else {
            self.views.lock().unwrap().insert(connection, (browser, chat_ids));
        }
        let mut viewed = false;
        for entry in entries {
            let mut chat = entry.chat.lock().unwrap();
            if chat.status == Status::Done && !chat.viewed_since_done {
                chat.viewed_since_done = true;
                self.save(&chat);
                viewed = true;
            }
        }
        if viewed {
            self.refresh_attention();
        }
    }

    fn shown(&self, id: &str) -> bool {
        self.views.lock().unwrap().values().any(|(_, shown)| shown.iter().any(|shown| shown == id))
    }

    fn showing(&self, id: &str) -> HashSet<String> {
        let views = self.views.lock().unwrap();
        let showing = views.values().filter(|(_, shown)| shown.iter().any(|shown| shown == id));
        showing.filter_map(|(browser, _)| browser.clone()).collect()
    }

    fn refresh_attention(&self) {
        let items = self.list().iter().filter(|chat| chat.needs_attention()).map(Chat::attention).collect();
        self.attention.publish(Source::Chats, items);
    }

    fn get(&self, id: &str) -> Option<Arc<Entry>> {
        let chats = self.chats.lock().unwrap();
        chats.iter().find(|entry| entry.chat.lock().unwrap().id == id).cloned()
    }
}

impl Chat {
    fn needs_attention(&self) -> bool {
        match self.status {
            Status::Waiting => true,
            Status::Done => !self.viewed_since_done,
            Status::Working => false,
        }
    }

    fn attention(&self) -> Attention {
        Attention {
            project_id: self.project_id.clone(),
            tab: Tab::Chat { chat_id: self.id.clone() },
            label: self.rig.clone(),
            status: self.status,
        }
    }
}

fn record_path(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join("chats").join(format!("{id}.json"))
}

fn write_record(data_dir: &Path, chat: &Chat) -> io::Result<()> {
    let path = record_path(data_dir, &chat.id);
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec(chat)?)?;
    fs::rename(tmp, path)
}
