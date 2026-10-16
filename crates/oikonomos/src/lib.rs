mod config;
mod device;
mod files;
mod holder;
mod install;
mod projects;
mod protocol;
mod terminals;
mod themes;
mod watcher;

use std::{net::Ipv4Addr, path::PathBuf, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::RustEmbed;
use serde_json::{Value, json};
use tokio::sync::{broadcast, watch};

use config::{Config, Editor, Font, Rig};
pub use holder::{HoldArgs, hold};
pub use install::{InstallArgs, UninstallArgs, install, uninstall};
use projects::Projects;
use terminals::Terminals;
use themes::Theme;

#[derive(RustEmbed)]
#[folder = "../../web/build"]
#[allow_missing = true]
struct Spa;

#[derive(clap::Args)]
pub struct ServeArgs {
    #[arg(long, default_value_t = 4580)]
    pub port: u16,
    #[arg(long, value_name = "DIR", default_value_os_t = home().join(".local/share/oikonomos"))]
    pub data_dir: PathBuf,
    #[arg(long, value_name = "FILE", default_value_os_t = home().join(".config/oikonomos/config.toml"))]
    pub config: PathBuf,
}

pub(crate) fn home() -> PathBuf {
    std::env::home_dir().expect("HOME is not set")
}

#[derive(Clone)]
struct AppState {
    terminals: Arc<Terminals>,
    projects: Arc<Projects>,
    config: watch::Receiver<Arc<Config>>,
    disk: broadcast::Sender<watcher::Changed>,
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/version", get(version))
        .route("/api/theme", get(theme))
        .route("/api/font", get(font))
        .route("/api/rigs", get(rigs))
        .route("/api/editor", get(editor))
        .route("/api/projects/{id}/tree", get(files::tree))
        .route("/api/projects/{id}/file", get(files::read).put(files::write))
        .route("/fonts/host/{index}", get(host_font))
        .route("/ws", get(device::upgrade))
        .fallback(spa)
        .with_state(state)
}

pub async fn serve(args: ServeArgs) -> std::io::Result<()> {
    let config = config::watch(&args.config)?;
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, args.port)).await?;
    let projects = Arc::new(Projects::load(&args.data_dir)?);
    let terminals = Arc::new(Terminals::new(args.data_dir));
    terminals.restore().await?;
    let disk = watcher::spawn(projects.subscribe());
    eprintln!("oikonomos listening on http://{}", listener.local_addr()?);
    axum::serve(listener, router(AppState { terminals, projects, config, disk })).await
}

async fn version() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") }))
}

async fn theme(State(state): State<AppState>) -> Json<Theme> {
    Json(state.config.borrow().theme.clone())
}

async fn font(State(state): State<AppState>) -> Json<Font> {
    Json(state.config.borrow().font.clone())
}

async fn rigs(State(state): State<AppState>) -> Json<Vec<Rig>> {
    Json(state.config.borrow().rigs.clone())
}

async fn editor(State(state): State<AppState>) -> Json<Editor> {
    Json(state.config.borrow().editor.clone())
}

async fn host_font(State(state): State<AppState>, Path(index): Path<usize>) -> Response {
    let path = match state.config.borrow().font.faces.get(index) {
        Some(face) => face.path.clone(),
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    match tokio::fs::read(&path).await {
        Ok(data) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream().to_string();
            ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "no-cache".into())], data)
                .into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn spa(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    match Spa::get(path).or_else(|| Spa::get("index.html")) {
        Some(file) => {
            let mime = file.metadata.mimetype().to_owned();
            ([(header::CONTENT_TYPE, mime)], file.data).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
