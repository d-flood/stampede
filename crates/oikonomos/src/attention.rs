use std::collections::HashSet;

use serde::Serialize;
use tokio::sync::{broadcast, watch};

use crate::chats::Status;

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attention {
    pub project_id: String,
    pub chat_id: String,
    pub label: String,
    pub status: Status,
}

pub struct Feed {
    items: watch::Sender<Vec<Attention>>,
    calls: broadcast::Sender<(Attention, HashSet<String>)>,
}

impl Default for Feed {
    fn default() -> Self {
        Self { items: watch::Sender::default(), calls: broadcast::Sender::new(64) }
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

    pub fn publish(&self, items: Vec<Attention>) {
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
