use std::{
    collections::HashSet,
    fs, io,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use axum::{Json, extract::State, http::StatusCode};
use base64ct::{Base64UrlUnpadded, Encoding};
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;
use web_push_native::{
    Auth, WebPushBuilder,
    jwt_simple::algorithms::{ECDSAP256PublicKeyLike, ES256KeyPair},
    p256::PublicKey,
};

use crate::{
    AppState,
    attention::{Attention, Feed},
};

const CONTACT: &str = "mailto:oikonomos@localhost";

pub struct Push {
    file: PathBuf,
    key: ES256KeyPair,
    subscriptions: Mutex<Vec<Subscription>>,
    client: reqwest::Client,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    browser_id: String,
    subscription: Target,
}

#[derive(Clone, Serialize, Deserialize)]
struct Target {
    endpoint: String,
    keys: Keys,
}

#[derive(Clone, Serialize, Deserialize)]
struct Keys {
    p256dh: String,
    auth: String,
}

impl Push {
    pub fn load(data_dir: &Path) -> io::Result<Self> {
        let dir = data_dir.join("push");
        fs::create_dir_all(&dir)?;
        let path = dir.join("vapid.pem");
        let key = match fs::read_to_string(&path) {
            Ok(pem) => ES256KeyPair::from_pem(&pem).map_err(io::Error::other)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let key = ES256KeyPair::generate();
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&path)?
                    .write_all(key.to_pem().map_err(io::Error::other)?.as_bytes())?;
                key
            }
            Err(error) => return Err(error),
        };
        let file = dir.join("subscriptions.json");
        let subscriptions = match fs::read(&file) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            bytes => serde_json::from_slice(&bytes?)?,
        };
        Ok(Self { file, key, subscriptions: Mutex::new(subscriptions), client: reqwest::Client::new() })
    }

    fn public_key(&self) -> String {
        Base64UrlUnpadded::encode_string(&self.key.public_key().public_key().to_bytes_uncompressed())
    }

    fn subscribe(&self, subscription: Subscription) {
        let mut subscriptions = self.subscriptions.lock().unwrap();
        subscriptions.retain(|other| {
            other.browser_id != subscription.browser_id
                && other.subscription.endpoint != subscription.subscription.endpoint
        });
        subscriptions.push(subscription);
        self.save(&subscriptions);
    }

    fn save(&self, subscriptions: &[Subscription]) {
        let tmp = self.file.with_extension("json.tmp");
        let saved = serde_json::to_vec_pretty(subscriptions)
            .map_err(io::Error::from)
            .and_then(|bytes| fs::write(&tmp, bytes))
            .and_then(|()| fs::rename(&tmp, &self.file));
        if let Err(error) = saved {
            eprintln!("cannot save {}: {error}", self.file.display());
        }
    }

    async fn send(&self, item: &Attention, except: &HashSet<String>) {
        let payload = serde_json::to_vec(item).unwrap();
        let targets: Vec<Target> = self
            .subscriptions
            .lock()
            .unwrap()
            .iter()
            .filter(|subscription| !except.contains(&subscription.browser_id))
            .map(|subscription| subscription.subscription.clone())
            .collect();
        for target in targets {
            let Some(builder) = target.builder() else { continue };
            let mut request = match builder.with_vapid(&self.key, CONTACT).build(payload.clone()) {
                Ok(request) => request,
                Err(error) => {
                    eprintln!("cannot build a push to {}: {error}", target.endpoint);
                    continue;
                }
            };
            request.headers_mut().insert("urgency", HeaderValue::from_static("high"));
            let sent = match reqwest::Request::try_from(request) {
                Ok(request) => self.client.execute(request).await,
                Err(error) => Err(error),
            };
            match sent.map(|response| response.status()) {
                Ok(status) if status.is_success() => {}
                Ok(status) => eprintln!("push to {} failed: {status}", target.endpoint),
                Err(error) => eprintln!("push to {} failed: {error}", target.endpoint),
            }
        }
    }
}

impl Target {
    fn builder(&self) -> Option<WebPushBuilder> {
        let decode = |text: &str| Base64UrlUnpadded::decode_vec(text.trim_end_matches('=')).ok();
        Some(WebPushBuilder::new(
            self.endpoint.parse().ok()?,
            PublicKey::from_sec1_bytes(&decode(&self.keys.p256dh)?).ok()?,
            Auth::from_exact_iter(decode(&self.keys.auth)?)?,
        ))
    }
}

pub fn spawn(push: Arc<Push>, attention: &Feed) {
    let mut calls = attention.calls();
    tokio::spawn(async move {
        loop {
            match calls.recv().await {
                Ok((item, except)) => {
                    let push = Arc::clone(&push);
                    tokio::spawn(async move { push.send(&item, &except).await });
                }
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            }
        }
    });
}

pub async fn key(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "key": state.push.public_key() }))
}

pub async fn subscribe(State(state): State<AppState>, Json(subscription): Json<Subscription>) -> StatusCode {
    if subscription.subscription.builder().is_none() {
        return StatusCode::BAD_REQUEST;
    }
    state.push.subscribe(subscription);
    StatusCode::NO_CONTENT
}
