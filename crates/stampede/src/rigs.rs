use std::time::Duration;

use axum::{Json, extract::{Query, State}, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AppState, chats::{Choice, claude, codex}, config::{self, Agent, Rig}, files::Failure};

#[derive(Deserialize)]
pub struct Selection {
    agent: Agent,
    rig: Option<String>,
}

pub async fn options(State(state): State<AppState>, Query(selection): Query<Selection>) -> Result<Json<Value>, Failure> {
    let rig = {
        let config = state.config.borrow();
        selection.rig.as_ref().and_then(|name| config.rigs.iter().find(|rig| rig.name == *name && rig.agent == selection.agent))
            .cloned().unwrap_or_else(|| config.rig_for(selection.agent))
    };
    let cwd = crate::home();
    let settings = rig.settings();
    let (listed, settings) = tokio::time::timeout(Duration::from_secs(15), async {
        match selection.agent {
            Agent::ClaudeCode => claude::options(&cwd, &rig.env, &settings).await,
            Agent::Codex => codex::options(&cwd, &rig.env, &settings).await,
        }
    }).await.map_err(|_| (StatusCode::GATEWAY_TIMEOUT, "Provider took too long to list models. Try again.".to_owned()))?
        .map_err(|error| (StatusCode::BAD_GATEWAY, error.to_string()))?;
    let models: Vec<Value> = listed.iter().filter_map(|model| {
        let (value, label, efforts) = match selection.agent {
            Agent::ClaudeCode => (
                model["value"].as_str()?, model["displayName"].as_str().or(model["value"].as_str())?,
                model["supportedEffortLevels"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>(),
            ),
            Agent::Codex => (
                model["model"].as_str()?, model["displayName"].as_str().or(model["model"].as_str())?,
                model["supportedReasoningEfforts"].as_array().into_iter().flatten().filter_map(|effort| effort["reasoningEffort"].as_str()).collect(),
            ),
        };
        Some(json!({ "value": value, "label": label, "efforts": efforts }))
    }).collect();
    let modes: Vec<Choice> = selection.agent.modes().iter().map(|mode| Choice::mode(mode)).collect();
    Ok(Json(json!({ "models": models, "modes": modes, "settings": settings })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    original: Option<String>,
    name: String,
    agent: Agent,
    model: Option<String>,
    effort: Option<String>,
    mode: String,
}

pub async fn save(State(state): State<AppState>, Json(mut save): Json<Save>) -> Result<Json<Vec<Rig>>, Failure> {
    let mut configured = state.config.clone();
    save.name = save.name.trim().to_owned();
    let original = save.original.clone();
    let name = save.name.clone();
    let rigs = tokio::task::spawn_blocking(move || {
        let rig = Rig { name: save.name, agent: save.agent, env: Default::default(), shortcut: None, model: save.model, effort: save.effort, mode: Some(save.mode) };
        config::write_rig(&state.config_path, save.original.as_deref(), rig)
    }).await.map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    tokio::time::timeout(Duration::from_secs(2), configured.wait_for(|config| config.rigs == rigs))
        .await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Rig saved, but configuration did not reload".to_owned()))?
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    if let Some(original) = original.filter(|original| *original != name) {
        state.chats.rename_rig(&original, &name)
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        state.epics.rename_rig(&original, &name)
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
    }
    Ok(Json(rigs))
}

pub async fn delete(State(state): State<AppState>, Json(name): Json<String>) -> Result<Json<Vec<Rig>>, Failure> {
    let mut configured = state.config.clone();
    let rigs = tokio::task::spawn_blocking(move || config::delete_rig(&state.config_path, &name))
        .await.map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    tokio::time::timeout(Duration::from_secs(2), configured.wait_for(|config| config.rigs == rigs))
        .await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Rig deleted, but configuration did not reload".to_owned()))?
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(rigs))
}
