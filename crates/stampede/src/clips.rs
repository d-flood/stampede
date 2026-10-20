use std::time::Duration;

use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;

use crate::{AppState, config::{self, Clip, Error}, files::Failure};

pub async fn list(State(state): State<AppState>) -> Json<Vec<Clip>> {
    Json(state.config.borrow().clips.clone())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    original: Option<String>,
    name: String,
    text: String,
}

pub async fn save(State(state): State<AppState>, Json(save): Json<Save>) -> Result<Json<Vec<Clip>>, Failure> {
    let path = state.config_path.clone();
    apply(state, move || config::write_clip(&path, save.original.as_deref(), Clip { name: save.name, text: save.text })).await
}

pub async fn reorder(State(state): State<AppState>, Json(order): Json<Vec<String>>) -> Result<Json<Vec<Clip>>, Failure> {
    let path = state.config_path.clone();
    apply(state, move || config::reorder_clips(&path, &order)).await
}

pub async fn delete(State(state): State<AppState>, Json(name): Json<String>) -> Result<Json<Vec<Clip>>, Failure> {
    let path = state.config_path.clone();
    apply(state, move || config::delete_clip(&path, &name)).await
}

async fn apply(state: AppState, write: impl FnOnce() -> Result<Vec<Clip>, Error> + Send + 'static) -> Result<Json<Vec<Clip>>, Failure> {
    let mut configured = state.config.clone();
    let clips = tokio::task::spawn_blocking(write)
        .await.map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    tokio::time::timeout(Duration::from_secs(2), configured.wait_for(|config| config.clips == clips))
        .await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Clips saved, but configuration did not reload".to_owned()))?
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(clips))
}
