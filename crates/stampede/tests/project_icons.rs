mod common;

use std::{fs, sync::{Arc, Mutex}};

use axum::{Router, extract::OriginalUri, http::{StatusCode, header}, response::IntoResponse, routing::get};
use base64ct::{Base64, Encoding};
use common::{Server, control, next_control};
use serde_json::json;
use tokio_tungstenite::connect_async;

fn png() -> Vec<u8> {
    Base64::decode_vec("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=").unwrap()
}

#[tokio::test]
async fn icons_survive_closing_reopening_and_restart_and_broadcast_to_devices() {
    let mut server = Server::start("project-icons-").await;
    let root = server.root.path().join("project");
    fs::create_dir(&root).unwrap();
    let connect = || connect_async(format!("ws://127.0.0.1:{}/ws", server.port));
    let (mut socket, _) = connect().await.unwrap();
    control(&mut socket, json!({ "type": "project.open", "path": root })).await;
    let opened = next_control(&mut socket, "project.opened").await;
    let id = opened["projectId"].as_str().unwrap().to_owned();
    let initial = next_control(&mut socket, "projects.changed").await;
    assert!(initial["projects"][0]["icon"].is_null());
    let base = format!("http://127.0.0.1:{}", server.port);
    let api = format!("{base}/api/projects/{id}/icon");
    let client = reqwest::Client::new();
    assert_eq!(client.put(&api).body("not an image").send().await.unwrap().status(), 400);
    assert_eq!(client.put(&api).body(png()).send().await.unwrap().status(), 204);
    let changed = next_control(&mut socket, "projects.changed").await;
    let asset = changed["projects"][0]["icon"].as_str().unwrap().to_owned();
    let response = client.get(format!("{base}/api/project-icons/{asset}")).send().await.unwrap();
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(response.bytes().await.unwrap().as_ref(), png());

    control(&mut socket, json!({ "type": "project.close", "projectId": id })).await;
    assert_eq!(next_control(&mut socket, "projects.changed").await["projects"], json!([]));
    control(&mut socket, json!({ "type": "project.open", "path": root })).await;
    next_control(&mut socket, "project.opened").await;
    assert_eq!(next_control(&mut socket, "projects.changed").await["projects"][0]["icon"], asset);
    drop(socket);
    server.restart().await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "projects.list" })).await;
    assert_eq!(next_control(&mut socket, "projects.changed").await["projects"][0]["icon"], asset);
    assert_eq!(client.delete(&api).send().await.unwrap().status(), 204);
    assert!(next_control(&mut socket, "projects.changed").await["projects"][0]["icon"].is_null());
    drop(socket);
    server.restart().await;
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{}/ws", server.port)).await.unwrap();
    control(&mut socket, json!({ "type": "projects.list" })).await;
    assert!(next_control(&mut socket, "projects.changed").await["projects"][0]["icon"].is_null());
    assert_eq!(client.put(format!("{base}/api/projects/missing/icon")).body(png()).send().await.unwrap().status(), 404);
}

#[tokio::test]
async fn discovery_uses_the_full_url_redirects_base_and_icon_links_before_domain_fallback() {
    let server = Server::start("website-icons-").await;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let site = Router::new().fallback(get(move |OriginalUri(uri): OriginalUri| {
        seen.lock().unwrap().push(uri.to_string());
        async move {
            match uri.path() {
                "/repo" => (StatusCode::FOUND, [(header::LOCATION, "/repo/?theme=dark")]).into_response(),
                "/repo/" => "<head><base href='assets/'><link rel='apple-touch-icon' href='touch.png'><link rel='shortcut ICON' href='missing.png'><link rel='ICON' href='brand.svg'></head>".into_response(),
                "/repo/assets/brand.svg" => ([(header::CONTENT_TYPE, "image/svg+xml")], "<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32'/>").into_response(),
                "/fallback/" => "<title>No declared icon</title>".into_response(),
                "/fallback/favicon.ico" => ([(header::CONTENT_TYPE, "image/png")], png()).into_response(),
                "/large/" => "x".repeat(2 * 1024 * 1024 + 1).into_response(),
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let site = tokio::spawn(async move { axum::serve(listener, site).await.unwrap() });
    let client = reqwest::Client::new();
    let api = format!("http://127.0.0.1:{}/api/project-icons/website", server.port);
    let discover = |url: String| client.post(&api).header("content-type", "application/json").body(json!({ "url": url }).to_string()).send();
    let response = discover(format!("{origin}/repo")).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "image/svg+xml");
    assert!(response.text().await.unwrap().contains("<svg"));
    assert_eq!(*requests.lock().unwrap(), ["/repo", "/repo/?theme=dark", "/repo/assets/missing.png", "/repo/assets/brand.svg"]);
    let response = discover(format!("{origin}/fallback/")).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.bytes().await.unwrap().as_ref(), png());
    for value in [format!("{origin}/none/"), format!("{origin}/large/"), "file:///etc/passwd".to_owned(), "https://user:pass@example.org".to_owned()] {
        let response: reqwest::Response = discover(value).await.unwrap();
        assert_eq!(response.status(), 400);
    }
    site.abort();
}
