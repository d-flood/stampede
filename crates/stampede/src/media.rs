use std::{fs, io::{self, Read, Write}, path::{Path, PathBuf}};

use axum::{body::Body, extract::{Path as Param, State}, http::{HeaderMap, StatusCode, header}, response::{IntoResponse, Response}};
use base64ct::{Base64, Encoding};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use uuid::Uuid;

use crate::{AppState, chats::Item};

pub const TOOL: &str = "render_media";
pub const CLAUDE_TOOL: &str = "mcp__stampede_chat__render_media";
const LIMIT: u64 = 100 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    path: PathBuf,
    title: String,
}

#[derive(Deserialize, Serialize)]
struct Asset {
    title: String,
    asset: String,
}

fn mime(asset: &str) -> Option<&'static str> {
    let (id, extension) = asset.rsplit_once('.')?;
    if Uuid::parse_str(id).ok()?.to_string() != id { return None; }
    match extension {
        "png" => Some("image/png"),
        "jpg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "mp4" => Some("video/mp4"),
        "webm" => Some("video/webm"),
        _ => None,
    }
}

fn title_valid(title: &str) -> bool {
    !title.trim().is_empty() && title.chars().count() <= 200
}

pub fn cache_images(data_dir: &Path, item: &mut Item) {
    match item {
        Item::User { image_urls, .. } => {
            *image_urls = image_urls.iter().filter_map(|source| cache_image(data_dir, source).ok()).collect();
        }
        Item::Subagent { items, .. } => {
            for item in items { cache_images(data_dir, item); }
        }
        _ => {}
    }
}

fn cache_image(data_dir: &Path, source: &str) -> io::Result<String> {
    if let Some(asset) = source.strip_prefix("/api/media/") && mime(asset).is_some() { return Ok(source.to_owned()); }
    let (media_type, data) = source.strip_prefix("data:").and_then(|source| source.split_once(";base64,"))
        .ok_or_else(|| io::Error::other("Expected an image data URL."))?;
    let extension = match media_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => return Err(io::Error::other("Unsupported image type.")),
    };
    if data.len() > 5 * 1024 * 1024 * 4 / 3 + 4 { return Err(io::Error::other("Image is too large.")); }
    let bytes = Base64::decode_vec(data).map_err(io::Error::other)?;
    if bytes.is_empty() || bytes.len() > 5 * 1024 * 1024 { return Err(io::Error::other("Invalid image size.")); }
    let digest = Sha256::digest(&bytes);
    let asset = format!("{}.{extension}", Uuid::from_slice(&digest[..16]).unwrap());
    let directory = data_dir.join("media");
    fs::create_dir_all(&directory)?;
    if !directory.join(&asset).exists() {
        let temporary = directory.join(format!("{}.partial", Uuid::new_v4()));
        let save = (|| -> io::Result<()> {
            let mut file = fs::File::create_new(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, directory.join(&asset))
        })();
        if let Err(error) = save {
            let _ = fs::remove_file(temporary);
            return Err(error);
        }
    }
    Ok(format!("/api/media/{asset}"))
}

fn publish(data_dir: &Path, arguments: &Value) -> Result<Asset, String> {
    let input: Input = serde_json::from_value(arguments.clone()).map_err(|_| "Provide only an absolute path and a title.".to_owned())?;
    if !input.path.is_absolute() || !title_valid(&input.title) {
        return Err("Provide an absolute path and a title (1–200 characters).".to_owned());
    }
    let save = || -> io::Result<Asset> {
        let metadata = fs::metadata(&input.path)?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > LIMIT {
            return Err(io::Error::other("Choose a nonempty regular file up to 100 MiB."));
        }
        let mut source = fs::File::open(&input.path)?;
        let mut signature = [0; 16];
        let count = source.read(&mut signature)?;
        let bytes = &signature[..count];
        let extension = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { "png" }
            else if bytes.starts_with(b"\xff\xd8\xff") { "jpg" }
            else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") { "gif" }
            else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") { "webp" }
            else if bytes.get(4..8) == Some(b"ftyp") { "mp4" }
            else if bytes.starts_with(b"\x1a\x45\xdf\xa3") { "webm" }
            else { return Err(io::Error::other("Supported media: PNG, JPEG, GIF, WebP, MP4, and WebM.")); };
        let directory = data_dir.join("media");
        fs::create_dir_all(&directory)?;
        let id = Uuid::new_v4();
        let asset = format!("{id}.{extension}");
        let temporary = directory.join(format!("{id}.partial"));
        let copied = (|| -> io::Result<()> {
            let mut destination = fs::File::create_new(&temporary)?;
            destination.write_all(bytes)?;
            let length = io::copy(&mut source.take(LIMIT + 1 - count as u64), &mut destination)? + count as u64;
            if length > LIMIT { return Err(io::Error::other("Keep media under 100 MiB.")); }
            destination.sync_all()?;
            fs::rename(&temporary, directory.join(&asset))
        })();
        if let Err(error) = copied {
            let _ = fs::remove_file(temporary);
            return Err(error);
        }
        Ok(Asset { title: input.title.clone(), asset })
    };
    save().map_err(|error| error.to_string())
}

pub fn call(data_dir: &Path, arguments: &Value) -> Value {
    match publish(data_dir, arguments) {
        Ok(asset) => json!({ "content": [{ "type": "text", "text": serde_json::to_string(&asset).unwrap() }], "structuredContent": asset, "isError": false }),
        Err(error) => json!({ "content": [{ "type": "text", "text": error }], "isError": true }),
    }
}

pub fn item(result: &Value) -> Option<Item> {
    if result["isError"] == true || result["is_error"] == true { return None; }
    let asset: Asset = serde_json::from_value(result["structuredContent"].clone()).ok().or_else(|| {
        let content = &result["content"];
        if let Some(text) = content.as_str() { return serde_json::from_str(text).ok(); }
        content.as_array()?.iter().filter(|block| block["type"] == "text")
            .find_map(|block| serde_json::from_str(block["text"].as_str()?).ok())
    })?;
    if !title_valid(&asset.title) || mime(&asset.asset).is_none() { return None; }
    Some(Item::Media { title: asset.title, asset: asset.asset })
}

fn range(value: &str, length: u64) -> Option<(u64, u64)> {
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?;
        return (suffix > 0).then(|| (length.saturating_sub(suffix), length - 1));
    }
    let start = start.parse::<u64>().ok()?;
    let end = if end.is_empty() { length - 1 } else { end.parse::<u64>().ok()?.min(length - 1) };
    (start < length && start <= end).then_some((start, end))
}

pub async fn read(State(state): State<AppState>, Param(asset): Param<String>, headers: HeaderMap) -> Response {
    let Some(content_type) = mime(&asset) else { return StatusCode::NOT_FOUND.into_response() };
    let Ok(mut file) = tokio::fs::File::open(state.media_dir.join(asset)).await else { return StatusCode::NOT_FOUND.into_response() };
    let Ok(metadata) = file.metadata().await else { return StatusCode::INTERNAL_SERVER_ERROR.into_response() };
    let length = metadata.len();
    if !metadata.is_file() || length == 0 { return StatusCode::NOT_FOUND.into_response(); }
    let requested = headers.get(header::RANGE).and_then(|value| value.to_str().ok()).filter(|value| !value.contains(','));
    let (status, start, end) = match requested {
        Some(value) => match range(value, length) {
            Some((start, end)) => (StatusCode::PARTIAL_CONTENT, start, end),
            None => return (StatusCode::RANGE_NOT_SATISFIABLE, [(header::CONTENT_RANGE, format!("bytes */{length}"))]).into_response(),
        },
        None => (StatusCode::OK, 0, length - 1),
    };
    if file.seek(io::SeekFrom::Start(start)).await.is_err() { return StatusCode::INTERNAL_SERVER_ERROR.into_response(); }
    let stream = futures_util::stream::try_unfold((file, end - start + 1), |(mut file, remaining)| async move {
        if remaining == 0 { return Ok::<_, io::Error>(None); }
        let mut bytes = vec![0; remaining.min(64 * 1024) as usize];
        let count = file.read(&mut bytes).await?;
        if count == 0 { return Err(io::Error::from(io::ErrorKind::UnexpectedEof)); }
        bytes.truncate(count);
        Ok(Some((bytes, (file, remaining - count as u64))))
    });
    let mut response = (status, [
        (header::CONTENT_TYPE, content_type.to_owned()),
        (header::CONTENT_LENGTH, (end - start + 1).to_string()),
        (header::ACCEPT_RANGES, "bytes".to_owned()),
        (header::CACHE_CONTROL, "private, max-age=31536000, immutable".to_owned()),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
    ], Body::from_stream(stream)).into_response();
    if status == StatusCode::PARTIAL_CONTENT {
        response.headers_mut().insert(header::CONTENT_RANGE, format!("bytes {start}-{end}/{length}").parse().unwrap());
    }
    response
}
