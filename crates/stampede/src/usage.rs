use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{broadcast::error::RecvError, watch},
    time::{MissedTickBehavior, interval, timeout},
};

use crate::{
    AppState, chats,
    config::{Agent, Config},
    epics::Epics,
    files::Failure,
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
    readings: watch::Sender<Snapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCredits {
    pub available_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_id: Option<String>,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub readings: Vec<Reading>,
    pub codex_resets: Option<ResetCredits>,
}

impl Usage {
    pub fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.readings.subscribe()
    }

    pub fn snapshot(&self) -> Snapshot {
        self.readings.borrow().clone()
    }

    pub fn readings(&self) -> Vec<Reading> {
        self.readings.borrow().readings.clone()
    }

    pub fn record(&self, readings: Vec<Reading>) {
        if readings.is_empty() {
            return;
        }
        self.readings.send_modify(|current| current.record(readings));
    }

    pub fn record_snapshot(&self, agent: Agent, snapshot: Snapshot) {
        self.readings.send_modify(|current| {
            if agent == Agent::Codex {
                current.codex_resets = snapshot.codex_resets;
            }
            current.record(snapshot.readings);
        });
    }
}

impl Snapshot {
    fn record(&mut self, readings: Vec<Reading>) {
        for mut reading in readings {
            match self.readings.iter_mut().find(|old| old.agent == reading.agent && old.window == reading.window && old.session_id == reading.session_id) {
                Some(old) => {
                    if reading.window == Window::Session && old.consumption.as_ref().is_some_and(|old| old.unit == Unit::Credits && reading.consumption.as_ref().is_none_or(|new| new.unit != Unit::Credits || new.amount < old.amount)) {
                        continue;
                    }
                    reading.chat_id = reading.chat_id.or_else(|| old.chat_id.clone());
                    *old = reading;
                }
                None => self.readings.push(reading),
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reset {
    idempotency_key: String,
    credit_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetOutcome {
    Reset,
    AlreadyRedeemed,
    NothingToReset,
    NoCredit,
}

pub async fn reset(State(state): State<AppState>, Json(request): Json<Reset>) -> Result<Json<ResetOutcome>, Failure> {
    if request.idempotency_key.trim().is_empty() || request.credit_id.as_ref().is_some_and(|id| id.trim().is_empty()) {
        return Err((StatusCode::BAD_REQUEST, "Reset identifiers must not be empty.".to_owned()));
    }
    let rig = state.config.borrow().rigs.iter().find(|rig| rig.agent == Agent::Codex).cloned()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "No Codex rig is configured.".to_owned()))?;
    let (outcome, snapshot) = timeout(READING, chats::codex::consume_reset(&rig.env, &request.idempotency_key, request.credit_id.as_deref()))
        .await.map_err(|_| (StatusCode::GATEWAY_TIMEOUT, "Codex took too long. Retry to check this reset.".to_owned()))?
        .map_err(|error| (StatusCode::BAD_GATEWAY, error.to_string()))?;
    state.usage.record_snapshot(Agent::Codex, snapshot);
    Ok(Json(outcome))
}

/// Reads each Agent's windows through its first Rig every few minutes, since TUIs spend them too, but only while a
/// Device is connected or a Run is live, and at once when the first Device connects.
pub fn spawn(usage: Arc<Usage>, config: watch::Receiver<Arc<Config>>, mut devices: watch::Receiver<usize>, epics: Arc<Epics>) {
    tokio::spawn(async move {
        let mut changes = epics.subscribe();
        let mut ticks = interval(REFRESH);
        ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let (mut connected, mut active) = (0, false);
        loop {
            let count = *devices.borrow_and_update();
            let wanted = count > 0 || epics.live();
            if (wanted && !active) || (count > 0 && connected == 0) {
                ticks.reset_immediately();
            }
            (connected, active) = (count, wanted);
            tokio::select! {
                _ = ticks.tick(), if active => read(&usage, &config),
                Ok(()) = devices.changed() => {}
                changed = changes.recv() => if let Err(RecvError::Closed) = changed { break },
            }
        }
    });
}

fn read(usage: &Arc<Usage>, config: &watch::Receiver<Arc<Config>>) {
    let rigs = config.borrow().rigs.clone();
    for agent in [Agent::ClaudeCode, Agent::Codex, Agent::Copilot] {
        let Some(rig) = rigs.iter().find(|rig| rig.agent == agent).cloned() else { continue };
        let usage = Arc::clone(usage);
        tokio::spawn(async move {
            match timeout(READING, chats::usage(&rig)).await.unwrap_or_else(|elapsed| Err(elapsed.into())) {
                Ok(snapshot) => usage.record_snapshot(agent, snapshot),
                Err(error) => eprintln!("cannot read {} usage: {error}", agent.name()),
            }
        });
    }
}
