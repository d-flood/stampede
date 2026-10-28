mod attention;
mod browser;
mod canvas;
mod chats;
mod cli;
mod clips;
mod config;
mod device;
mod epics;
mod files;
mod fonts;
mod git;
mod holder;
mod install;
mod language_servers;
mod layouts;
mod media;
mod memory;
mod projects;
mod project_icons;
mod protocol;
mod push;
mod runs;
mod rigs;
mod search;
mod storage;
mod templates;
mod terminals;
mod themes;
mod usage;
mod unix;
mod watcher;

use std::{net::Ipv4Addr, path::PathBuf, sync::Arc, time::UNIX_EPOCH};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    handler::Handler,
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    serve::ListenerExt,
};
use base64ct::{Base64, Encoding};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{broadcast, watch};
use tower_http::compression::{CompressionLayer, DefaultPredicate, Predicate, predicate::NotForContentType};

use attention::Feed;
use chats::Chats;
use config::{Agent, Config, Editor, Font, FontRole, Keybar, Rig};
use epics::{Author, Epics, Event, Pause};
pub use chats::{ChatToolsArgs, HoldChatArgs, hold_chat, chat_tools};
pub use cli::{CliCommand, cli};
pub use holder::{HoldArgs, hold};
pub use install::{InstallArgs, UninstallArgs, install, uninstall};
use language_servers::LanguageServers;
use layouts::Layouts;
use projects::Projects;
use push::Push;
use runs::{Refusal, Runs};
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
    #[arg(long, value_name = "FILE", default_value_os_t = home().join(".config/stampede/config.toml"))]
    pub config: PathBuf,
}

pub(crate) fn home() -> PathBuf {
    std::env::home_dir().expect("HOME is not set")
}

fn data_dir() -> PathBuf {
    home().join(".local/share/stampede")
}

#[derive(Clone)]
struct AppState {
    media_dir: PathBuf,
    canvases: Arc<canvas::Canvases>,
    terminals: Arc<Terminals>,
    chats: Arc<Chats>,
    attention: Arc<Feed>,
    usage: Arc<Usage>,
    projects: Arc<Projects>,
    config: watch::Receiver<Arc<Config>>,
    config_path: PathBuf,
    disk: broadcast::Sender<watcher::Changed>,
    opened: watcher::Opened,
    git: git::Git,
    push: Arc<Push>,
    languages: Arc<LanguageServers>,
    layouts: Arc<Layouts>,
    browsers: Arc<browser::Browsers>,
    epics: Arc<Epics>,
    runs: Arc<Runs>,
    devices: device::Devices,
}

fn router(state: AppState) -> Router {
    let compression = CompressionLayer::new().compress_when(DefaultPredicate::new().and(NotForContentType::const_new("font/woff")));
    Router::new()
        .route("/api/version", get(version))
        .route("/api/browser", post(browser::open))
        .route("/api/host-name", get(host_name).put(set_host_name))
        .route("/api/theme", get(theme).put(set_theme))
        .route("/api/themes", get(theme_names))
        .route("/api/themes/{name}", get(preview_theme))
        .route("/api/font", get(font))
        .route("/api/fonts", get(font_families))
        .route("/api/fonts/{family}", get(preview_font))
        .route("/api/font-choice/{role}", get(chosen_font).put(choose_font))
        .route("/api/rigs", get(rigs).put(rigs::save).delete(rigs::delete))
        .route("/api/rigs/options", get(rigs::options))
        .route("/api/commit-rig", get(commit_rig).put(set_commit_rig))
        .route("/api/commit-instructions", get(commit_instructions).put(set_commit_instructions))
        .route("/api/modes", get(modes).put(set_modes))
        .route("/api/rigs/order", put(rigs::reorder))
        .route("/api/clips", get(clips::list).put(clips::save).delete(clips::delete))
        .route("/api/clips/order", put(clips::reorder))
        .route("/api/usage/codex/reset", post(usage::reset))
        .route("/api/templates", get(templates))
        .route("/api/editor", get(editor))
        .route("/api/leader", get(leader))
        .route("/api/keybar", get(keybar))
        .route("/api/scrollback", get(scrollback))
        .route("/api/config/file", get(files::read_config).put(files::write_config))
        .route("/api/media/{asset}", get(media::read))
        .route("/api/canvases/{dir}/{name}", get(canvas::read))
        .route("/api/project-icons/{asset}", get(project_icons::read))
        .route("/api/project-icons/website", post(project_icons::website))
        .route("/api/push/key", get(push::key))
        .route("/api/push/subscribe", post(push::subscribe).delete(push::unsubscribe))
        .route("/api/devices/{id}/layout", get(layouts::read).put(layouts::write))
        .route("/api/devices/{id}/closing", post(layouts::closing))
        .route("/api/epics/{slug}", post(record_epic))
        .route("/api/epics/{slug}/land", post(land_epic))
        .route("/api/projects/{id}/tree", get(files::tree))
        .route("/api/projects/{id}/icon", put(project_icons::save).delete(project_icons::remove))
        .route("/api/projects/{id}/files", get(files::paths))
        .route("/api/projects/{id}/image", get(files::image))
        .route("/api/projects/{id}/file/copy", post(files::copy))
        .route("/api/projects/{id}/file/rename", post(files::rename))
        .route("/api/projects/{id}/file", get(files::read).head(files::exists).put(files::write).post(files::create).delete(files::delete))
        .route("/api/projects/{id}/worktree", delete(delete_worktree))
        .route("/api/projects/{id}/worktrees", post(create_worktree))
        .route("/api/projects/{id}/git", get(git::status))
        .route("/api/projects/{id}/git/refs", get(git::refs))
        .route("/api/projects/{id}/git/log", get(git::history))
        .route("/api/projects/{id}/git/commit/{sha}", get(git::commit))
        .route("/api/projects/{id}/git/show", get(git::show))
        .route("/api/projects/{id}/git/index", get(git::index))
        .route("/api/projects/{id}/git/commit/{sha}/message", get(git::message))
        .route("/api/projects/{id}/git/stage", post(git::actions::stage))
        .route("/api/projects/{id}/git/unstage", post(git::actions::unstage))
        .route("/api/projects/{id}/git/discard", post(git::actions::discard))
        .route("/api/projects/{id}/git/commit", post(git::actions::commit))
        .route("/api/projects/{id}/git/undo", post(git::actions::undo))
        .route("/api/projects/{id}/git/reword", post(git::actions::reword))
        .route("/api/projects/{id}/git/squash", post(git::actions::squash))
        .route("/api/projects/{id}/git/draft", post(git::actions::draft))
        .route("/api/projects/{id}/git/fetch", post(git::actions::fetch))
        .route("/api/projects/{id}/git/pull", post(git::actions::pull))
        .route("/api/projects/{id}/git/push", post(git::actions::push))
        .route("/api/projects/{id}/git/publish", post(git::actions::publish))
        .route("/api/projects/{id}/git/switch", post(git::actions::switch))
        .route("/api/projects/{id}/git/stashes", get(git::actions::stashes).post(git::actions::stash))
        .route("/api/projects/{id}/git/stashes/{index}/{action}", post(git::actions::unstash))
        .route("/api/lsp/log", get(language_servers::log))
        .route("/fonts/host/{index}", get(host_font).layer(compression.clone()))
        .route("/fonts/system/{family}/{index}", get(system_font).layer(compression.clone()))
        .route("/manifest.webmanifest", get(manifest))
        .route("/ws", get(device::upgrade))
        .fallback(spa.layer(compression))
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
    let terminals = Arc::new(Terminals::new(args.data_dir.clone()));
    terminals.restore().await?;
    chats::listen_delegations(&args.data_dir, Arc::clone(&chats), Arc::clone(&terminals), config.clone())?;
    chats.restore(&rigs).await?;
    memory::trim_periodically();
    let runs = Runs::new(
        &args.data_dir,
        Arc::clone(&epics),
        Arc::clone(&chats),
        Arc::clone(&terminals),
        Arc::clone(&projects),
        config.clone(),
        Arc::clone(&attention),
        Arc::clone(&usage),
    )?;
    cli::listen(requests, Arc::clone(&epics), Arc::clone(&runs));
    let devices = device::Devices::default();
    usage::spawn(Arc::clone(&usage), config.clone(), devices.subscribe(), Arc::clone(&epics));
    terminals.scrollback(config.borrow().scrollback);
    let mut settings = config.clone();
    let configured_terminals = Arc::clone(&terminals);
    tokio::spawn(async move {
        while settings.changed().await.is_ok() {
            configured_terminals.scrollback(settings.borrow_and_update().scrollback);
        }
    });
    let (disk, opened) = watcher::spawn(projects.subscribe());
    let git = git::spawn(projects.subscribe(), &disk, devices.subscribe());
    eprintln!("stampede listening on http://{}", listener.local_addr()?);
    let canvases = canvas::Canvases::watch(&args.data_dir)?;
    let state = AppState { media_dir: args.data_dir.join("media"), canvases, terminals, chats, attention, usage, projects, config, config_path: args.config, disk, opened, git, push, languages, layouts, browsers, epics, runs, devices };
    let listener = listener.tap_io(|stream| {
        let _ = stream.set_nodelay(true);
    });
    axum::serve(listener, router(state)).with_graceful_shutdown(async {
        if std::env::var_os("STAMPEDE_TEST_OWNER").is_none() { std::future::pending::<()>().await; }
        while holder::test_owner_alive() { tokio::time::sleep(std::time::Duration::from_millis(100)).await; }
    }).await
}

async fn version() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION"), "buildId": env!("STAMPEDE_BUILD_ID") }))
}

async fn host_name(State(state): State<AppState>) -> Json<Option<String>> {
    Json(state.config.borrow().name.clone())
}

async fn set_host_name(State(state): State<AppState>, Json(name): Json<Option<String>>) -> Result<Json<Option<String>>, files::Failure> {
    tokio::task::spawn_blocking(move || config::write_host_name(&state.config_path, name.as_deref()))
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map(Json)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))
}

async fn manifest(State(state): State<AppState>) -> Response {
    let name = state.config.borrow().name.clone().unwrap_or_else(|| "Stampede".to_owned());
    let manifest = json!({
        "name": name,
        "short_name": name,
        "start_url": "/",
        "scope": "/",
        "display": "standalone",
        "icons": [
            { "src": "/icons/icon-192.png", "sizes": "192x192", "type": "image/png" },
            { "src": "/icons/icon-512.png", "sizes": "512x512", "type": "image/png" },
            { "src": "/icons/maskable-512.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" }
        ]
    });
    ([(header::CONTENT_TYPE, "application/manifest+json"), (header::CACHE_CONTROL, "no-cache")], manifest.to_string()).into_response()
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

async fn preview_theme(Path(name): Path<String>) -> Result<Json<Theme>, files::Failure> {
    tokio::task::spawn_blocking(move || Theme::resolve(&name))
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
        | Event::TagsSet { .. }
        | Event::RigSet { .. }
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
    let named: Vec<&String> = match &event {
        Event::RunStarted { rig, reviewers, remediator, .. } => [rig].into_iter().chain(reviewers).chain(remediator).collect(),
        Event::RunChanged { rig, reviewers, remediator, .. } => rig.iter().chain(reviewers.iter().flatten()).chain(remediator).collect(),
        Event::RigSet { rig, .. } => rig.iter().collect(),
        _ => Vec::new(),
    };
    if let Some(rig) = named.into_iter().find(|rig| !state.config.borrow().rigs.iter().any(|other| other.name == **rig)) {
        return Err((StatusCode::BAD_REQUEST, format!("there is no Rig {rig} in the config")));
    }
    state.epics.record(&slug, event).map(|()| StatusCode::NO_CONTENT).map_err(|error| (StatusCode::BAD_REQUEST, error))
}

async fn land_epic(State(state): State<AppState>, Path(slug): Path<String>) -> Result<String, files::Failure> {
    state.runs.land(&slug).await.map_err(|refusal| (StatusCode::BAD_REQUEST, refusal))
}

#[derive(Deserialize)]
struct Creation {
    branch: Option<String>,
    from: String,
}

async fn create_worktree(State(state): State<AppState>, Path(id): Path<String>, Json(creation): Json<Creation>) -> Result<Json<Value>, files::Failure> {
    let project_id = state.runs.create_worktree(&id, creation.branch.as_deref(), &creation.from).await.map_err(|error| (StatusCode::BAD_REQUEST, error))?;
    Ok(Json(json!({ "projectId": project_id })))
}

#[derive(Deserialize)]
struct Deletion {
    #[serde(default)]
    force: bool,
}

async fn delete_worktree(State(state): State<AppState>, Path(id): Path<String>, Query(query): Query<Deletion>) -> Result<StatusCode, files::Failure> {
    match state.runs.delete_worktree(&id, query.force).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(Refusal::Unmerged(work)) => Err((StatusCode::CONFLICT, work)),
        Err(Refusal::Refused(reason)) => Err((StatusCode::BAD_REQUEST, reason)),
    }
}

async fn font(State(state): State<AppState>) -> Json<Font> {
    Json(state.config.borrow().font.clone())
}

async fn font_families() -> Result<Json<Vec<String>>, files::Failure> {
    tokio::task::spawn_blocking(fonts::families)
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map(Json)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

async fn preview_font(Path(family): Path<String>) -> Result<Json<Font>, files::Failure> {
    let font = tokio::task::spawn_blocking(move || fonts::resolve(&family))
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    if font.faces.is_empty() {
        return Err((StatusCode::NOT_FOUND, format!("no installed font family {:?}", font.family)));
    }
    Ok(Json(font))
}

async fn chosen_font(State(state): State<AppState>, Path(role): Path<FontRole>) -> Result<Json<Option<Font>>, files::Failure> {
    let family = state.config.borrow().chosen_font(role);
    tokio::task::spawn_blocking(move || family.map(|family| fonts::resolve(&family)).transpose())
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map(Json)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

async fn choose_font(State(state): State<AppState>, Path(role): Path<FontRole>, Json(family): Json<Option<String>>) -> Result<Json<Option<Font>>, files::Failure> {
    tokio::task::spawn_blocking(move || -> Result<_, config::Error> {
        let font = family.as_deref().map(fonts::resolve).transpose()?;
        if let Some(font) = &font
            && font.faces.is_empty()
        {
            return Err(format!("no installed font family {:?}", font.family).into());
        }
        config::write_font(&state.config_path, role, family.as_deref())?;
        Ok(font)
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
    .map(Json)
    .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))
}

async fn rigs(State(state): State<AppState>) -> Json<Vec<Rig>> {
    Json(state.config.borrow().rigs.clone())
}

async fn commit_rig(State(state): State<AppState>) -> Json<Option<String>> {
    Json(state.config.borrow().commit_rig().map(|rig| rig.name.clone()))
}

async fn set_commit_rig(State(state): State<AppState>, Json(name): Json<String>) -> Result<Json<String>, files::Failure> {
    tokio::task::spawn_blocking(move || config::write_commit_rig(&state.config_path, &name))
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map(Json)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))
}

async fn commit_instructions(State(state): State<AppState>) -> Json<String> {
    Json(state.config.borrow().commit_instructions().to_owned())
}

async fn set_commit_instructions(State(state): State<AppState>, Json(text): Json<String>) -> Result<Json<String>, files::Failure> {
    tokio::task::spawn_blocking(move || config::write_commit_instructions(&state.config_path, &text))
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map(Json)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))
}

#[derive(Deserialize)]
struct ChosenModes {
    agent: Agent,
    modes: Vec<String>,
}

async fn modes(State(state): State<AppState>) -> Json<Value> {
    Json(mode_choices(&state.config.borrow()))
}

async fn set_modes(State(state): State<AppState>, Json(chosen): Json<ChosenModes>) -> Result<Json<Value>, files::Failure> {
    tokio::task::spawn_blocking(move || config::write_modes(&state.config_path, chosen.agent, &chosen.modes))
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map(|config| Json(mode_choices(&config)))
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))
}

fn mode_choices(config: &Config) -> Value {
    Value::Object([Agent::ClaudeCode, Agent::Codex, Agent::Copilot].into_iter().map(|agent| {
        let available = config.available_modes(agent);
        let choices = agent.chat_modes().into_iter().map(|mode| {
            json!({ "value": mode, "label": chats::Choice::mode(mode).label, "available": available.contains(&mode) })
        });
        (agent.name().to_owned(), choices.collect())
    }).collect())
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

async fn host_font(State(state): State<AppState>, Path(index): Path<usize>, headers: HeaderMap) -> Response {
    let path = match state.config.borrow().font.faces.get(index) {
        Some(face) => face.path.clone(),
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    font_file(path, &headers).await
}

async fn system_font(Path((family, index)): Path<(String, usize)>, headers: HeaderMap) -> Response {
    match tokio::task::spawn_blocking(move || fonts::file(&family, index)).await {
        Ok(Ok(Some(path))) => font_file(path, &headers).await,
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn font_file(path: PathBuf, headers: &HeaderMap) -> Response {
    let Ok(metadata) = tokio::fs::metadata(&path).await else { return StatusCode::NOT_FOUND.into_response() };
    let modified = metadata.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).unwrap_or_default();
    let etag = format!("W/\"{:x}-{:x}\"", modified.as_nanos(), metadata.len());
    if unchanged(headers, &etag) {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag), (header::CACHE_CONTROL, "no-cache".into())]).into_response();
    }
    match tokio::fs::read(&path).await {
        Ok(data) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream().to_string();
            ([(header::CONTENT_TYPE, mime), (header::ETAG, etag), (header::CACHE_CONTROL, "no-cache".into())], data)
                .into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn spa(uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let found = Spa::get(path).map(|file| (file, if path.starts_with("_app/immutable/") { "max-age=31536000, immutable" } else { "no-cache" }));
    let Some((file, cache)) = found.or_else(|| Spa::get("index.html").map(|file| (file, "no-cache"))) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let etag = format!("W/\"{}\"", Base64::encode_string(&file.metadata.sha256_hash()));
    if unchanged(&headers, &etag) {
        return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag), (header::CACHE_CONTROL, cache.to_owned())]).into_response();
    }
    let mime = file.metadata.mimetype().to_owned();
    ([(header::CONTENT_TYPE, mime), (header::ETAG, etag), (header::CACHE_CONTROL, cache.to_owned())], file.data).into_response()
}

fn unchanged(headers: &HeaderMap, etag: &str) -> bool {
    headers.get(header::IF_NONE_MATCH).is_some_and(|value| value == etag)
}
