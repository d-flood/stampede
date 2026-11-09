use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    env,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use axum::{
    extract::{Query, State},
    http::header,
    response::IntoResponse,
};
use base64ct::{Base64, Encoding};
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256, Sha512};
use tokio::{
    io::AsyncWriteExt,
    process::{Child, Command},
    sync::{broadcast, mpsc},
    task::AbortHandle,
    time::{Instant, sleep_until},
};
use tokio_util::{
    bytes::{Buf, BytesMut},
    codec::{AnyDelimiterCodec, Decoder, FramedRead},
};
use uuid::Uuid;

use crate::{AppState, device::{Outgoing, send}};

const LOG_LINES: usize = 1000;
const GRACE: Duration = Duration::from_secs(60);

const LANGUAGES: [&str; 6] = ["typescript", "svelte", "astro", "python", "ruff", "eslint"];

static PINS: LazyLock<HashMap<String, Pin>> =
    LazyLock::new(|| toml::from_str(include_str!("../language-servers.toml")).expect("invalid language-servers.toml"));

struct Npm {
    name: &'static str,
    package: &'static str,
    manifest: &'static str,
    lock: &'static str,
}

impl Npm {
    fn version(&self) -> String {
        let lock: Value = serde_json::from_str(self.lock).expect("invalid pinned package-lock.json");
        let version = lock["packages"][format!("node_modules/{}", self.package)]["version"].as_str();
        version.expect("pinned package-lock.json lacks its language server").to_owned()
    }
}

const SVELTE: Npm = Npm {
    name: "svelte",
    package: "svelte-language-server",
    manifest: include_str!("../language-servers/svelte/package.json"),
    lock: include_str!("../language-servers/svelte/package-lock.json"),
};

const ASTRO: Npm = Npm {
    name: "astro",
    package: "@astrojs/language-server",
    manifest: include_str!("../language-servers/astro/package.json"),
    lock: include_str!("../language-servers/astro/package-lock.json"),
};

const ESLINT: &str = include_str!("../language-servers/eslint.cjs");

const ESLINT_CONFIGS: [&str; 6] =
    ["eslint.config.js", "eslint.config.mjs", "eslint.config.cjs", "eslint.config.ts", "eslint.config.mts", "eslint.config.cts"];

#[derive(Deserialize)]
struct Pin {
    version: String,
    #[serde(rename = "linux-x64")]
    linux_x64: Download,
}

#[derive(Deserialize)]
struct Download {
    url: String,
    sha256: String,
    package: Option<String>,
    checksums: Option<String>,
}

pub fn parse(channel: &str) -> Option<(&str, &str)> {
    channel.strip_prefix("lsp:")?.split_once(':')
}

pub fn configured(language: &str, root: &Path) -> bool {
    match language {
        "ruff" => {
            ["ruff.toml", ".ruff.toml"].iter().any(|name| root.join(name).is_file())
                || fs::read_to_string(root.join("pyproject.toml"))
                    .ok()
                    .and_then(|text| text.parse::<toml::Table>().ok())
                    .is_some_and(|pyproject| pyproject.get("tool").and_then(|tool| tool.get("ruff")).is_some())
        }
        "eslint" => {
            ESLINT_CONFIGS.iter().any(|name| root.join(name).is_file()) && root.join("node_modules/eslint").is_dir()
        }
        _ => true,
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename = "lsp.switch", rename_all = "camelCase")]
pub struct Switch {
    pub project_id: String,
    pub on: bool,
}

pub struct LanguageServers {
    dir: PathBuf,
    file: PathBuf,
    cache: Option<PathBuf>,
    grace: Duration,
    switches: Mutex<BTreeSet<String>>,
    switched: broadcast::Sender<Switch>,
    logs: Mutex<HashMap<String, VecDeque<String>>>,
    installing: tokio::sync::Mutex<()>,
    client: reqwest::Client,
}

impl LanguageServers {
    pub fn load(data_dir: &Path) -> io::Result<Self> {
        let file = data_dir.join("language-server-switches.json");
        let switches = match fs::read(&file) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeSet::new(),
            bytes => serde_json::from_slice(&bytes?)?,
        };
        let grace = env::var("STAMPEDE_LSP_GRACE_MS").ok().and_then(|ms| ms.parse().ok());
        Ok(Self {
            dir: std::path::absolute(data_dir.join("language-servers"))?,
            file,
            cache: env::var_os("STAMPEDE_LSP_CACHE").map(PathBuf::from),
            grace: grace.map_or(GRACE, Duration::from_millis),
            switches: Mutex::new(switches),
            switched: broadcast::channel(64).0,
            logs: Mutex::new(HashMap::new()),
            installing: tokio::sync::Mutex::new(()),
            client: reqwest::Client::new(),
        })
    }

    pub fn is_on(&self, project_id: &str) -> bool {
        self.switches.lock().unwrap().contains(project_id)
    }

    pub fn switch(&self, project_id: &str, on: bool) {
        let mut switches = self.switches.lock().unwrap();
        let changed = if on { switches.insert(project_id.to_owned()) } else { switches.remove(project_id) };
        if !changed {
            return;
        }
        let tmp = self.file.with_extension("json.tmp");
        let saved = serde_json::to_vec(&*switches)
            .map_err(io::Error::from)
            .and_then(|bytes| fs::write(&tmp, bytes))
            .and_then(|()| fs::rename(&tmp, &self.file));
        if let Err(error) = saved {
            eprintln!("cannot save {}: {error}", self.file.display());
        }
        let _ = self.switched.send(Switch { project_id: project_id.to_owned(), on });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Switch> {
        self.switched.subscribe()
    }

    fn log(&self, channel: &str, line: String) {
        let mut logs = self.logs.lock().unwrap();
        let log = logs.entry(channel.to_owned()).or_default();
        if log.len() == LOG_LINES {
            log.pop_front();
        }
        log.push_back(line);
    }

    pub fn start(
        self: &Arc<Self>,
        channel: &str,
        root: &str,
        pins: BTreeMap<String, String>,
        outgoing: Outgoing,
    ) -> Option<Server> {
        let (_, language) = parse(channel)?;
        if !LANGUAGES.contains(&language) {
            return None;
        }
        let (input, pending) = mpsc::unbounded_channel();
        let task = tokio::spawn(Arc::clone(self).run(
            channel.to_owned(),
            language.to_owned(),
            PathBuf::from(root),
            pins,
            outgoing,
            pending,
        ));
        Some(Server { input, task: task.abort_handle() })
    }

    async fn run(
        self: Arc<Self>,
        channel: String,
        language: String,
        root: PathBuf,
        pins: BTreeMap<String, String>,
        outgoing: Outgoing,
        mut input: mpsc::UnboundedReceiver<Input>,
    ) {
        let state = |state: &str| send(&outgoing, json!({ "type": "lsp.state", "channel": channel, "state": state }));
        state("starting");
        let (mut child, mut options) = match self.spawn(&language, &pins, &root).await {
            Ok(spawned) => spawned,
            Err(error) => {
                self.log(&channel, error);
                return state("error");
            }
        };
        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = FramedRead::new(child.stdout.take().unwrap(), Frames);
        let mut stderr = FramedRead::new(child.stderr.take().unwrap(), AnyDelimiterCodec::new(b"\n".into(), Vec::new()));
        state("on");
        let mut idle = None;
        loop {
            tokio::select! {
                Some(input) = input.recv() => match input {
                    Input::Message(mut message) => {
                        if let Some(injected) = options.as_ref().and_then(|options| initialize(&message, options)) {
                            message = injected;
                            options = None;
                        }
                        let frame = format!("Content-Length: {}\r\n\r\n{message}", message.len());
                        let _ = stdin.write_all(frame.as_bytes()).await;
                    }
                    Input::Shown => idle = None,
                    Input::Hidden => idle = Some(Instant::now() + self.grace),
                },
                frame = stdout.next() => match frame {
                    Some(Ok(body)) => {
                        let message = String::from_utf8_lossy(&body);
                        send(&outgoing, json!({ "type": "lsp", "channel": channel, "message": message }));
                    }
                    Some(Err(error)) => {
                        self.log(&channel, format!("stampede: cannot read the server's output: {error}"));
                        break;
                    }
                    None => break,
                },
                Some(Ok(line)) = stderr.next() => self.log(&channel, String::from_utf8_lossy(&line).into_owned()),
                () = sleep_until(idle.unwrap_or_else(Instant::now)), if idle.is_some() => return state("off"),
            }
        }
        let _ = child.start_kill();
        while let Some(Ok(line)) = stderr.next().await {
            self.log(&channel, String::from_utf8_lossy(&line).into_owned());
        }
        match child.wait().await {
            Ok(status) => self.log(&channel, format!("stampede: the server exited ({status})")),
            Err(error) => self.log(&channel, format!("stampede: the server exited: {error}")),
        }
        state("error");
    }

    async fn spawn(
        self: &Arc<Self>,
        language: &str,
        pins: &BTreeMap<String, String>,
        root: &Path,
    ) -> Result<(Child, Option<Value>), String> {
        let install = |name| detached(Arc::clone(self).install(name, pins.get(name).cloned()));
        let mut options = None;
        let (program, args): (_, Vec<OsString>) = match language {
            "typescript" => (install("typescript").await?.join("lib/tsc"), vec!["--lsp".into(), "--stdio".into()]),
            "svelte" | "astro" => {
                let (npm, server) = match language {
                    "svelte" => (&SVELTE, "svelte-language-server/bin/server.js"),
                    _ => (&ASTRO, "@astrojs/language-server/bin/nodeServer.js"),
                };
                let node = install("node").await?;
                let dir = detached(Arc::clone(self).install_npm(npm, node.clone(), pins.get(npm.name).cloned())).await?;
                if language == "astro" {
                    options = Some(json!({ "typescript": { "tsdk": dir.join("node_modules/typescript/lib") } }));
                }
                (node.join("bin/node"), vec![dir.join("node_modules").join(server).into(), "--stdio".into()])
            }
            "python" => (install("ty").await?.join("ty"), vec!["server".into()]),
            "ruff" => (install("ruff").await?.join("ruff"), vec!["server".into()]),
            "eslint" => (install("node").await?.join("bin/node"), vec![self.eslint()?.into()]),
            _ => return Err(format!("stampede: no language server for {language}")),
        };
        Command::new(&program)
            .args(args)
            .current_dir(root)
            .env_remove("PATH")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map(|child| (child, options))
            .map_err(|error| format!("stampede: cannot start {}: {error}", program.display()))
    }

    fn eslint(&self) -> Result<PathBuf, String> {
        let path = self.dir.join("eslint").join(format!("{}.cjs", &hex(&Sha256::digest(ESLINT))[..16]));
        if path.is_file() {
            return Ok(path);
        }
        let tmp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
        fs::create_dir_all(path.parent().unwrap())
            .and_then(|()| fs::write(&tmp, ESLINT))
            .and_then(|()| fs::rename(&tmp, &path))
            .map_err(|error| format!("stampede: cannot write {}: {error}", path.display()))?;
        Ok(path)
    }

    async fn install(self: Arc<Self>, name: &'static str, version: Option<String>) -> Result<PathBuf, String> {
        let pin = PINS.get(name).ok_or_else(|| format!("stampede: no language server is pinned for {name}"))?;
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err("stampede: pinned language servers are built for Linux x64 only".to_owned());
        }
        let version = version.unwrap_or_else(|| pin.version.clone());
        let dir = self.dir.join(name).join(&version);
        if dir.is_dir() {
            return Ok(dir);
        }
        let _installing = self.installing.lock().await;
        if dir.is_dir() {
            return Ok(dir);
        }
        let download = &pin.linux_x64;
        let url = download.url.replace("{version}", &version);
        let tarball = if version == pin.version {
            self.fetch(&url, |bytes| hex(&Sha256::digest(bytes)) == download.sha256).await?
        } else if let Some(checksums) = &download.checksums {
            let checksums = checksums.replace("{version}", &version);
            let file = url.rsplit('/').next().unwrap_or_default();
            let listing = self.get(&checksums).await?;
            let expected = String::from_utf8_lossy(&listing)
                .lines()
                .filter_map(|line| line.split_once(char::is_whitespace))
                .find(|(_, listed)| listed.trim_start().trim_start_matches('*') == file)
                .map(|(sum, _)| sum.to_ascii_lowercase())
                .ok_or_else(|| format!("stampede: {checksums} lists no checksum for {file}"))?;
            self.fetch(&url, |bytes| hex(&Sha256::digest(bytes)) == expected).await?
        } else if let Some(package) = &download.package {
            let url = format!("https://registry.npmjs.org/{package}/{version}");
            let meta: Value = serde_json::from_slice(&self.get(&url).await?).map_err(|error| format!("{url}: {error}"))?;
            let (Some(tarball), Some(integrity)) = (meta["dist"]["tarball"].as_str(), meta["dist"]["integrity"].as_str())
            else {
                return Err(format!("stampede: {url} lists no tarball with an integrity hash"));
            };
            let expected = integrity
                .strip_prefix("sha512-")
                .and_then(|digest| Base64::decode_vec(digest).ok())
                .ok_or_else(|| format!("stampede: {url} has an integrity hash that is not sha512: {integrity}"))?;
            self.fetch(tarball, |bytes| Sha512::digest(bytes)[..] == expected[..]).await?
        } else {
            return Err(format!("stampede: {name} takes no version override"));
        };
        let unpacked = dir.clone();
        tokio::task::spawn_blocking(move || unpack(&tarball, &unpacked))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| format!("stampede: cannot unpack into {}: {error}", dir.display()))?;
        Ok(dir)
    }

    async fn install_npm(self: Arc<Self>, npm: &'static Npm, node: PathBuf, version: Option<String>) -> Result<PathBuf, String> {
        let pinned = npm.version();
        let version = version.unwrap_or_else(|| pinned.clone());
        let core = version.split(['-', '+']).next().unwrap_or_default().split('.').collect::<Vec<_>>();
        let valid = core.len() == 3
            && core.iter().all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
            && version.bytes().all(|byte| byte.is_ascii_alphanumeric() || b".-+".contains(&byte));
        if !valid {
            return Err(format!("stampede: the {} pin must be an exact version", npm.name));
        }
        let dir = self.dir.join(npm.name).join(&version);
        if dir.is_dir() {
            return Ok(dir);
        }
        let _installing = self.installing.lock().await;
        if dir.is_dir() {
            return Ok(dir);
        }
        let tmp = self.dir.join(npm.name).join(format!(".{}", Uuid::new_v4()));
        let scratch = tmp.with_extension("npm");
        let installed = async {
            let written = fs::create_dir_all(&tmp)
                .and_then(|()| fs::write(tmp.join("package.json"), npm.manifest))
                .and_then(|()| fs::write(tmp.join("package-lock.json"), npm.lock));
            written.map_err(|error| format!("stampede: cannot write {}: {error}", tmp.display()))?;
            if version != pinned {
                let mut manifest: Value = serde_json::from_str(npm.manifest).map_err(|error| error.to_string())?;
                manifest["dependencies"][npm.package] = json!(version);
                fs::write(tmp.join("package.json"), serde_json::to_vec(&manifest).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
                let output = Command::new(node.join("bin/node"))
                    .arg(node.join("lib/node_modules/npm/bin/npm-cli.js"))
                    .args(["install", "--package-lock-only", "--ignore-scripts", "--no-audit", "--no-fund", "--no-update-notifier"])
                    .args(["--userconfig", "/dev/null", "--cache"])
                    .arg(self.cache.as_ref().map_or_else(|| scratch.clone(), |cache| cache.join("npm")))
                    .current_dir(&tmp)
                    .env_remove("PATH")
                    .stdin(Stdio::null())
                    .output()
                    .await
                    .map_err(|error| format!("stampede: cannot resolve the {} pin: {error}", npm.name))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!("stampede: cannot resolve the {} pin: {stderr}", npm.name));
                }
            }
            let output = Command::new(node.join("bin/node"))
                .arg(node.join("lib/node_modules/npm/bin/npm-cli.js"))
                .args(["ci", "--ignore-scripts", "--no-audit", "--no-fund", "--no-update-notifier"])
                .args(["--userconfig", "/dev/null", "--cache"])
                .arg(self.cache.as_ref().map_or_else(|| scratch.clone(), |cache| cache.join("npm")))
                .current_dir(&tmp)
                .env_remove("PATH")
                .stdin(Stdio::null())
                .output()
                .await
                .map_err(|error| format!("stampede: cannot run npm ci: {error}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("stampede: npm ci failed ({}):\n{stderr}", output.status));
            }
            fs::rename(&tmp, &dir).map_err(|error| format!("stampede: cannot install into {}: {error}", dir.display()))
        }
        .await;
        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::remove_dir_all(&scratch);
        installed.map(|()| dir)
    }

    async fn fetch(&self, url: &str, verify: impl Fn(&[u8]) -> bool) -> Result<Vec<u8>, String> {
        let cached = self.cache.as_ref().zip(url.rsplit('/').next()).map(|(cache, name)| cache.join(name));
        if let Some(path) = &cached
            && let Ok(bytes) = fs::read(path)
            && verify(&bytes)
        {
            return Ok(bytes);
        }
        let bytes = self.get(url).await?;
        if !verify(&bytes) {
            return Err(format!("stampede: {url} does not match its pinned checksum"));
        }
        if let Some(path) = cached {
            let tmp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
            let saved = path
                .parent()
                .map_or(Ok(()), fs::create_dir_all)
                .and_then(|()| fs::write(&tmp, &bytes))
                .and_then(|()| fs::rename(&tmp, &path));
            if let Err(error) = saved {
                eprintln!("cannot cache {}: {error}", path.display());
            }
        }
        Ok(bytes)
    }

    async fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        let response = self.client.get(url).send().await.and_then(|response| response.error_for_status());
        let bytes = match response {
            Ok(response) => response.bytes().await,
            Err(error) => Err(error),
        };
        bytes.map(Vec::from).map_err(|error| format!("stampede: cannot download {url}: {error}"))
    }
}

fn unpack(tarball: &[u8], dir: &Path) -> io::Result<()> {
    let parent = dir.parent().unwrap();
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".{}", Uuid::new_v4()));
    tar::Archive::new(GzDecoder::new(tarball)).unpack(&tmp)?;
    let mut entries = fs::read_dir(&tmp)?;
    let renamed = match (entries.next(), entries.next()) {
        (Some(Ok(top)), None) => fs::rename(top.path(), dir),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "the tarball has no single top-level directory")),
    };
    fs::remove_dir_all(&tmp)?;
    renamed
}

fn initialize(message: &str, options: &Value) -> Option<String> {
    let mut request: Value = serde_json::from_str(message).ok()?;
    if request["method"] != "initialize" {
        return None;
    }
    request["params"]["initializationOptions"] = options.clone();
    Some(request.to_string())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

async fn detached<T: Send + 'static>(install: impl Future<Output = Result<T, String>> + Send + 'static) -> Result<T, String> {
    // Detached, so that a stop mid-install never leaves half an install.
    tokio::spawn(install).await.map_err(|error| error.to_string())?
}

pub enum Input {
    Message(String),
    Shown,
    Hidden,
}

pub struct Server {
    input: mpsc::UnboundedSender<Input>,
    task: AbortHandle,
}

impl Server {
    pub fn running(&self) -> bool {
        !self.task.is_finished()
    }

    pub fn send(&self, input: Input) {
        let _ = self.input.send(input);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Frames;

impl Decoder for Frames {
    type Item = BytesMut;
    type Error = io::Error;

    fn decode(&mut self, buffer: &mut BytesMut) -> io::Result<Option<BytesMut>> {
        let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else {
            return Ok(None);
        };
        let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
        let headers = std::str::from_utf8(&buffer[..end]).map_err(|_| invalid("headers are not UTF-8"))?;
        let length = headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse::<usize>().ok())
            .ok_or_else(|| invalid("no Content-Length header"))?;
        if buffer.len() < end + 4 + length {
            buffer.reserve(end + 4 + length - buffer.len());
            return Ok(None);
        }
        buffer.advance(end + 4);
        Ok(Some(buffer.split_to(length)))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogQuery {
    project_id: String,
    language: String,
}

pub async fn log(State(state): State<AppState>, Query(query): Query<LogQuery>) -> impl IntoResponse {
    let channel = format!("lsp:{}:{}", query.project_id, query.language);
    let logs = state.languages.logs.lock().unwrap();
    let lines = logs.get(&channel).map(|log| log.iter().map(|line| format!("{line}\n")).collect::<String>());
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], lines.unwrap_or_default())
}
