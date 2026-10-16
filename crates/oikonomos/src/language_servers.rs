use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    env, fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use axum::{
    extract::{Query, State, ws::Message},
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

use crate::{AppState, device::send};

const LOG_LINES: usize = 1000;
const GRACE: Duration = Duration::from_secs(60);

static PINS: LazyLock<HashMap<String, Pin>> =
    LazyLock::new(|| toml::from_str(include_str!("../language-servers.toml")).expect("invalid language-servers.toml"));

#[derive(Deserialize)]
struct Pin {
    version: String,
    #[serde(rename = "linux-x64")]
    linux_x64: Download,
}

#[derive(Deserialize)]
struct Download {
    package: String,
    url: String,
    sha256: String,
}

pub fn parse(channel: &str) -> Option<(&str, &str)> {
    channel.strip_prefix("lsp:")?.split_once(':')
}

fn command(language: &str) -> Option<(&'static str, &'static [&'static str])> {
    match language {
        "typescript" => Some(("lib/tsc", &["--lsp", "--stdio"])),
        _ => None,
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
        let grace = env::var("OIKONOMOS_LSP_GRACE_MS").ok().and_then(|ms| ms.parse().ok());
        Ok(Self {
            dir: std::path::absolute(data_dir.join("language-servers"))?,
            file,
            cache: env::var_os("OIKONOMOS_LSP_CACHE").map(PathBuf::from),
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
        version: Option<String>,
        outgoing: mpsc::UnboundedSender<Message>,
    ) -> Option<Server> {
        let (_, language) = parse(channel)?;
        command(language)?;
        PINS.get(language)?;
        let (input, pending) = mpsc::unbounded_channel();
        let task = tokio::spawn(Arc::clone(self).run(
            channel.to_owned(),
            language.to_owned(),
            PathBuf::from(root),
            version,
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
        version: Option<String>,
        outgoing: mpsc::UnboundedSender<Message>,
        mut input: mpsc::UnboundedReceiver<Input>,
    ) {
        let state = |state: &str| send(&outgoing, json!({ "type": "lsp.state", "channel": channel, "state": state }));
        state("starting");
        let mut child = match self.spawn(&language, version, &root).await {
            Ok(child) => child,
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
                    Input::Message(message) => {
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
                        self.log(&channel, format!("oikonomos: cannot read the server's output: {error}"));
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
            Ok(status) => self.log(&channel, format!("oikonomos: the server exited ({status})")),
            Err(error) => self.log(&channel, format!("oikonomos: the server exited: {error}")),
        }
        state("error");
    }

    async fn spawn(self: &Arc<Self>, language: &str, version: Option<String>, root: &Path) -> Result<Child, String> {
        let installer = Arc::clone(self);
        let name = language.to_owned();
        // Detached, so that a stop mid-download never leaves half an install.
        let dir = tokio::spawn(async move { installer.install(&name, version).await })
            .await
            .map_err(|error| error.to_string())??;
        let (program, args) = command(language).ok_or_else(|| format!("oikonomos: no language server for {language}"))?;
        let program = dir.join(program);
        Command::new(&program)
            .args(args)
            .current_dir(root)
            .env_remove("PATH")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("oikonomos: cannot start {}: {error}", program.display()))
    }

    async fn install(&self, name: &str, version: Option<String>) -> Result<PathBuf, String> {
        let pin = PINS.get(name).ok_or_else(|| format!("oikonomos: no language server is pinned for {name}"))?;
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err("oikonomos: pinned language servers are built for Linux x64 only".to_owned());
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
        let tarball = if version == pin.version {
            self.fetch(&download.url, |bytes| hex(&Sha256::digest(bytes)) == download.sha256).await?
        } else {
            let url = format!("https://registry.npmjs.org/{}/{version}", download.package);
            let meta: Value = serde_json::from_slice(&self.get(&url).await?).map_err(|error| format!("{url}: {error}"))?;
            let (Some(tarball), Some(integrity)) = (meta["dist"]["tarball"].as_str(), meta["dist"]["integrity"].as_str())
            else {
                return Err(format!("oikonomos: {url} lists no tarball with an integrity hash"));
            };
            let expected = integrity
                .strip_prefix("sha512-")
                .and_then(|digest| Base64::decode_vec(digest).ok())
                .ok_or_else(|| format!("oikonomos: {url} has an integrity hash that is not sha512: {integrity}"))?;
            self.fetch(tarball, |bytes| Sha512::digest(bytes)[..] == expected[..]).await?
        };
        let unpacked = dir.clone();
        tokio::task::spawn_blocking(move || unpack(&tarball, &unpacked))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| format!("oikonomos: cannot unpack into {}: {error}", dir.display()))?;
        Ok(dir)
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
            return Err(format!("oikonomos: {url} does not match its pinned checksum"));
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
        bytes.map(Vec::from).map_err(|error| format!("oikonomos: cannot download {url}: {error}"))
    }
}

fn unpack(tarball: &[u8], dir: &Path) -> io::Result<()> {
    let parent = dir.parent().unwrap();
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".{}", Uuid::new_v4()));
    tar::Archive::new(GzDecoder::new(tarball)).unpack(&tmp)?;
    let renamed = fs::rename(tmp.join("package"), dir);
    fs::remove_dir_all(&tmp)?;
    renamed
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
