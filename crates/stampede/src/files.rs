use std::{
    collections::HashSet,
    fs, io,
    path::{Component, Path, PathBuf},
};

use axum::{
    Json,
    extract::{Path as Param, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{AppState, config};

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

#[derive(Deserialize)]
pub struct Create {
    path: String,
    dir: bool,
}

#[derive(Deserialize)]
pub struct Copy {
    path: String,
    destination: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSave {
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
        let path = resolve(&root, &query.path)?;
        let _ = state.opened.send((id, path.clone()));
        let bytes = fs::read(path).map_err(failure)?;
        Ok(Json(contents(bytes)).into_response())
    })
    .await
}

pub async fn exists(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Query(query): Query<FileQuery>,
) -> Result<Response, Failure> {
    let root = root(&state, &id)?;
    blocking(move || {
        let status = if resolve(&root, &query.path)?.is_file() { StatusCode::OK } else { StatusCode::NOT_FOUND };
        Ok(status.into_response())
    })
    .await
}

pub async fn image(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Query(query): Query<FileQuery>,
) -> Result<Response, Failure> {
    let root = root(&state, &id)?;
    blocking(move || {
        let path = resolve(&root, &query.path)?;
        let mime = mime_guess::from_path(&query.path).first_or_octet_stream();
        if mime.type_() != "image" {
            return Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, "Not an image file".to_owned()));
        }
        let _ = state.opened.send((id, path.clone()));
        let bytes = fs::read(path).map_err(failure)?;
        Ok(([
            (header::CONTENT_TYPE, mime.as_ref()),
            (header::CACHE_CONTROL, "no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::CONTENT_SECURITY_POLICY, "sandbox; default-src 'none'"),
        ], bytes).into_response())
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
        let path = match resolve(&root, &save.path) {
            Ok(path) => path,
            Err((StatusCode::NOT_FOUND, _)) => {
                if root.join(&save.path).is_symlink() {
                    return Err((StatusCode::FORBIDDEN, "Cannot recreate a dangling symlink".to_owned()));
                }
                let relative = Path::new(&save.path);
                let parent = relative.parent().unwrap_or(Path::new(""));
                resolve(&root, &parent.to_string_lossy())?.join(relative.file_name().ok_or_else(|| {
                    (StatusCode::BAD_REQUEST, "No file name".to_owned())
                })?)
            }
            Err(error) => return Err(error),
        };
        let current = match fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(failure(error)),
        };
        let current_hash = current.as_deref().map(hash).unwrap_or_default();
        if current_hash != save.base_hash {
            let current = current.map_or_else(|| json!({ "content": "", "hash": "" }), contents);
            return Ok((StatusCode::CONFLICT, Json(current)).into_response());
        }
        fs::write(&path, &save.content).map_err(failure)?;
        Ok(Json(json!({ "hash": hash(save.content.as_bytes()) })).into_response())
    })
    .await
}

pub async fn create(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Json(create): Json<Create>,
) -> Result<Response, Failure> {
    let root = root(&state, &id)?;
    blocking(move || {
        let relative = Path::new(&create.path);
        if !relative.components().all(|part| matches!(part, Component::Normal(_))) {
            return Err((StatusCode::FORBIDDEN, "Invalid path within the Project".to_owned()));
        }
        let name = relative.file_name().ok_or_else(|| (StatusCode::BAD_REQUEST, "No name".to_owned()))?;
        let parent = relative.parent().unwrap_or(Path::new(""));
        let path = resolve(&root, &parent.to_string_lossy())?.join(name);
        if create.dir {
            fs::create_dir(path).map_err(failure)?;
        } else {
            fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(failure)?;
        }
        Ok(StatusCode::CREATED.into_response())
    })
    .await
}

pub async fn delete(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Query(query): Query<FileQuery>,
) -> Result<Response, Failure> {
    let root = root(&state, &id)?;
    blocking(move || {
        let relative = Path::new(&query.path);
        if !relative.components().all(|part| matches!(part, Component::Normal(_) | Component::CurDir)) {
            return Err((StatusCode::FORBIDDEN, "Path is outside the Project".to_owned()));
        }
        let name = relative.file_name().ok_or_else(|| {
            (StatusCode::BAD_REQUEST, "Cannot delete the Project directory".to_owned())
        })?;
        let parent = relative.parent().unwrap_or(Path::new(""));
        let path = resolve(&root, &parent.to_string_lossy())?.join(name);
        if fs::symlink_metadata(&path).map_err(failure)?.is_dir() {
            fs::remove_dir_all(path).map_err(failure)?;
        } else {
            fs::remove_file(path).map_err(failure)?;
        }
        Ok(StatusCode::NO_CONTENT.into_response())
    })
    .await
}

pub async fn copy(
    State(state): State<AppState>,
    Param(id): Param<String>,
    Json(copy): Json<Copy>,
) -> Result<Response, Failure> {
    let root = root(&state, &id)?;
    blocking(move || {
        let relative = Path::new(&copy.path);
        if !relative.components().all(|part| matches!(part, Component::Normal(_))) {
            return Err((StatusCode::FORBIDDEN, "Invalid path within the Project".to_owned()));
        }
        let name = relative.file_name().ok_or_else(|| (StatusCode::BAD_REQUEST, "No name".to_owned()))?;
        let parent = relative.parent().unwrap_or(Path::new(""));
        let source = resolve(&root, &parent.to_string_lossy())?.join(name);
        let metadata = fs::symlink_metadata(&source).map_err(failure)?;
        let destination = resolve(&root, &copy.destination)?;
        if metadata.is_dir() && destination.starts_with(&source) {
            return Err((StatusCode::BAD_REQUEST, "Cannot copy a directory into itself".to_owned()));
        }
        let name = name.to_string_lossy();
        let (stem, extension) = if metadata.is_dir() {
            (name.as_ref(), String::new())
        } else {
            match name.rsplit_once('.') {
                Some((stem, extension)) if !stem.is_empty() => (stem, format!(".{extension}")),
                _ => (name.as_ref(), String::new()),
            }
        };
        for number in 0.. {
            let name = match number {
                0 => name.to_string(),
                1 => format!("{stem} copy{extension}"),
                _ => format!("{stem} copy {number}{extension}"),
            };
            match copy_entry(&source, &destination.join(&name)) {
                Ok(()) => return Ok((StatusCode::CREATED, Json(json!({ "path": if copy.destination.is_empty() {
                    name
                } else {
                    format!("{}/{name}", copy.destination)
                } }))).into_response()),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(failure(error)),
            }
        }
        unreachable!()
    })
    .await
}

fn copy_entry(source: &Path, destination: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.is_symlink() {
        return std::os::unix::fs::symlink(fs::read_link(source)?, destination);
    }
    if metadata.is_dir() {
        fs::create_dir(destination)?;
        let result = (|| {
            for entry in fs::read_dir(source)? {
                let entry = entry?;
                copy_entry(&entry.path(), &destination.join(entry.file_name()))?;
            }
            fs::set_permissions(destination, metadata.permissions())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(destination);
        }
        return result;
    }
    if !metadata.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Only files, directories, and symlinks can be copied"));
    }
    let mut source = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new().write(true).create_new(true).open(destination)?;
    let result = io::copy(&mut source, &mut output).and_then(|_| output.set_permissions(metadata.permissions()));
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

pub async fn read_config(State(state): State<AppState>) -> Result<Response, Failure> {
    blocking(move || {
        let bytes = config_bytes(&state.config_path)?;
        let error = str::from_utf8(&bytes).ok().and_then(config::check);
        let mut contents = contents(bytes);
        contents["error"] = json!(error);
        Ok(Json(contents).into_response())
    })
    .await
}

pub async fn write_config(
    State(state): State<AppState>,
    Json(save): Json<ConfigSave>,
) -> Result<Response, Failure> {
    blocking(move || {
        let current = config_bytes(&state.config_path)?;
        if hash(&current) != save.base_hash {
            return Ok((StatusCode::CONFLICT, Json(contents(current))).into_response());
        }
        fs::write(&state.config_path, &save.content).map_err(failure)?;
        let error = config::check(&save.content);
        Ok(Json(json!({ "hash": hash(save.content.as_bytes()), "error": error })).into_response())
    })
    .await
}

fn config_bytes(path: &Path) -> Result<Vec<u8>, Failure> {
    match fs::read(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        bytes => bytes.map_err(failure),
    }
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
        io::ErrorKind::AlreadyExists => StatusCode::CONFLICT,
        io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
        io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
        io::ErrorKind::IsADirectory | io::ErrorKind::NotADirectory => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, error.to_string())
}
