use std::{
    collections::HashSet,
    fs, io,
    path::{Component, Path, PathBuf},
};

use axum::{
    Json,
    extract::{Path as Param, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::AppState;

pub type Failure = (StatusCode, String);

#[derive(Deserialize)]
pub struct TreeQuery {
    #[serde(default)]
    path: String,
    #[serde(rename = "includeIgnored")]
    include_ignored: Option<String>,
}

#[derive(Deserialize)]
pub struct FileQuery {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Save {
    path: String,
    content: String,
    base_hash: String,
}

#[derive(Serialize)]
struct Entry {
    name: String,
    path: String,
    dir: bool,
    ignored: bool,
}

pub async fn tree(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Query(query): Query<TreeQuery>,
) -> Result<Response, Failure> {
    let root = root(&state, &id)?;
    let include_ignored = query.include_ignored.as_deref() == Some("1");
    blocking(move || {
        let dir = resolve(&root, &query.path)?;
        let entries = list(&dir, &query.path, include_ignored).map_err(failure)?;
        Ok(Json(entries).into_response())
    })
    .await
}

pub async fn paths(State(state): State<AppState>, Param(id): Param<String>) -> Result<Response, Failure> {
    let root = root(&state, &id)?;
    blocking(move || Ok(Json(walk(&root)).into_response())).await
}

pub async fn read(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Query(query): Query<FileQuery>,
) -> Result<Response, Failure> {
    let root = root(&state, &id)?;
    blocking(move || {
        let bytes = fs::read(resolve(&root, &query.path)?).map_err(failure)?;
        Ok(Json(contents(bytes)).into_response())
    })
    .await
}

pub async fn write(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Json(save): Json<Save>,
) -> Result<Response, Failure> {
    let root = root(&state, &id)?;
    blocking(move || {
        let path = resolve(&root, &save.path)?;
        let current = fs::read(&path).map_err(failure)?;
        if hash(&current) != save.base_hash {
            return Ok((StatusCode::CONFLICT, Json(contents(current))).into_response());
        }
        fs::write(&path, &save.content).map_err(failure)?;
        Ok(Json(json!({ "hash": hash(save.content.as_bytes()) })).into_response())
    })
    .await
}

pub fn root(state: &AppState, id: &str) -> Result<PathBuf, Failure> {
    let projects = state.projects.list();
    match projects.iter().find(|project| project.id == id) {
        Some(project) => Ok(PathBuf::from(&project.path)),
        None => Err((StatusCode::NOT_FOUND, format!("Project {id} is not open"))),
    }
}

async fn blocking(
    work: impl FnOnce() -> Result<Response, Failure> + Send + 'static,
) -> Result<Response, Failure> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
}

fn resolve(root: &Path, rel: &str) -> Result<PathBuf, Failure> {
    let outside = || (StatusCode::FORBIDDEN, format!("{rel} is outside the Project"));
    let relative = |component| matches!(component, Component::Normal(_) | Component::CurDir);
    if !Path::new(rel).components().all(relative) {
        return Err(outside());
    }
    let path = root.join(rel).canonicalize().map_err(failure)?;
    if !path.starts_with(root) {
        return Err(outside());
    }
    Ok(path)
}

fn list(dir: &Path, rel: &str, include_ignored: bool) -> io::Result<Vec<Entry>> {
    let kept: HashSet<_> = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build()
        .skip(1)
        .filter_map(|entry| Some(entry.ok()?.file_name().to_owned()))
        .collect();
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let ignored = !kept.contains(&entry.file_name());
        if ignored && !include_ignored {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else { continue };
        let path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        entries.push(Entry { dir: entry.path().is_dir(), name, path, ignored });
    }
    entries.sort_by_key(|entry| (!entry.dir, entry.name.to_lowercase()));
    Ok(entries)
}

fn walk(root: &Path) -> Vec<String> {
    let mut paths: Vec<_> = WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build()
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|kind| !kind.is_dir()))
        .filter_map(|entry| Some(entry.path().strip_prefix(root).ok()?.to_str()?.to_owned()))
        .collect();
    paths.sort_unstable();
    paths
}

fn contents(bytes: Vec<u8>) -> Value {
    let hash = hash(&bytes);
    match String::from_utf8(bytes) {
        Ok(content) if !content.contains('\0') => json!({ "content": content, "hash": hash }),
        _ => json!({ "binary": true, "hash": hash }),
    }
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn failure(error: io::Error) -> Failure {
    let status = match error.kind() {
        io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
        io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
        io::ErrorKind::IsADirectory | io::ErrorKind::NotADirectory => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, error.to_string())
}
