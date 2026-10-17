use std::{collections::HashSet, sync::Mutex};

use serde::Serialize;
use tokio::sync::{broadcast, watch};

use crate::chats::Status;

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attention {
    pub project_id: String,
    #[serde(flatten)]
    pub tab: Tab,
    pub label: String,
    pub status: Status,
}

#[derive(Clone, PartialEq, Serialize)]
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

#[derive(Clone, Copy)]
pub enum Source {
    Chats,
    Epics,
}

pub struct Feed {
    sources: Mutex<[Vec<Attention>; 2]>,
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
        sources[source as usize] = items;
        let items = sources.concat();
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
