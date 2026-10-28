use std::{
    collections::HashSet,
    fs, io,
    path::{Component, Path, PathBuf},
    sync::{Arc, mpsc::{self, RecvTimeoutError}},
    thread,
    time::{Duration, Instant},
};

use axum::{extract::{Path as Param, Query, State}, http::{HeaderName, StatusCode, header}, response::{IntoResponse, Response}};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::{AppState, chats::Item};

const SETTLED: Duration = Duration::from_millis(150);

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename = "canvas.changed", rename_all = "camelCase")]
pub struct Changed {
    pub canvas: String,
    pub versions: usize,
}

pub struct Canvases {
    root: PathBuf,
    history: PathBuf,
    changed: broadcast::Sender<Changed>,
}

pub fn root(data_dir: &Path) -> PathBuf {
    std::path::absolute(data_dir).unwrap_or_else(|_| data_dir.to_owned()).join("canvases")
}

pub fn dir(data_dir: &Path, chat_id: &str) -> PathBuf {
    root(data_dir).join(chat_id)
}

fn valid(part: &str) -> bool {
    !part.starts_with('.') && part.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

fn identify(root: &Path, path: &Path) -> Option<String> {
    let mut parts = path.strip_prefix(root).ok()?.components();
    let (Some(Component::Normal(dir)), Some(Component::Normal(name)), None) = (parts.next(), parts.next(), parts.next()) else { return None };
    let (dir, name) = (dir.to_str()?, name.to_str()?);
    (valid(dir) && valid(name) && name.ends_with(".html")).then(|| format!("{dir}/{name}"))
}

pub fn recognize(data_dir: &Path, cwd: &Path, item: &mut Item) {
    match item {
        Item::Edit { path, added, removed, write, .. } => {
            if let Some(canvas) = identify(&root(data_dir), &cwd.join(&*path)) {
                *item = Item::Canvas { canvas, added: *added, removed: *removed, write: *write };
            }
        }
        Item::Subagent { items, .. } => {
            for item in items { recognize(data_dir, cwd, item); }
        }
        _ => {}
    }
}

fn existing(root: &Path) -> HashSet<String> {
    fs::read_dir(root).into_iter().flatten().flatten()
        .flat_map(|dir| fs::read_dir(dir.path()).into_iter().flatten().flatten())
        .filter_map(|file| identify(root, &file.path()))
        .collect()
}

impl Canvases {
    pub fn watch(data_dir: &Path) -> io::Result<Arc<Self>> {
        let root = root(data_dir);
        fs::create_dir_all(&root)?;
        let (changed, _) = broadcast::channel(64);
        let canvases = Arc::new(Self { history: root.with_file_name("canvas-versions"), root, changed });
        let (events, received) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = events.send(event);
        })
        .map_err(io::Error::other)?;
        watcher.watch(&canvases.root, RecursiveMode::Recursive).map_err(io::Error::other)?;
        let watching = Arc::clone(&canvases);
        thread::spawn(move || {
            let _watcher = watcher;
            let mut pending = existing(&watching.root);
            let mut settled = Instant::now();
            loop {
                let event = if pending.is_empty() { received.recv().map_err(|_| RecvTimeoutError::Disconnected) } else { received.recv_timeout(settled.saturating_duration_since(Instant::now())) };
                match event {
                    Ok(Ok(notify::Event { kind, paths, .. })) if !kind.is_access() => {
                        let changed: Vec<_> = paths.iter().filter_map(|path| identify(&watching.root, path)).collect();
                        if !changed.is_empty() {
                            settled = Instant::now() + SETTLED;
                            pending.extend(changed);
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                    _ => {}
                }
                if pending.is_empty() || Instant::now() < settled { continue }
                for canvas in pending.drain() {
                    match watching.snapshot(&canvas) {
                        Ok(versions) => { let _ = watching.changed.send(Changed { canvas, versions }); }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => eprintln!("cannot keep a version of canvas {canvas}: {error}"),
                    }
                }
            }
        });
        Ok(canvases)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Changed> {
        self.changed.subscribe()
    }

    fn versions(&self, canvas: &str) -> (PathBuf, usize) {
        let directory = self.history.join(canvas);
        let count = fs::read_dir(&directory).map_or(0, |entries| {
            entries.flatten().filter(|entry| entry.path().extension() == Some("html".as_ref())).count()
        });
        (directory, count)
    }

    fn snapshot(&self, canvas: &str) -> io::Result<usize> {
        let content = fs::read(self.root.join(canvas))?;
        let (directory, count) = self.versions(canvas);
        if content.is_empty() || (count > 0 && fs::read(directory.join(format!("{count}.html")))? == content) {
            return Ok(count);
        }
        fs::create_dir_all(&directory)?;
        let temporary = directory.join(format!("{}.partial", count + 1));
        fs::write(&temporary, &content)?;
        fs::rename(temporary, directory.join(format!("{}.html", count + 1)))?;
        Ok(count + 1)
    }
}

#[derive(Deserialize)]
pub struct Version {
    version: Option<usize>,
}

pub async fn read(State(state): State<AppState>, Param((dir, name)): Param<(String, String)>, Query(Version { version }): Query<Version>) -> Response {
    let canvases = &state.canvases;
    let Some(canvas) = identify(&canvases.root, &canvases.root.join(dir).join(name)) else { return StatusCode::NOT_FOUND.into_response() };
    let (directory, versions) = canvases.versions(&canvas);
    let (path, caching) = match version {
        None => (canvases.root.join(&canvas), "no-store"),
        Some(version) if (1..=versions).contains(&version) => (directory.join(format!("{version}.html")), "private, max-age=31536000, immutable"),
        Some(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let Ok(content) = tokio::fs::read(path).await else { return StatusCode::NOT_FOUND.into_response() };
    ([
        (header::CONTENT_TYPE, "text/plain; charset=utf-8".to_owned()),
        (header::CACHE_CONTROL, caching.to_owned()),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        (HeaderName::from_static("x-canvas-versions"), versions.to_string()),
    ], content).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(path: &str) -> Item {
        Item::Edit { path: path.to_owned(), diff: String::new(), added: 3, removed: 1, write: false }
    }

    #[test]
    fn edits_to_canvas_files_become_canvas_items() {
        let data = Path::new("/data");
        let mut item = edit("/data/canvases/0123abcd/pricing.html");
        recognize(data, Path::new("/project"), &mut item);
        assert_eq!(item, Item::Canvas { canvas: "0123abcd/pricing.html".to_owned(), added: 3, removed: 1, write: false });
        let mut nested = edit("data/canvases/0123abcd/pricing.html");
        recognize(data, Path::new("/"), &mut nested);
        assert!(matches!(nested, Item::Canvas { .. }));
        for path in ["/data/canvases/0123abcd/notes.md", "/data/canvases/0123abcd/assets/page.html", "/data/canvases/.hidden/page.html", "/project/page.html", "page.html"] {
            let mut item = edit(path);
            recognize(data, Path::new("/project"), &mut item);
            assert_eq!(item, edit(path), "{path}");
        }
    }
}
