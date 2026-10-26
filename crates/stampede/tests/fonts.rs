mod common;

use std::{fs, time::Duration};

use common::Server;
use serde_json::{Value, json};

const ORIGINAL: &str = "# keep\nroots = []\nrig = []\n\n[editor]\nkeymap = 'vim'\n";
const FAMILY: &str = "Symbols Nerd Font Mono";

async fn get(client: &reqwest::Client, url: &str) -> reqwest::Response {
    client.get(url).send().await.unwrap()
}

async fn json(response: reqwest::Response) -> Value {
    serde_json::from_str(&response.text().await.unwrap()).unwrap()
}

async fn save(client: &reqwest::Client, base: &str, family: Value) -> reqwest::Response {
    client.put(format!("{base}/api/ui-font"))
        .header("Content-Type", "application/json")
        .body(family.to_string())
        .send().await.unwrap()
}

async fn settled(client: &reqwest::Client, base: &str, expected: &Value) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while json(get(client, &format!("{base}/api/ui-font")).await).await != *expected {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }).await.unwrap();
}

#[tokio::test]
async fn installed_fonts_are_listed_served_and_saved_without_rewriting_other_settings() {
    let server = Server::with_config("fonts-", ORIGINAL).await;
    let data = fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/static/fonts/SymbolsNerdFontMono-Regular.woff2")).unwrap();
    let installed = server.root.path().join("home/.local/share/fonts");
    fs::create_dir_all(&installed).unwrap();
    fs::write(installed.join("Symbols.woff2"), &data).unwrap();
    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", server.port);
    let config = server.root.path().join("config.toml");

    let families: Vec<String> = serde_json::from_value(json(get(&client, &format!("{base}/api/fonts")).await).await).unwrap();
    assert!(families.iter().any(|family| family == FAMILY), "{families:?}");
    assert!(families.windows(2).all(|pair| pair[0] < pair[1]));

    let font = json(get(&client, &format!("{base}/api/fonts/Symbols%20Nerd%20Font%20Mono")).await).await;
    assert_eq!(font["family"], FAMILY);
    let mut served = Vec::new();
    for face in font["faces"].as_array().unwrap() {
        let url = face["url"].as_str().unwrap();
        assert!(url.starts_with("/fonts/system/Symbols%20Nerd%20Font%20Mono/"), "{url}");
        assert_eq!(face["stretch"], "100%");
        served.push(get(&client, &format!("{base}{url}")).await.bytes().await.unwrap());
    }
    assert!(served.iter().any(|bytes| bytes[..] == data[..]));
    assert_eq!(get(&client, &format!("{base}/api/fonts/No%20Such%20Font")).await.status(), 404);
    assert_eq!(get(&client, &format!("{base}/fonts/system/Symbols%20Nerd%20Font%20Mono/9999")).await.status(), 404);
    assert_eq!(get(&client, &format!("{base}/fonts/system/No%20Such%20Font/0")).await.status(), 404);

    assert_eq!(json(get(&client, &format!("{base}/api/ui-font")).await).await, Value::Null);
    let response = save(&client, &base, json!(FAMILY)).await;
    assert_eq!(response.status(), 200);
    assert_eq!(json(response).await, font);
    assert_eq!(fs::read_to_string(&config).unwrap(), format!("ui_font = \"{FAMILY}\"\n{ORIGINAL}"));
    settled(&client, &base, &font).await;

    let response = save(&client, &base, json!("No Such Font")).await;
    assert_eq!(response.status(), 400);
    assert_eq!(fs::read_to_string(&config).unwrap(), format!("ui_font = \"{FAMILY}\"\n{ORIGINAL}"));

    fs::write(&config, format!("ui_font = 'Uninstalled'\n{ORIGINAL}")).unwrap();
    settled(&client, &base, &json!({ "family": "Uninstalled", "faces": [] })).await;

    let response = save(&client, &base, Value::Null).await;
    assert_eq!(response.status(), 200);
    assert_eq!(json(response).await, Value::Null);
    assert_eq!(fs::read_to_string(&config).unwrap(), ORIGINAL);
    settled(&client, &base, &Value::Null).await;
}

#[tokio::test]
async fn hashed_assets_are_immutable_and_gzipped_while_the_rest_revalidates() {
    let server = Server::start("assets-").await;
    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", server.port);
    let gzipped = |path: &str| client.get(format!("{base}{path}")).header("Accept-Encoding", "gzip").send();

    let index = get(&client, &format!("{base}/")).await;
    assert_eq!(index.headers()["cache-control"], "no-cache");
    let etag = index.headers()["etag"].clone();
    let html = index.text().await.unwrap();
    let bundle = html.split('"').find(|part| part.starts_with("/_app/immutable/") && part.ends_with(".js"))
        .expect("the index page names no hashed bundle file; build the frontend with pnpm -C web build");
    let revalidated = client.get(format!("{base}/")).header("If-None-Match", etag).send().await.unwrap();
    assert_eq!(revalidated.status(), 304);

    let bundle = gzipped(bundle).await.unwrap();
    assert_eq!(bundle.status(), 200);
    assert_eq!(bundle.headers()["cache-control"], "max-age=31536000, immutable");
    assert_eq!(bundle.headers()["content-encoding"], "gzip");

    let font = gzipped("/fonts/SymbolsNerdFontMono-Regular.woff2").await.unwrap();
    assert_eq!(font.status(), 200);
    assert_eq!(font.headers().get("content-encoding"), None);

    let missing = get(&client, &format!("{base}/_app/immutable/missing.js")).await;
    assert!(!missing.headers()["cache-control"].to_str().unwrap().contains("immutable"));
}
