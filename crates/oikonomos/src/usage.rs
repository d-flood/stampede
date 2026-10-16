use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tokio::{
    sync::watch,
    time::{MissedTickBehavior, interval, timeout},
};

use crate::{
    chats,
    config::{Agent, Config},
};

const REFRESH: Duration = Duration::from_secs(5 * 60);
const READING: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Window {
    FiveHour,
    Weekly,
}

/// Times are milliseconds since the Unix epoch.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub agent: Agent,
    pub window: Window,
    pub used: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
    pub read_at: u64,
}

impl Reading {
    pub fn new(agent: Agent, window: Window, used: f64, resets_at: Option<u64>) -> Self {
        let read_at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64);
        Self { agent, window, used, resets_at, read_at }
    }
}

/// The latest reading of each Agent's Usage windows.
#[derive(Default)]
pub struct Usage {
    readings: watch::Sender<Vec<Reading>>,
}

impl Usage {
    pub fn subscribe(&self) -> watch::Receiver<Vec<Reading>> {
        self.readings.subscribe()
    }

    pub fn readings(&self) -> Vec<Reading> {
        self.readings.borrow().clone()
    }

    pub fn record(&self, readings: Vec<Reading>) {
        if readings.is_empty() {
            return;
        }
        self.readings.send_modify(|current| {
            for reading in readings {
                match current.iter_mut().find(|old| old.agent == reading.agent && old.window == reading.window) {
                    Some(old) => *old = reading,
                    None => current.push(reading),
                }
            }
        });
    }
}

/// Reads each Agent's windows through its first Rig now and every few minutes, since TUIs spend them too.
pub fn spawn(usage: Arc<Usage>, config: watch::Receiver<Arc<Config>>) {
    tokio::spawn(async move {
        let mut ticks = interval(REFRESH);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticks.tick().await;
            let rigs = config.borrow().rigs.clone();
            for agent in [Agent::ClaudeCode, Agent::Codex] {
                let Some(rig) = rigs.iter().find(|rig| rig.agent == agent).cloned() else { continue };
                let usage = Arc::clone(&usage);
                tokio::spawn(async move {
                    match timeout(READING, chats::usage(&rig)).await.unwrap_or_else(|elapsed| Err(elapsed.into())) {
                        Ok(readings) => usage.record(readings),
                        Err(error) => eprintln!("cannot read {} usage: {error}", agent.name()),
                    }
                });
            }
        }
    });
}
