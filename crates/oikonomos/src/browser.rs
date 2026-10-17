use std::{collections::HashMap, process::Stdio, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    body::Body,
    extract::{
        FromRequestParts, Request, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use lol_html::{HtmlRewriter, Settings, element, html_content::ContentType};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    net::TcpListener,
    process::{Child, Command},
    sync::Mutex,
    task::JoinHandle,
    time::timeout,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message as UpstreamMessage, client::IntoClientRequest},
};

use crate::AppState;

type Failure = (StatusCode, String);

pub struct Browsers {
    port: u16,
    client: Client,
    previews: Mutex<HashMap<String, Preview>>,
}

struct Preview {
    origin: String,
    port: u16,
    task: JoinHandle<()>,
    tailscale: Option<Child>,
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
struct Proxy {
    target: Url,
    origin: String,
    parent_origin: String,
    client: Client,
}

const CONSOLE_PATH: &str = "/__oikonomos/console.js";

#[derive(Deserialize)]
pub struct Open {
    url: String,
}

impl Browsers {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            client: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            previews: Mutex::new(HashMap::new()),
        }
    }

    async fn open(&self, target: &Url, device: &Url) -> Result<String, Failure> {
        let mut previews = self.previews.lock().await;
        let target_port = target.port_or_known_default().unwrap();
        if target_port == self.port || previews.values().any(|preview| preview.port == target_port)
        {
            return Err(bad(
                "Choose a development server port, rather than an Oikonomos port.",
            ));
        }
        let key = format!(
            "{} {}",
            device.origin().ascii_serialization(),
            target.origin().ascii_serialization()
        );
        if let Some(preview) = previews.get_mut(&key) {
            let alive = !preview.task.is_finished()
                && preview
                    .tailscale
                    .as_mut()
                    .is_none_or(|child| child.try_wait().is_ok_and(|status| status.is_none()));
            if alive {
                return Ok(preview.origin.clone());
            }
        }
        previews.remove(&key);
        let listener = TcpListener::bind("127.0.0.1:0").await.map_err(upstream)?;
        let port = listener.local_addr().map_err(upstream)?.port();
        let (origin, tailscale) = if device.scheme() == "http" && loopback(device) {
            (format!("http://127.0.0.1:{port}"), None)
        } else if device.scheme() == "https"
            && device
                .host_str()
                .is_some_and(|host| host.ends_with(".ts.net"))
        {
            let (origin, child) = expose(port, device.host_str().unwrap()).await?;
            (origin, Some(child))
        } else {
            return Err(bad(
                "Browser previews require Oikonomos over local HTTP or Tailscale HTTPS.",
            ));
        };
        let mut base = target.clone();
        base.set_path("/");
        base.set_query(None);
        base.set_fragment(None);
        let proxy = Proxy {
            target: base,
            origin: origin.clone(),
            parent_origin: device.origin().ascii_serialization(),
            client: self.client.clone(),
        };
        let task = tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                Router::new().fallback(forward).with_state(Arc::new(proxy)),
            )
            .await;
        });
        previews.insert(
            key,
            Preview {
                origin: origin.clone(),
                port,
                task,
                tailscale,
            },
        );
        Ok(origin)
    }
}

pub async fn open(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Open>,
) -> Result<Json<Value>, Failure> {
    let value = if input.url.contains("://") {
        input.url
    } else {
        format!("http://{}", input.url)
    };
    let target = Url::parse(&value).map_err(|_| bad("Enter an address such as localhost:5173."))?;
    if target.scheme() != "http"
        || !loopback(&target)
        || target.port() == Some(0)
        || !target.username().is_empty()
        || target.password().is_some()
    {
        return Err(bad(
            "Enter an HTTP address on the Host: localhost, 127.0.0.1, or [::1].",
        ));
    }
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Url::parse(value).ok())
        .ok_or_else(|| bad("Missing Device origin."))?;
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let expected = Url::parse(&format!("{}://{host}", origin.scheme()))
        .map_err(|_| bad("Invalid Device origin."))?;
    if origin.origin() != expected.origin() {
        return Err((
            StatusCode::FORBIDDEN,
            "Open previews from Oikonomos.".into(),
        ));
    }
    let preview = state.browsers.open(&target, &origin).await?;
    let mut src = Url::parse(&preview).unwrap();
    src.set_path(target.path());
    src.set_query(target.query());
    src.set_fragment(target.fragment());
    Ok(Json(json!({ "url": target.as_str(), "src": src.as_str() })))
}

fn loopback(url: &Url) -> bool {
    matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
}

fn bad(message: &str) -> Failure {
    (StatusCode::BAD_REQUEST, message.into())
}

fn upstream(error: impl std::fmt::Display) -> Failure {
    (
        StatusCode::BAD_GATEWAY,
        format!("Cannot reach the Host preview: {error}"),
    )
}

async fn expose(port: u16, hostname: &str) -> Result<(String, Child), Failure> {
    let mut command = Command::new("tailscale");
    command
        .args([
            "serve",
            "--yes",
            &format!("--https={port}"),
            &format!("http://127.0.0.1:{port}"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(target_os = "linux")]
    unsafe {
        command.pre_exec(|| {
            nix::sys::prctl::set_pdeathsig(nix::sys::signal::Signal::SIGTERM)
                .map_err(std::io::Error::from)
        });
    }
    let mut child = command
        .spawn()
        .map_err(|error| upstream(format!("Tailscale Serve: {error}")))?;
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let ready = timeout(Duration::from_secs(15), async {
        while let Some(line) = lines.next_line().await.map_err(upstream)? {
            if let Some(url) = line.split_whitespace().find_map(|word| Url::parse(word).ok()) {
                if url.scheme() == "https" && url.host_str() == Some(hostname) && url.port() == Some(port) {
                    return Ok(url.origin().ascii_serialization());
                }
            }
        }
        let mut error = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut error).await;
        }
        Err(upstream(format!("Tailscale Serve: {}", error.trim())))
    }).await.map_err(|_| upstream("Tailscale Serve timed out. Check that Serve is enabled and available to the Host user."))??;
    Ok((ready, child))
}

async fn forward(State(proxy): State<Arc<Proxy>>, request: Request) -> Result<Response, Failure> {
    if request.uri().path() == CONSOLE_PATH {
        return Ok((
            [
                (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
                (header::CACHE_CONTROL, "no-store"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            include_str!("browser-console.js"),
        )
            .into_response());
    }
    let (mut parts, body) = request.into_parts();
    let head = parts.method == axum::http::Method::HEAD;
    let path = parts.uri.path_and_query().map_or("/", |path| path.as_str());
    let mut target = proxy.target.clone();
    let (path, query) = path
        .split_once('?')
        .map_or((path, None), |(path, query)| (path, Some(query)));
    target.set_path(path);
    target.set_query(query);
    let websocket = parts
        .headers
        .get(header::UPGRADE)
        .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"websocket"));
    if websocket {
        let upgrade = WebSocketUpgrade::from_request_parts(&mut parts, &())
            .await
            .map_err(|error| bad(&error.to_string()))?;
        target.set_scheme("ws").unwrap();
        let mut request = target.as_str().into_client_request().map_err(upstream)?;
        for name in [
            header::COOKIE,
            header::AUTHORIZATION,
            header::SEC_WEBSOCKET_PROTOCOL,
        ] {
            if let Some(value) = parts.headers.get(&name) {
                request.headers_mut().insert(name, value.clone());
            }
        }
        request.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_str(&proxy.target.origin().ascii_serialization()).unwrap(),
        );
        let (upstream, response) = timeout(Duration::from_secs(5), connect_async(request))
            .await
            .map_err(upstream)?
            .map_err(upstream)?;
        let protocol = response
            .headers()
            .get(header::SEC_WEBSOCKET_PROTOCOL)
            .and_then(|value| value.to_str().ok());
        let upgrade = upgrade.protocols(protocol.into_iter().map(str::to_owned));
        return Ok(upgrade.on_upgrade(move |socket| bridge(socket, upstream)));
    }
    let mut headers = parts.headers;
    clean(&mut headers);
    headers.remove(header::HOST);
    headers.insert(
        header::ACCEPT_ENCODING,
        HeaderValue::from_static("identity"),
    );
    for name in [header::ORIGIN, header::REFERER] {
        if let Some(value) = headers.get(&name).and_then(|value| value.to_str().ok()) {
            if let Some(suffix) = value
                .strip_prefix(&proxy.origin)
                .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
            {
                let value = format!("{}{suffix}", proxy.target.origin().ascii_serialization());
                headers.insert(name, HeaderValue::from_str(&value).map_err(upstream)?);
            }
        }
    }
    let response = proxy
        .client
        .request(parts.method, target)
        .headers(headers)
        .body(reqwest::Body::wrap_stream(body.into_data_stream()))
        .send()
        .await
        .map_err(upstream)?;
    let status = response.status();
    let mut headers = response.headers().clone();
    clean(&mut headers);
    headers.remove(header::X_FRAME_OPTIONS);
    if let Some(value) = headers
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
    {
        if let Ok(url) = Url::parse(value) {
            if loopback(&url) && url.port_or_known_default() == proxy.target.port_or_known_default()
            {
                let mut location = Url::parse(&proxy.origin).unwrap();
                location.set_path(url.path());
                location.set_query(url.query());
                location.set_fragment(url.fragment());
                headers.insert(
                    header::LOCATION,
                    HeaderValue::from_str(location.as_str()).map_err(upstream)?,
                );
            }
        }
    }
    let policies: Vec<_> = headers
        .get_all(header::CONTENT_SECURITY_POLICY)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(';')
                .filter(|directive| {
                    !directive
                        .trim_start()
                        .split_whitespace()
                        .next()
                        .is_some_and(|name| name.eq_ignore_ascii_case("frame-ancestors"))
                })
                .collect::<Vec<_>>()
                .join(";")
        })
        .collect();
    headers.remove(header::CONTENT_SECURITY_POLICY);
    for policy in policies {
        headers.append(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_str(&policy).map_err(upstream)?,
        );
    }
    let html = !head
        && status != StatusCode::PARTIAL_CONTENT
        && headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .eq_ignore_ascii_case("text/html")
            })
        && headers
            .get(header::CONTENT_ENCODING)
            .is_none_or(|value| value == "identity");
    let body = if html {
        for name in [
            header::CONTENT_LENGTH,
            header::ETAG,
            header::LAST_MODIFIED,
            header::ACCEPT_RANGES,
        ] {
            headers.remove(name);
        }
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        console_body(response, &proxy)
    } else {
        Body::from_stream(response.bytes_stream())
    };
    let mut result = (status, body).into_response();
    *result.headers_mut() = headers;
    Ok(result)
}

fn console_body(response: reqwest::Response, proxy: &Proxy) -> Body {
    let parent_origin = proxy
        .parent_origin
        .replace('&', "&amp;")
        .replace('"', "&quot;");
    let script = format!(
        r#"<script src="{}{CONSOLE_PATH}" data-parent-origin="{parent_origin}"></script>"#,
        proxy.origin
    );
    let output = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = Arc::clone(&output);
    let mut injected = false;
    let rewriter = HtmlRewriter::new(
        Settings::new_send().append_element_content_handler(element!("*", move |element| {
            if !injected
                && !["html", "head", "meta", "base", "title", "link", "style"]
                    .contains(&element.tag_name().as_str())
            {
                element.before(&script, ContentType::Html);
                injected = true;
            }
            Ok(())
        })),
        move |chunk: &[u8]| sink.lock().unwrap().extend_from_slice(chunk),
    );
    let stream = futures_util::stream::try_unfold(
        (response.bytes_stream(), Some(rewriter)),
        move |(mut stream, mut rewriter)| {
            let output = Arc::clone(&output);
            async move {
                while let Some(mut active) = rewriter.take() {
                    match stream.next().await {
                        Some(chunk) => {
                            active
                                .write(&chunk.map_err(std::io::Error::other)?)
                                .map_err(std::io::Error::other)?;
                            rewriter = Some(active);
                        }
                        None => active.end().map_err(std::io::Error::other)?,
                    }
                    let chunk = std::mem::take(&mut *output.lock().unwrap());
                    if !chunk.is_empty() {
                        return Ok::<_, std::io::Error>(Some((chunk, (stream, rewriter))));
                    }
                }
                Ok(None)
            }
        },
    );
    Body::from_stream(stream)
}

fn clean(headers: &mut HeaderMap) {
    let extra: Vec<_> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(',').map(|name| name.trim().to_owned()))
        .collect();
    for name in extra {
        headers.remove(name);
    }
    for name in [
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ] {
        headers.remove(name);
    }
}

async fn bridge(
    socket: WebSocket,
    upstream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) {
    let (mut write, mut read) = socket.split();
    let (mut upstream_write, mut upstream_read) = upstream.split();
    let send = async {
        while let Some(Ok(message)) = read.next().await {
            let message = match message {
                Message::Text(value) => UpstreamMessage::Text(value.as_str().into()),
                Message::Binary(value) => UpstreamMessage::Binary(value),
                Message::Close(value) => UpstreamMessage::Close(value.map(|value| {
                    tokio_tungstenite::tungstenite::protocol::CloseFrame {
                        code: value.code.into(),
                        reason: value.reason.as_str().into(),
                    }
                })),
                Message::Ping(_) | Message::Pong(_) => continue,
            };
            if upstream_write.send(message).await.is_err() {
                break;
            }
        }
        let _ = upstream_write.close().await;
    };
    let receive = async {
        while let Some(Ok(message)) = upstream_read.next().await {
            let message = match message {
                UpstreamMessage::Text(value) => Message::Text(value.as_str().into()),
                UpstreamMessage::Binary(value) => Message::Binary(value),
                UpstreamMessage::Close(value) => {
                    Message::Close(value.map(|value| axum::extract::ws::CloseFrame {
                        code: value.code.into(),
                        reason: value.reason.as_str().into(),
                    }))
                }
                _ => continue,
            };
            if write.send(message).await.is_err() {
                break;
            }
        }
        let _ = write.close().await;
    };
    tokio::select! { _ = send => {}, _ = receive => {} }
}
