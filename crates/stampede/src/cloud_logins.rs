use std::{collections::BTreeMap, fs, io::{self, Write}, os::unix::fs::OpenOptionsExt, path::{Path, PathBuf}, sync::OnceLock, time::Duration};

use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use tokio::{process::Command, sync::watch, time::timeout};
use uuid::Uuid;

use crate::{AppState, cloud, files};

const CLAUDE_TOKEN_ENV: &str = "CLAUDE_CODE_OAUTH_TOKEN";
pub const CLAUDE_LOGIN_HINT: &str = "Claude Code has no login on this Cloud Host. Add a token in Settings › Cloud Hosts.";

#[derive(Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Logins {
    pub claude: Option<String>,
    pub gh: Option<String>,
}

fn path(data: &Path) -> PathBuf { data.join("cloud-logins.json") }

impl Logins {
    pub fn read(data: &Path) -> io::Result<Self> {
        match fs::read(path(data)) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, data: &Path) -> io::Result<()> {
        let temporary = data.join(format!(".cloud-logins-{}", Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary)?;
            file.write_all(&serde_json::to_vec(self).map_err(io::Error::other)?)?;
            fs::rename(&temporary, path(data))
        })();
        if result.is_err() { let _ = fs::remove_file(&temporary); }
        result
    }

    pub fn environment(&self) -> BTreeMap<String, String> {
        [(CLAUDE_TOKEN_ENV, &self.claude), ("GH_TOKEN", &self.gh)].into_iter()
            .filter_map(|(name, value)| value.as_ref().map(|value| (name.to_owned(), value.clone()))).collect()
    }

    pub fn has_claude(env: &BTreeMap<String, String>) -> bool {
        env.get(CLAUDE_TOKEN_ENV).is_some_and(|value| !value.is_empty())
    }

    pub fn redact(env: &BTreeMap<String, String>, mut text: String) -> String {
        for key in [CLAUDE_TOKEN_ENV, "GH_TOKEN"] {
            if let Some(secret) = env.get(key).filter(|value| !value.is_empty()) { text = text.replace(secret, "[redacted]"); }
        }
        text
    }

    pub async fn configure_git(&self) -> bool {
        let Some(token) = &self.gh else { return true; };
        let output = timeout(Duration::from_secs(10), Command::new("gh").args(["auth", "setup-git", "--hostname", "github.com", "--force"])
            .env("GH_TOKEN", token).kill_on_drop(true).output()).await;
        if !matches!(output, Ok(Ok(output)) if output.status.success()) { return false; }
        for prefix in ["git@github.com:", "ssh://git@github.com/"] {
            let output = timeout(Duration::from_secs(10), Command::new("git")
                .args(["config", "--global", "--fixed-value", "--replace-all", "url.https://github.com/.insteadOf", prefix, prefix])
                .kill_on_drop(true).output()).await;
            if !matches!(output, Ok(Ok(output)) if output.status.success()) { return false; }
        }
        true
    }

    pub async fn home(data: &Path) -> Self {
        let mut logins = Self::read(data).unwrap_or_default();
        let output = timeout(Duration::from_secs(10), Command::new("gh").args(["auth", "token"]).kill_on_drop(true).output()).await;
        logins.gh = match output {
            Ok(Ok(output)) if output.status.success() => String::from_utf8(output.stdout).ok().map(|token| token.trim().to_owned()).filter(|token| !token.is_empty()),
            _ => None,
        };
        logins
    }
}

static CLOUD_DATA: OnceLock<PathBuf> = OnceLock::new();

pub fn set_cloud_data(data: &Path) { let _ = CLOUD_DATA.set(data.to_owned()); }

pub fn github_environment() -> Option<(&'static str, String)> {
    let logins = Logins::read(CLOUD_DATA.get()?).ok()?;
    logins.gh.map(|token| ("GH_TOKEN", token))
}

pub struct Store {
    pub data: PathBuf,
    changed: watch::Sender<u64>,
}

impl Store {
    pub fn new(data: PathBuf) -> Self { Self { data, changed: watch::Sender::default() } }
    pub fn subscribe(&self) -> watch::Receiver<u64> { self.changed.subscribe() }
    pub fn save(&self, logins: &Logins) -> io::Result<()> {
        logins.save(&self.data)?;
        self.changed.send_modify(|revision| *revision += 1);
        Ok(())
    }
}

pub async fn set_claude(State(state): State<AppState>, Json(token): Json<Option<String>>) -> Result<StatusCode, files::Failure> {
    if state.cloud.is_some() { return Err((StatusCode::FORBIDDEN, cloud::SETTINGS_FROM_HOME.to_owned())); }
    let logins = Logins { claude: token.map(|token| token.trim().to_owned()).filter(|token| !token.is_empty()), gh: None };
    state.logins.save(&logins).map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Could not save Claude Code login.".to_owned()))?;
    Ok(StatusCode::NO_CONTENT)
}
