use crate::common;

use std::{fs, time::Duration};

use common::Server;
use serde_json::{Value, json};

const ORIGINAL: &str = "# my Host\nroots = []\n\n[[rig]]\nname = 'Mine'\nagent = 'codex'\n\n[editor]\nkeymap = 'vim'\n";

async fn json(response: reqwest::Response) -> Value {
    serde_json::from_str(&response.text().await.unwrap()).unwrap()
}

async fn host_name(client: &reqwest::Client, base: &str) -> Value {
    let response = client.get(format!("{base}/api/host-name")).send().await.unwrap();
    assert_eq!(response.status(), 200);
    json(response).await
}

async fn save(client: &reqwest::Client, base: &str, name: Value) -> reqwest::Response {
    client.put(format!("{base}/api/host-name"))
        .header("Content-Type", "application/json")
        .body(name.to_string())
        .send().await.unwrap()
}

async fn manifest(client: &reqwest::Client, base: &str) -> String {
    let response = client.get(format!("{base}/manifest.webmanifest")).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "application/manifest+json");
    assert_eq!(response.headers()["cache-control"], "no-cache");
    let manifest = json(response).await;
    let mut keys: Vec<&str> = manifest.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["display", "icons", "name", "scope", "short_name", "start_url"]);
    assert_eq!(manifest["start_url"], "/");
    assert_eq!(manifest["scope"], "/");
    assert_eq!(manifest["display"], "standalone");
    assert_eq!(manifest["icons"], json!([
        { "src": "/icons/icon-192.png", "sizes": "192x192", "type": "image/png" },
        { "src": "/icons/icon-512.png", "sizes": "512x512", "type": "image/png" },
        { "src": "/icons/maskable-512.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" }
    ]));
    assert_eq!(manifest["short_name"], manifest["name"]);
    manifest["name"].as_str().unwrap().to_owned()
}

async fn settled(client: &reqwest::Client, base: &str, name: Option<&str>) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while host_name(client, base).await != json!(name) || manifest(client, base).await != name.unwrap_or("Stampede") {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }).await.unwrap();
}

#[tokio::test]
async fn host_name_is_saved_in_the_config_and_names_the_manifest_live() {
    let mut server = Server::with_config("host-name-", ORIGINAL).await;
    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", server.port);
    let config = server.root.path().join("config.toml");
    assert_eq!(host_name(&client, &base).await, Value::Null);
    assert_eq!(manifest(&client, &base).await, "Stampede");

    let response = save(&client, &base, json!(" atlas ")).await;
    assert_eq!(response.status(), 200);
    assert_eq!(json(response).await, "atlas");
    assert_eq!(fs::read_to_string(&config).unwrap(), format!("name = \"atlas\"\n{ORIGINAL}"));
    settled(&client, &base, Some("atlas")).await;

    let response = save(&client, &base, json!("zeus")).await;
    assert_eq!(json(response).await, "zeus");
    assert_eq!(fs::read_to_string(&config).unwrap(), format!("name = \"zeus\"\n{ORIGINAL}"));
    settled(&client, &base, Some("zeus")).await;
    let rigs = json(client.get(format!("{base}/api/rigs")).send().await.unwrap()).await;
    assert_eq!(rigs[0]["name"], "Mine");

    server.restart().await;
    assert_eq!(host_name(&client, &base).await, "zeus");
    assert_eq!(manifest(&client, &base).await, "zeus");

    fs::write(&config, format!("name = 'hand'\n{ORIGINAL}")).unwrap();
    settled(&client, &base, Some("hand")).await;

    let response = save(&client, &base, Value::Null).await;
    assert_eq!(response.status(), 200);
    assert_eq!(json(response).await, Value::Null);
    assert_eq!(fs::read_to_string(&config).unwrap(), ORIGINAL);
    settled(&client, &base, None).await;

    save(&client, &base, json!("atlas")).await;
    settled(&client, &base, Some("atlas")).await;
    let response = save(&client, &base, json!("  ")).await;
    assert_eq!(response.status(), 200);
    assert_eq!(json(response).await, Value::Null);
    assert_eq!(fs::read_to_string(&config).unwrap(), ORIGINAL);
    settled(&client, &base, None).await;

    let invalid = "[editor\n";
    fs::write(&config, invalid).unwrap();
    for name in [json!("atlas"), Value::Null] {
        let response = save(&client, &base, name).await;
        assert_eq!(response.status(), 400);
        assert!(!response.text().await.unwrap().is_empty());
        assert_eq!(fs::read_to_string(&config).unwrap(), invalid);
    }
    assert_eq!(manifest(&client, &base).await, "Stampede");
}
