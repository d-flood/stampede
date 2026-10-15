mod device;
mod holder;
mod protocol;
mod terminals;

use std::{net::Ipv4Addr, path::PathBuf, sync::Arc};

use axum::{
    Json, Router,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::RustEmbed;
use serde_json::{Value, json};

pub use holder::{HoldArgs, hold};
use terminals::Terminals;

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

pub fn router(terminals: Arc<Terminals>) -> Router {
    Router::new()
        .route("/api/version", get(version))
        .route("/ws", get(device::upgrade))
        .fallback(spa)
        .with_state(terminals)
}

pub async fn serve(args: ServeArgs) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, args.port)).await?;
    let terminals = Arc::new(Terminals::new(args.data_dir));
    terminals.restore().await?;
    eprintln!("oikonomos listening on http://{}", listener.local_addr()?);
    axum::serve(listener, router(terminals)).await
}

async fn version() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") }))
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
