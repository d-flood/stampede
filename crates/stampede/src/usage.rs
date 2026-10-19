use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Window {
    FiveHour,
    Weekly,
    Monthly,
    Session,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Credits,
    Requests,
    Tokens,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Consumption {
    pub amount: f64,
    pub unit: Unit,
}

/// Times are milliseconds since the Unix epoch.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub agent: Agent,
    pub window: Window,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumption: Option<Consumption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starts_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
    pub read_at: u64,
}

impl Reading {
    pub fn new(agent: Agent, window: Window, used: f64, resets_at: Option<u64>) -> Self {
        let read_at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64);
        Self { agent, window, used: Some(used), consumption: None, chat_id: None, session_id: None, partial: false, starts_at: None, resets_at, read_at }
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
            for mut reading in readings {
                match current.iter_mut().find(|old| old.agent == reading.agent && old.window == reading.window && old.session_id == reading.session_id) {
                    Some(old) => {
                        if reading.window == Window::Session && old.consumption.as_ref().is_some_and(|old| old.unit == Unit::Credits && reading.consumption.as_ref().is_none_or(|new| new.unit != Unit::Credits || new.amount < old.amount)) {
                            continue;
                        }
                        reading.chat_id = reading.chat_id.or_else(|| old.chat_id.clone());
                        *old = reading;
                    }
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
            for agent in [Agent::ClaudeCode, Agent::Codex, Agent::Copilot] {
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
