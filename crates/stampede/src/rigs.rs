use std::{collections::BTreeMap, io, path::Path, process::Stdio, time::Duration};

use axum::{Json, extract::{Query, State}, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, process::Command};

use crate::{AppState, chats::{Choice, claude, codex, copilot}, config::{self, Agent, Rig}, files::Failure};

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
            Agent::Copilot => copilot::options(&cwd, &rig.env, &settings).await,
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
            Agent::Copilot => (
                model["id"].as_str()?, model["name"].as_str().or(model["id"].as_str())?,
                model["supportedReasoningEfforts"].as_array().into_iter().flatten().filter_map(Value::as_str).collect(),
            ),
        };
        Some(json!({ "value": value, "label": label, "efforts": efforts }))
    }).collect();
    let available = state.config.borrow().available_modes(selection.agent);
    let modes: Vec<Choice> = selection.agent.modes().iter().filter(|mode| available.contains(mode)).map(|mode| Choice::mode(mode)).collect();
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

pub async fn reorder(State(state): State<AppState>, Json(order): Json<Vec<String>>) -> Result<Json<Vec<Rig>>, Failure> {
    let mut configured = state.config.clone();
    let rigs = tokio::task::spawn_blocking(move || config::reorder_rigs(&state.config_path, &order))
        .await.map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    tokio::time::timeout(Duration::from_secs(2), configured.wait_for(|config| config.rigs == rigs))
        .await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Rig order saved, but configuration did not reload".to_owned()))?
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
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

/// Asks a Rig one question with no tools, MCP servers, settings or instruction files, so it reads only the prompt.
/// Codex offers no way to skip its AGENTS.md files.
pub async fn ask(cwd: &Path, rig: &Rig, prompt: &str) -> io::Result<String> {
    let answer = match rig.agent {
        Agent::ClaudeCode => claude::ask(cwd, &rig.env, rig.model.as_deref(), rig.effort.as_deref(), prompt).await,
        Agent::Codex => {
            let mut command = Command::new("codex");
            command.args(["exec", "--ephemeral", "--skip-git-repo-check", "--ignore-user-config", "--ignore-rules", "--sandbox", "read-only"]);
            command.args(rig.model.iter().flat_map(|model| ["--model", model]));
            command.args(rig.effort.iter().flat_map(|effort| ["--config".to_owned(), format!("model_reasoning_effort=\"{effort}\"")]));
            command.arg("-");
            once(command, &rig.env, cwd, prompt).await
        }
        Agent::Copilot => {
            let mut command = Command::new("copilot");
            command.args(["--silent", "--no-custom-instructions", "--available-tools=none", "--disable-builtin-mcps", "--no-ask-user"]);
            command.args(rig.model.iter().flat_map(|model| ["--model", model]));
            command.args(rig.effort.iter().flat_map(|effort| ["--reasoning-effort", effort]));
            once(command, &rig.env, cwd, prompt).await
        }
    }.map_err(|error| io::Error::other(format!("{} failed: {error}", rig.name)))?;
    if answer.is_empty() {
        return Err(io::Error::other(format!("{} ended without an answer.", rig.name)));
    }
    Ok(answer)
}

/// Runs a command that reads one prompt on stdin and prints only its answer.
async fn once(mut command: Command, env: &BTreeMap<String, String>, cwd: &Path, prompt: &str) -> io::Result<String> {
    let mut child = command.envs(env).current_dir(cwd)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .kill_on_drop(true).spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    let write = async move {
        let _ = stdin.write_all(prompt.as_bytes()).await;
    };
    let ((), output) = tokio::join!(write, child.wait_with_output());
    let output = output?;
    let answer = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if output.status.success() {
        return Ok(answer);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(io::Error::other([stderr.trim(), answer.as_str()].into_iter().find(|text| !text.is_empty()).unwrap_or("no output").to_owned()))
}
