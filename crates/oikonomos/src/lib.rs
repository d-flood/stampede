mod attention;
mod browser;
mod chats;
mod cli;
mod config;
mod device;
mod epics;
mod files;
mod git;
mod holder;
mod install;
mod language_servers;
mod layouts;
mod projects;
mod protocol;
mod push;
mod runs;
mod search;
mod templates;
mod terminals;
mod themes;
mod usage;
mod watcher;

use std::{net::Ipv4Addr, path::PathBuf, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rust_embed::RustEmbed;
use serde_json::{Value, json};
use tokio::sync::{broadcast, watch};

use attention::Feed;
use chats::Chats;
use config::{Config, Editor, Font, Keybar, Rig};
use epics::{Author, Epics, Event, Pause};
pub use chats::{HoldChatArgs, hold_chat, chat_tools};
pub use cli::{CliCommand, cli};
pub use holder::{HoldArgs, hold};
pub use install::{InstallArgs, UninstallArgs, install, uninstall};
use language_servers::LanguageServers;
use layouts::Layouts;
use projects::Projects;
use push::Push;
use runs::Runs;
use terminals::Terminals;
use themes::Theme;
use usage::Usage;

#[derive(RustEmbed)]
#[folder = "../../web/build"]
#[allow_missing = true]
struct Spa;

#[derive(clap::Args)]
pub struct ServeArgs {
    #[arg(long, default_value_t = 4580)]
    pub port: u16,
    #[arg(long, value_name = "DIR", default_value_os_t = data_dir())]
    pub data_dir: PathBuf,
    #[arg(long, value_name = "FILE", default_value_os_t = home().join(".config/oikonomos/config.toml"))]
    pub config: PathBuf,
}

pub(crate) fn home() -> PathBuf {
    std::env::home_dir().expect("HOME is not set")
}

fn data_dir() -> PathBuf {
    home().join(".local/share/oikonomos")
}

#[derive(Clone)]
struct AppState {
    terminals: Arc<Terminals>,
    chats: Arc<Chats>,
    attention: Arc<Feed>,
    usage: Arc<Usage>,
    projects: Arc<Projects>,
    config: watch::Receiver<Arc<Config>>,
    config_path: PathBuf,
    disk: broadcast::Sender<watcher::Changed>,
    opened: watcher::Opened,
    git: broadcast::Sender<git::Changed>,
    push: Arc<Push>,
    languages: Arc<LanguageServers>,
    layouts: Arc<Layouts>,
    browsers: Arc<browser::Browsers>,
    epics: Arc<Epics>,
    runs: Arc<Runs>,
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/version", get(version))
        .route("/api/browser", post(browser::open))
        .route("/api/theme", get(theme).put(set_theme))
        .route("/api/themes", get(theme_names))
        .route("/api/font", get(font))
        .route("/api/rigs", get(rigs))
        .route("/api/templates", get(templates))
        .route("/api/editor", get(editor))
        .route("/api/leader", get(leader))
        .route("/api/keybar", get(keybar))
        .route("/api/scrollback", get(scrollback))
        .route("/api/config/file", get(files::read_config).put(files::write_config))
        .route("/api/push/key", get(push::key))
        .route("/api/push/subscribe", post(push::subscribe))
        .route("/api/devices/{id}/layout", get(layouts::read).put(layouts::write))
        .route("/api/devices/{id}/closing", post(layouts::closing))
        .route("/api/epics/{slug}", post(record_epic))
        .route("/api/epics/{slug}/land", post(land_epic))
        .route("/api/projects/{id}/tree", get(files::tree))
        .route("/api/projects/{id}/files", get(files::paths))
        .route("/api/projects/{id}/file", get(files::read).head(files::exists).put(files::write))
        .route("/api/projects/{id}/git", get(git::status))
        .route("/api/projects/{id}/git/head", get(git::head))
        .route("/api/projects/{id}/git/log", get(git::history))
        .route("/api/projects/{id}/git/commit/{sha}", get(git::commit))
        .route("/api/projects/{id}/git/show", get(git::show))
        .route("/api/lsp/log", get(language_servers::log))
        .route("/fonts/host/{index}", get(host_font))
        .route("/ws", get(device::upgrade))
        .fallback(spa)
        .with_state(state)
}

pub async fn serve(args: ServeArgs) -> std::io::Result<()> {
    let config = config::watch(&args.config)?;
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, args.port)).await?;
    let epics = Arc::new(Epics::load(&args.data_dir)?);
    let requests = cli::bind(&args.data_dir)?;
    let projects = Arc::new(Projects::load(&args.data_dir)?);
    let push = Arc::new(Push::load(&args.data_dir)?);
    let languages = Arc::new(LanguageServers::load(&args.data_dir)?);
    let layouts = Arc::new(Layouts::load(&args.data_dir)?);
    let browsers = Arc::new(browser::Browsers::new(args.port));
    let attention = Arc::new(Feed::default());
    push::spawn(Arc::clone(&push), &attention);
    let usage = Arc::new(Usage::default());
    let chats = Arc::new(Chats::new(args.data_dir.clone(), Arc::clone(&attention), Arc::clone(&usage)));
    let rigs = config.borrow().rigs.clone();
    chats.restore(&rigs).await?;
    let terminals = Arc::new(Terminals::new(args.data_dir.clone()));
    terminals.restore().await?;
    let runs = Runs::new(
        &args.data_dir,
        Arc::clone(&epics),
        Arc::clone(&chats),
        Arc::clone(&terminals),
        Arc::clone(&projects),
        config.clone(),
        Arc::clone(&attention),
    )?;
    cli::listen(requests, Arc::clone(&epics), Arc::clone(&runs));
    usage::spawn(Arc::clone(&usage), config.clone());
    terminals.scrollback(config.borrow().scrollback);
    let mut settings = config.clone();
    let configured_terminals = Arc::clone(&terminals);
    tokio::spawn(async move {
        while settings.changed().await.is_ok() {
            configured_terminals.scrollback(settings.borrow_and_update().scrollback);
        }
    });
    let (disk, opened) = watcher::spawn(projects.subscribe());
    let git = git::spawn(projects.subscribe(), &disk);
    eprintln!("oikonomos listening on http://{}", listener.local_addr()?);
    let state = AppState { terminals, chats, attention, usage, projects, config, config_path: args.config, disk, opened, git, push, languages, layouts, browsers, epics, runs };
    axum::serve(listener, router(state)).await
}

async fn version() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION"), "buildId": env!("OIKONOMOS_BUILD_ID") }))
}

async fn theme(State(state): State<AppState>) -> Json<Theme> {
    Json(state.config.borrow().theme.clone())
}

async fn theme_names() -> Result<Json<Vec<String>>, files::Failure> {
    tokio::task::spawn_blocking(themes::names)
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map(Json)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

async fn set_theme(State(state): State<AppState>, Json(name): Json<String>) -> Result<Json<Theme>, files::Failure> {
    tokio::task::spawn_blocking(move || config::write_theme(&state.config_path, &name))
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map(Json)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))
}

async fn record_epic(State(state): State<AppState>, Path(slug): Path<String>, Json(event): Json<Event>) -> Result<StatusCode, files::Failure> {
    let editable = match &event {
        Event::NoteAdded { author, .. } => *author == Author::David,
        Event::BriefSet { .. }
        | Event::SetupSet { .. }
        | Event::TicketAdded { .. }
        | Event::BodySet { .. }
        | Event::BlockersSet { .. }
        | Event::Reopened { .. }
        | Event::Archived
        | Event::RunStarted { .. }
        | Event::RunChanged { .. }
        | Event::RunPaused { pause: Pause::David }
        | Event::RunResumed
        | Event::RunStopped => true,
        _ => false,
    };
    if !editable {
        return Err((StatusCode::FORBIDDEN, "a Device records only David's edits; Epics come from the CLI".to_owned()));
    }
    if let Event::RunStarted { rig, .. } | Event::RunChanged { rig: Some(rig), .. } = &event
        && !state.config.borrow().rigs.iter().any(|other| other.name == *rig)
    {
        return Err((StatusCode::BAD_REQUEST, format!("there is no Rig {rig} in the config")));
    }
    state.epics.record(&slug, event).map(|()| StatusCode::NO_CONTENT).map_err(|error| (StatusCode::BAD_REQUEST, error))
}

async fn land_epic(State(state): State<AppState>, Path(slug): Path<String>) -> Result<String, files::Failure> {
    state.runs.land(&slug).await.map_err(|refusal| (StatusCode::BAD_REQUEST, refusal))
}

async fn font(State(state): State<AppState>) -> Json<Font> {
    Json(state.config.borrow().font.clone())
}

async fn rigs(State(state): State<AppState>) -> Json<Vec<Rig>> {
    Json(state.config.borrow().rigs.clone())
}

async fn templates() -> Json<&'static [templates::Template]> {
    Json(templates::TEMPLATES)
}

async fn editor(State(state): State<AppState>) -> Json<Editor> {
    Json(state.config.borrow().editor.clone())
}

async fn leader(State(state): State<AppState>) -> Json<String> {
    Json(state.config.borrow().leader.clone())
}

async fn keybar(State(state): State<AppState>) -> Json<Keybar> {
    Json(state.config.borrow().keybar.clone())
}

async fn scrollback(State(state): State<AppState>) -> Json<usize> {
    Json(state.config.borrow().scrollback)
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
