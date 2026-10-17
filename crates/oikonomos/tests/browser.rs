mod common;

use std::{os::unix::fs::PermissionsExt, time::Duration};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Request, WebSocketUpgrade, ws::Message as BrowserMessage},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{any, get},
};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::{Value, json};
use tokio::{net::TcpListener, time::timeout};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

async fn preview(server: &common::Server, url: &str) -> reqwest::Response {
    Client::new()
        .post(format!("http://127.0.0.1:{}/api/browser", server.port))
        .header("origin", format!("http://127.0.0.1:{}", server.port))
        .header("content-type", "application/json")
        .body(json!({ "url": url }).to_string())
        .send()
        .await
        .unwrap()
}

async fn open(server: &common::Server, url: &str) -> Value {
    let response = preview(server, url).await;
    assert!(
        response.status().is_success(),
        "{}",
        response.text().await.unwrap()
    );
    serde_json::from_str(&response.text().await.unwrap()).unwrap()
}

#[tokio::test]
async fn proxy_preserves_paths_bodies_headers_redirects_and_streams() {
    let server = common::Server::start("browser-http-").await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new()
        .route(
            "/redirect",
            get(move || async move {
                (
                    StatusCode::FOUND,
                    [(
                        "location",
                        format!("http://localhost:{port}/next?q=1#anchor"),
                    )],
                )
            }),
        )
        .route(
            "/events",
            get(|| async {
                let stream = futures_util::stream::once(async {
                    Ok::<_, std::io::Error>(Bytes::from_static(b"data: ready\n\n"))
                })
                .chain(futures_util::stream::pending());
                (
                    [("content-type", "text/event-stream")],
                    Body::from_stream(stream),
                )
            }),
        )
        .fallback(any(|request: Request| async move {
            let (parts, body) = request.into_parts();
            let bytes = axum::body::to_bytes(body, 1_000_000).await.unwrap();
            let mut response = (StatusCode::CREATED, bytes).into_response();
            for (name, value) in [
                ("x-path", parts.uri.to_string()),
                ("x-method", parts.method.to_string()),
                ("x-host", parts.headers["host"].to_str().unwrap().to_owned()),
                (
                    "x-origin",
                    parts
                        .headers
                        .get("origin")
                        .map_or("", |value| value.to_str().unwrap())
                        .into(),
                ),
                ("x-frame-options", "DENY".into()),
                (
                    "content-security-policy",
                    "script-src 'self'; frame-ancestors 'none'; connect-src 'self'".into(),
                ),
                ("connection", "x-private".into()),
                ("x-private", "remove".into()),
            ] {
                response.headers_mut().insert(
                    axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                    value.parse().unwrap(),
                );
            }
            response
                .headers_mut()
                .append("set-cookie", "one=1; Path=/".parse().unwrap());
            response
                .headers_mut()
                .append("set-cookie", "two=2; Path=/".parse().unwrap());
            response
        }));
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let target = format!("http://127.0.0.1:{port}");
    let opened = open(&server, &format!("{target}/deep/path?q=%26#anchor")).await;
    let src = opened["src"].as_str().unwrap();
    let origin = reqwest::Url::parse(src)
        .unwrap()
        .origin()
        .ascii_serialization();
    assert_ne!(origin, target);
    assert_ne!(origin, format!("http://127.0.0.1:{}", server.port));
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let response = client
        .post(src)
        .header("origin", &origin)
        .body(vec![0, 255, 10])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["x-path"], "/deep/path?q=%26");
    assert_eq!(response.headers()["x-method"], "POST");
    assert_eq!(response.headers()["x-host"], format!("127.0.0.1:{port}"));
    assert_eq!(response.headers()["x-origin"], target);
    assert!(!response.headers().contains_key("x-frame-options"));
    assert!(!response.headers().contains_key("x-private"));
    assert_eq!(response.headers().get_all("set-cookie").iter().count(), 2);
    assert_eq!(
        response.headers()["content-security-policy"],
        "script-src 'self'; connect-src 'self'"
    );
    assert_eq!(response.bytes().await.unwrap().as_ref(), &[0, 255, 10]);
    let again = open(&server, &format!("{target}/asset.js")).await;
    assert_eq!(again["src"], format!("{origin}/asset.js"));
    let response = client
        .get(format!("{origin}/asset.js"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers()["x-path"], "/asset.js");
    let response = client
        .get(format!("{origin}/redirect"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(
        response.headers()["location"],
        format!("{origin}/next?q=1#anchor")
    );
    let mut response = client.get(format!("{origin}/events")).send().await.unwrap();
    let first = timeout(Duration::from_secs(2), response.chunk())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(first, "data: ready\n\n");
    task.abort();
}

#[tokio::test]
async fn proxy_forwards_websocket_protocol_text_binary_and_close() {
    let server = common::Server::start("browser-ws-").await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let app = Router::new().route(
            "/socket",
            get(
                move |upgrade: WebSocketUpgrade, headers: HeaderMap| async move {
                    assert_eq!(headers["origin"], format!("http://127.0.0.1:{port}"));
                    assert_eq!(headers["cookie"], "preview=1");
                    upgrade
                        .protocols(["vite-hmr"])
                        .on_upgrade(|mut socket| async move {
                            while let Some(Ok(message)) = socket.recv().await {
                                let closing = matches!(message, BrowserMessage::Close(_));
                                socket.send(message).await.unwrap();
                                if closing {
                                    break;
                                }
                            }
                        })
                },
            ),
        );
        axum::serve(listener, app).await.unwrap();
    });
    let opened = open(&server, &format!("127.0.0.1:{port}")).await;
    let origin = opened["src"].as_str().unwrap().trim_end_matches('/');
    let mut request = format!("{}/socket", origin.replacen("http", "ws", 1))
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("sec-websocket-protocol", "vite-hmr".parse().unwrap());
    request
        .headers_mut()
        .insert("origin", origin.parse().unwrap());
    request
        .headers_mut()
        .insert("cookie", "preview=1".parse().unwrap());
    let (mut socket, response) = connect_async(request).await.unwrap();
    assert_eq!(response.headers()["sec-websocket-protocol"], "vite-hmr");
    for message in [
        Message::Text("hot update".into()),
        Message::Binary(vec![0, 255].into()),
    ] {
        socket.send(message.clone()).await.unwrap();
        assert_eq!(
            timeout(Duration::from_secs(2), socket.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            message
        );
    }
    socket.close(None).await.unwrap();
    assert!(matches!(
        timeout(Duration::from_secs(2), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        Message::Close(_)
    ));
    task.abort();
}

#[tokio::test]
async fn console_bridge_preserves_html_policies_and_streaming() {
    let server = common::Server::start("browser-console-").await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new()
        .route("/html", get(|headers: HeaderMap| async move {
            assert_eq!(headers["accept-encoding"], "identity");
            let chunks = futures_util::stream::iter([
                Ok::<_, std::io::Error>(Bytes::from_static(b"<!doctype html><!-- <body> --><html><HE")),
                Ok(Bytes::from_static(b"AD><meta http-equiv=\"Content-Security-Policy\" content=\"script-src 'self'\"><title>Preview</title><scr")),
                Ok(Bytes::from_static(b"ipt src=\"/app.js\"></script></HEAD><body>Preview</body></html>")),
            ]);
            ([
                ("content-type", "text/html; charset=utf-8"),
                ("content-security-policy", "script-src 'self'; frame-ancestors 'none'"),
                ("etag", "\"original\""),
            ], Body::from_stream(chunks))
        }))
        .route("/stream", get(|| async {
            let stream = futures_util::stream::once(async {
                Ok::<_, std::io::Error>(Bytes::from_static(b"<!doctype html><body>Still streaming"))
            }).chain(futures_util::stream::pending());
            ([("content-type", "text/html")], Body::from_stream(stream))
        }))
        .route("/encoded", get(|| async {
            ([("content-type", "text/html"), ("content-encoding", "gzip")], vec![1u8, 2, 3])
        }));
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let opened = open(&server, &format!("localhost:{port}/html")).await;
    let src = opened["src"].as_str().unwrap();
    let origin = reqwest::Url::parse(src)
        .unwrap()
        .origin()
        .ascii_serialization();
    let client = Client::new();
    let response = client.get(src).send().await.unwrap();
    assert!(!response.headers().contains_key("content-length"));
    assert!(!response.headers().contains_key("etag"));
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        response.headers()["content-security-policy"],
        "script-src 'self'"
    );
    let html = response.text().await.unwrap();
    assert!(html.starts_with("<!doctype html><!-- <body> --><html><HEAD>"));
    assert_eq!(html.matches("/__oikonomos/console.js").count(), 1);
    let bridge = html.find("/__oikonomos/console.js").unwrap();
    assert!(bridge > html.find("Content-Security-Policy").unwrap());
    assert!(bridge < html.find("/app.js").unwrap());
    assert!(html.contains(&format!(
        "data-parent-origin=\"http://127.0.0.1:{}\"",
        server.port
    )));
    let bridge = client
        .get(format!("{origin}/__oikonomos/console.js"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        bridge.headers()["content-type"],
        "text/javascript; charset=utf-8"
    );
    assert!(bridge.text().await.unwrap().contains("unhandledrejection"));
    let mut response = client.get(format!("{origin}/stream")).send().await.unwrap();
    let chunk = timeout(Duration::from_secs(2), response.chunk())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&chunk).contains("/__oikonomos/console.js"));
    let response = client
        .get(format!("{origin}/encoded"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers()["content-encoding"], "gzip");
    assert_eq!(response.bytes().await.unwrap().as_ref(), &[1, 2, 3]);
    task.abort();
}

#[tokio::test]
async fn previews_reject_nonlocal_targets_and_keep_ports_separate() {
    let server = common::Server::start("browser-targets-").await;
    for target in [
        "https://localhost:5173",
        "http://example.com",
        "http://localhost.example.com",
        "http://user:secret@localhost:5173",
        "file:///etc/passwd",
    ] {
        assert_eq!(
            preview(&server, target).await.status(),
            StatusCode::BAD_REQUEST,
            "{target}"
        );
    }
    assert_eq!(
        preview(&server, &format!("localhost:{}", server.port))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let one = open(&server, "localhost:5173").await;
    let two = open(&server, "localhost:5174").await;
    assert_ne!(one["src"], two["src"]);
    let port = reqwest::Url::parse(one["src"].as_str().unwrap())
        .unwrap()
        .port()
        .unwrap();
    assert_eq!(
        preview(&server, &format!("localhost:{port}"))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let response = Client::new()
        .post(format!("http://127.0.0.1:{}/api/browser", server.port))
        .header("origin", "https://example.com")
        .header("content-type", "application/json")
        .body(json!({"url": "localhost:5173"}).to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn tailscale_preview_uses_a_temporary_foreground_endpoint() {
    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("tailscale");
    let pid = root.path().join("pid");
    std::fs::write(&script, format!("#!/bin/sh\n[ \"$1\" = serve ] && [ \"$2\" = --yes ] || exit 1\nport=${{3#--https=}}\nprintf '%s' \"$$\" > '{}'\nprintf 'Available within your tailnet:\\nhttps://preview.example.ts.net:%s\\n' \"$port\"\nexec /bin/sleep 60\n", pid.display())).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        root.path().display(),
        std::env::var("PATH").unwrap()
    );
    let mut server =
        common::Server::with_env("browser-tailscale-", "", vec![("PATH", path.into())]).await;
    let request = || {
        Client::new()
            .post(format!("http://127.0.0.1:{}/api/browser", server.port))
            .header("origin", "https://preview.example.ts.net:8443")
            .header("host", "preview.example.ts.net:8443")
            .header("content-type", "application/json")
            .body(json!({"url":"localhost:5173/deep?q=1"}).to_string())
    };
    let response = request().send().await.unwrap();
    assert!(
        response.status().is_success(),
        "{}",
        response.text().await.unwrap()
    );
    let first: Value = serde_json::from_str(&response.text().await.unwrap()).unwrap();
    let src = reqwest::Url::parse(first["src"].as_str().unwrap()).unwrap();
    assert_eq!(src.scheme(), "https");
    assert_eq!(src.host_str(), Some("preview.example.ts.net"));
    assert_ne!(src.port(), Some(8443));
    assert_eq!(src.path(), "/deep");
    assert_eq!(src.query(), Some("q=1"));
    let second: Value =
        serde_json::from_str(&request().send().await.unwrap().text().await.unwrap()).unwrap();
    assert_eq!(first, second);
    let pid: i32 = std::fs::read_to_string(pid).unwrap().parse().unwrap();
    server.stop().await;
    timeout(Duration::from_secs(3), async {
        while std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .is_ok_and(|stat| !stat.contains(") Z "))
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
