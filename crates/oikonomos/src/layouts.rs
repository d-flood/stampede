use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use axum::{
    Json,
    body::Bytes,
    extract::{Path as Param, State},
    http::StatusCode,
};
use serde_json::Value;
use uuid::Uuid;

use crate::AppState;

const KEEP: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const CLOSED: &str = "closed";

pub struct Layouts {
    dir: PathBuf,
}

impl Layouts {
    pub fn load(data_dir: &Path) -> io::Result<Self> {
        let dir = data_dir.join("devices");
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    async fn get(&self, name: &str) -> Option<Value> {
        serde_json::from_slice(&tokio::fs::read(self.file(name)).await.ok()?).ok()
    }

    async fn set(&self, name: &str, layout: &[u8]) -> Result<(), StatusCode> {
        let file = self.file(name);
        let tmp = self.dir.join(format!("{name}.{}.tmp", Uuid::new_v4()));
        let saved = match tokio::fs::write(&tmp, layout).await {
            Ok(()) => tokio::fs::rename(&tmp, &file).await,
            error => error,
        };
        saved.map_err(|error| {
            eprintln!("cannot save {}: {error}", file.display());
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }

    async fn prune(&self) {
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else { return };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let modified = entry.metadata().await.and_then(|metadata| metadata.modified());
            let old = modified.is_ok_and(|time| SystemTime::now().duration_since(time).unwrap_or_default() > KEEP);
            if old && let Err(error) = tokio::fs::remove_file(entry.path()).await {
                eprintln!("cannot prune {}: {error}", entry.path().display());
            }
        }
    }
}

pub fn spawn(layouts: Arc<Layouts>) {
    tokio::spawn(async move {
        let mut daily = tokio::time::interval(Duration::from_secs(24 * 60 * 60));
        loop {
            daily.tick().await;
            layouts.prune().await;
        }
    });
}

fn device(id: &str) -> Result<String, StatusCode> {
    Uuid::try_parse(id).map(|id| id.to_string()).map_err(|_| StatusCode::BAD_REQUEST)
}

fn layout(body: &Bytes) -> Result<(), StatusCode> {
    serde_json::from_slice::<Value>(body).map(drop).map_err(|_| StatusCode::BAD_REQUEST)
}

pub async fn read(State(state): State<AppState>, Param(id): Param<String>) -> Result<Json<Option<Value>>, StatusCode> {
    let layouts = &state.layouts;
    let own = layouts.get(&device(&id)?).await;
    Ok(Json(match own {
        Some(layout) => Some(layout),
        None => layouts.get(CLOSED).await,
    }))
}

pub async fn write(State(state): State<AppState>, Param(id): Param<String>, body: Bytes) -> Result<StatusCode, StatusCode> {
    let id = device(&id)?;
    layout(&body)?;
    state.layouts.set(&id, &body).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn closing(State(state): State<AppState>, Param(id): Param<String>, body: Bytes) -> Result<StatusCode, StatusCode> {
    let id = device(&id)?;
    layout(&body)?;
    state.layouts.set(&id, &body).await?;
    state.layouts.set(CLOSED, &body).await?;
    Ok(StatusCode::NO_CONTENT)
}
