mod common;

use std::{fs, time::Duration};

use common::Server;
use serde_json::Value;

async fn get(client: &reqwest::Client, url: &str) -> Value {
    let response = client.get(url).send().await.unwrap();
    serde_json::from_str(&response.text().await.unwrap()).unwrap()
}

async fn select(client: &reqwest::Client, api: &str, name: &str) -> reqwest::Response {
    client.put(format!("{api}/theme"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(name).unwrap())
        .send().await.unwrap()
}

#[tokio::test]
async fn themes_are_listed_saved_and_reloaded_without_rewriting_other_settings() {
    let original = "# café\nroots = []\nrig = []\n\"theme\" = '''light:Alabaster,dark:Dracula''' # keep this\n\n[editor]\nkeymap = 'vim'\n";
    let mut server = Server::with_config("themes-", original).await;
    let user = server.root.path().join("home/.config/ghostty/themes");
    fs::create_dir_all(&user).unwrap();
    fs::write(user.join("Custom Theme"), "background = #123456\nforeground = #abcdef\n").unwrap();
    fs::write(user.join("Dracula"), "background = #112233\n").unwrap();
    let client = reqwest::Client::new();
    let api = format!("http://127.0.0.1:{}/api", server.port);
    let names: Vec<String> = serde_json::from_value(get(&client, &format!("{api}/themes")).await).unwrap();
    assert!(names.len() > 100);
    assert!(names.iter().any(|name| name == "Custom Theme"));
    assert_eq!(names.iter().filter(|name| *name == "Dracula").count(), 1);
    assert!(names.windows(2).all(|pair| pair[0] < pair[1]));

    let saved = get(&client, &format!("{api}/theme")).await;
    let preview = get(&client, &format!("{api}/themes/Custom%20Theme")).await;
    assert_eq!(preview["name"], "Custom Theme");
    assert_eq!(preview["dark"]["background"], "#123456");
    assert_eq!(get(&client, &format!("{api}/theme")).await, saved);
    assert_eq!(fs::read_to_string(server.root.path().join("config.toml")).unwrap(), original);
    let invalid = client.get(format!("{api}/themes/.invalid")).send().await.unwrap();
    assert_eq!(invalid.status(), 400);

    let response = select(&client, &api, "Dracula").await;
    assert!(response.status().is_success());
    let selected: Value = serde_json::from_str(&response.text().await.unwrap()).unwrap();
    assert_eq!(selected["name"], "Dracula");
    assert_eq!(selected["light"]["background"], "#112233");
    assert_eq!(selected["dark"]["background"], "#112233");
    let expected = original.replace("'''light:Alabaster,dark:Dracula'''", "\"Dracula\"");
    let config = server.root.path().join("config.toml");
    assert_eq!(fs::read_to_string(&config).unwrap(), expected);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = get(&client, &format!("{api}/theme")).await;
            if current == selected { break; }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }).await.unwrap();

    let response = select(&client, &api, "../invalid").await;
    assert_eq!(response.status(), 400);
    assert_eq!(fs::read_to_string(&config).unwrap(), expected);

    server.restart().await;
    let reloaded = get(&client, &format!("{api}/theme")).await;
    assert_eq!(reloaded, selected);

    fs::write(&config, "# preserve\n[editor]\nkeymap = 'vim'\n").unwrap();
    let response = select(&client, &api, "Custom Theme").await;
    assert!(response.status().is_success());
    assert_eq!(fs::read_to_string(&config).unwrap(), "theme = \"Custom Theme\"\n# preserve\n[editor]\nkeymap = 'vim'\n");

    let invalid = "[editor\n";
    fs::write(&config, invalid).unwrap();
    let response = select(&client, &api, "Dracula").await;
    assert_eq!(response.status(), 400);
    assert_eq!(fs::read_to_string(&config).unwrap(), invalid);
}
