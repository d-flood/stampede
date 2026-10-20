use std::{collections::HashSet, time::Duration};

use axum::{body::Bytes, extract::{Path, State}, http::{StatusCode, header}, response::{IntoResponse, Response}, Json};
use futures_util::StreamExt;
use lol_html::{element, rewrite_str, RewriteStrSettings};
use reqwest::Url;
use serde::Deserialize;

use crate::{AppState, files::Failure};

const LIMIT: usize = 2 * 1024 * 1024;

pub async fn read(State(state): State<AppState>, Path(asset): Path<String>) -> Response {
    let Some(hash) = asset.strip_suffix(".png") else { return StatusCode::NOT_FOUND.into_response() };
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match tokio::fs::read(state.projects.icon_dir().join(asset)).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "private, max-age=31536000, immutable"), (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn save(State(state): State<AppState>, Path(id): Path<String>, bytes: Bytes) -> Result<StatusCode, Failure> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.len() > LIMIT {
        return Err((StatusCode::BAD_REQUEST, "Choose a valid image up to 2 MiB.".to_owned()));
    }
    update(state, id, Some(bytes)).await
}

pub async fn remove(State(state): State<AppState>, Path(id): Path<String>) -> Result<StatusCode, Failure> {
    update(state, id, None).await
}

async fn update(state: AppState, id: String, bytes: Option<Bytes>) -> Result<StatusCode, Failure> {
    tokio::task::spawn_blocking(move || state.projects.set_icon(&id, bytes.as_deref())).await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .map_err(|error| (if error.kind() == std::io::ErrorKind::NotFound { StatusCode::NOT_FOUND } else { StatusCode::INTERNAL_SERVER_ERROR }, error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Website { url: String }

pub async fn website(Json(input): Json<Website>) -> Result<Response, Failure> {
    tokio::time::timeout(Duration::from_secs(25), discover(&input.url)).await
        .map_err(|_| (StatusCode::GATEWAY_TIMEOUT, "Finding the website icon timed out.".to_owned()))?
        .map_err(|error| (StatusCode::BAD_REQUEST, error))
}

fn url(value: &str) -> Result<Url, String> {
    let value = value.trim();
    let parsed = Url::parse(&if value.contains("://") { value.to_owned() } else { format!("https://{value}") })
        .map_err(|_| "Enter a website URL.".to_owned())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() || !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("Enter an HTTP or HTTPS website URL without credentials.".to_owned());
    }
    Ok(parsed)
}

fn candidates(html: &str, page: &Url) -> Result<Vec<Url>, String> {
    let mut base = None;
    let mut links = Vec::new();
    rewrite_str(html, RewriteStrSettings::new()
            .append_element_content_handler(element!("base[href]", |element| {
                if base.is_none() { base = element.get_attribute("href"); }
                Ok(())
            }))
            .append_element_content_handler(element!("link[href][rel]", |element| {
                let rel = element.get_attribute("rel").unwrap_or_default().to_ascii_lowercase();
                let tokens: Vec<_> = rel.split_ascii_whitespace().collect();
                let rank = if tokens.contains(&"icon") { 0 } else if tokens.contains(&"apple-touch-icon") { 1 } else { return Ok(()) };
                links.push((rank, element.get_attribute("href").unwrap_or_default()));
                Ok(())
            }))
    ).map_err(|error| error.to_string())?;
    let base = base.and_then(|base| page.join(&base).ok()).unwrap_or_else(|| page.clone());
    links.sort_by_key(|(rank, _)| *rank);
    let mut seen = HashSet::new();
    Ok(links.into_iter().filter_map(|(_, link)| base.join(&link).ok())
        .chain(page.join("favicon.ico").ok()).chain(page.join("/favicon.ico").ok())
        .filter(|url| matches!(url.scheme(), "http" | "https") && url.username().is_empty() && url.password().is_none() && seen.insert(url.clone()))
        .take(12).collect())
}

async fn fetch(client: &reqwest::Client, url: Url) -> Result<(Url, String, Vec<u8>), String> {
    let response = client.get(url).send().await.map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| error.to_string())?;
    if response.content_length().is_some_and(|length| length > LIMIT as u64) { return Err("Website response is too large.".to_owned()); }
    let url = response.url().clone();
    let mime = response.headers().get(header::CONTENT_TYPE).and_then(|value| value.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").trim().to_owned();
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        if bytes.len() + chunk.len() > LIMIT { return Err("Website response is too large.".to_owned()); }
        bytes.extend_from_slice(&chunk);
    }
    Ok((url, mime, bytes))
}

fn image_type(mime: &str, bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { Some("image/png") }
    else if bytes.starts_with(b"\xff\xd8\xff") { Some("image/jpeg") }
    else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") { Some("image/gif") }
    else if bytes.starts_with(b"\0\0\x01\0") { Some("image/x-icon") }
    else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") { Some("image/webp") }
    else if mime == "image/svg+xml" || str::from_utf8(bytes).is_ok_and(|text| text.trim_start().starts_with("<svg") || (text.trim_start().starts_with("<?xml") && text.contains("<svg"))) { Some("image/svg+xml") }
    else { None }
}

async fn discover(value: &str) -> Result<Response, String> {
    let page = url(value)?;
    let client = reqwest::Client::builder().timeout(Duration::from_secs(6))
        .redirect(reqwest::redirect::Policy::limited(5)).user_agent("Stampede").build().map_err(|error| error.to_string())?;
    let (page, mime, bytes) = fetch(&client, page).await?;
    if let Some(mime) = image_type(&mime, &bytes) { return Ok(image_response(mime, bytes)); }
    let html = String::from_utf8_lossy(&bytes);
    for candidate in candidates(&html, &page)? {
        if let Ok((_, mime, bytes)) = fetch(&client, candidate).await
            && let Some(mime) = image_type(&mime, &bytes) {
            return Ok(image_response(mime, bytes));
        }
    }
    Err("No website icon found. Choose an image instead.".to_owned())
}

fn image_response(mime: &str, bytes: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "no-store"), (header::CONTENT_SECURITY_POLICY, "sandbox; default-src 'none'"), (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], bytes).into_response()
}
