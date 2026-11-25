use std::{collections::{BTreeMap, HashSet}, sync::Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};

use crate::chats::Status;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attention {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    pub project_id: String,
    #[serde(flatten)]
    pub tab: Tab,
    pub label: String,
    pub status: Status,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Tab {
    Chat {
        #[serde(rename = "chatId")]
        chat_id: String,
    },
    Epic {
        epic: String,
    },
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    Chats,
    Epics,
    Cloud(String),
}

pub struct Feed {
    sources: Mutex<BTreeMap<Source, Vec<Attention>>>,
    items: watch::Sender<Vec<Attention>>,
    calls: broadcast::Sender<(Attention, HashSet<String>)>,
}

impl Default for Feed {
    fn default() -> Self {
        Self { sources: Mutex::default(), items: watch::Sender::default(), calls: broadcast::Sender::new(64) }
    }
}

impl Feed {
    pub fn subscribe(&self) -> watch::Receiver<Vec<Attention>> {
        self.items.subscribe()
    }

    pub fn items(&self) -> Vec<Attention> {
        self.items.borrow().clone()
    }

    pub fn calls(&self) -> broadcast::Receiver<(Attention, HashSet<String>)> {
        self.calls.subscribe()
    }

    pub fn publish(&self, source: Source, items: Vec<Attention>) {
        let mut sources = self.sources.lock().unwrap();
        sources.insert(source, items);
        let items = sources.values().flatten().cloned().collect();
        self.items.send_if_modified(|current| {
            let changed = *current != items;
            *current = items;
            changed
        });
    }

    pub fn call(&self, item: Attention, except: HashSet<String>) {
        let _ = self.calls.send((item, except));
    }
}
