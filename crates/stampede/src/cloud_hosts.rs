use std::{collections::HashMap, convert::Infallible, fs, io, path::{Path as FsPath, PathBuf}, sync::{self, Arc}, time::Duration};

use axum::{body::Body, extract::{FromRequestParts, Request, State, WebSocketUpgrade}, http::{StatusCode, header}, response::{IntoResponse, Response}};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{sync::{Mutex, Notify, mpsc, watch}, time::{sleep, timeout}};
use tokio_tungstenite::{connect_async, tungstenite::{Message, client::IntoClientRequest}};
use uuid::Uuid;

use crate::{AppState, Creation, browser, cloud::{self, Forward, Service}, cloud_setup, cloud_link, config, device::Devices, files, git, projects::Project, runs};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Setup {
    Creating,
    Running,
    Ready,
    Failed,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CloudHost {
    pub id: String,
    pub service: Service,
    pub parent: String,
    pub branch: String,
    pub existing: bool,
    pub project: String,
    pub path: String,
    pub setup: Setup,
    #[serde(default)]
    pub setup_terminal: Option<String>,
    pub tip: Option<String>,
    pub in_sync: bool,
}

pub struct CloudHosts {
    home_id: String,
    file: PathBuf,
    records: watch::Sender<Vec<CloudHost>>,
    creating: Mutex<()>,
    reaches: sync::Mutex<HashMap<String, Arc<Reach>>>,
    client: reqwest::Client,
}

/// How a Cloud Host is reached now, and by whom.
#[derive(Default)]
pub struct Reach {
    pub forward: Mutex<Option<Forward>>,
    /// The Devices connected to it through the proxy.
    pub devices: Devices,
    /// Wakes the Home link when a Device reaches the Cloud Host, if only for one request.
    pub reached: Notify,
}

impl CloudHosts {
    pub fn load(data_dir: &FsPath) -> io::Result<Self> {
        fs::create_dir_all(data_dir)?;
        let identity = data_dir.join("host-id");
        let home_id = match fs::read_to_string(&identity) {
            Ok(id) => id,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let id = Uuid::new_v4().simple().to_string();
                fs::write(identity, &id)?;
                id
            }
            Err(error) => return Err(error),
        };
        let file = data_dir.join("cloud-hosts.json");
        let records = match fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        Ok(Self {
            home_id, file, records: watch::Sender::new(records), creating: Mutex::new(()), reaches: sync::Mutex::default(),
            client: reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).build().map_err(io::Error::other)?,
        })
    }

    pub fn subscribe(&self) -> watch::Receiver<Vec<CloudHost>> {
        self.records.subscribe()
    }

    pub fn projects(&self) -> Vec<Value> {
        self.records.borrow().iter().map(|host| json!({
            "id": host.project, "host": host.id, "path": host.path,
            "worktree": { "parent": host.parent, "branch": host.branch },
            "cloud": { "setup": host.setup, "state": "awake", "reason": null, "inSync": host.in_sync, "setupTerminal": host.setup_terminal },
        })).collect()
    }

    fn record(&self, host: CloudHost) -> Result<(), String> {
        let mut records = self.records.borrow().clone();
        records.push(host);
        self.save(records)
    }

    pub fn set_setup(&self, id: &str, setup: Setup, terminal: Option<String>) -> Result<(), String> {
        let mut records = self.records.borrow().clone();
        let Some(host) = records.iter_mut().find(|host| host.id == id) else { return Ok(()) };
        host.setup = setup;
        host.setup_terminal = terminal.or(host.setup_terminal.take());
        self.save(records)
    }

    fn save(&self, records: Vec<CloudHost>) -> Result<(), String> {
        let temporary = self.file.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&records).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
        fs::rename(temporary, &self.file).map_err(|error| error.to_string())?;
        self.records.send_replace(records);
        Ok(())
    }

    pub fn reach(&self, id: &str) -> Arc<Reach> {
        Arc::clone(self.reaches.lock().unwrap().entry(id.to_owned()).or_default())
    }

    pub async fn port(&self, id: &str) -> Result<u16, String> {
        let reach = self.reach(id);
        let mut held = reach.forward.lock().await;
        if let Some(forward) = held.as_mut() && forward.running() {
            return Ok(forward.port);
        }
        *held = None;
        let remote_port = remote_port(id).await?;
        let forward = cloud::forward(id, remote_port).await?;
        Ok(held.insert(forward).port)
    }
    pub async fn replace_build(&self, id: &str) -> Result<(), String> {
        let reach = self.reach(id);
        let mut forward = reach.forward.lock().await;
        let home = remote(id, &["sh", "-c", "printf '%s' \"$HOME\""], None).await?;
        let binary = format!("{home}/.local/bin/stampede");
        let next = format!("{binary}.next");
        cloud::push(id, &std::env::current_exe().map_err(|error| error.to_string())?, FsPath::new(&next)).await?;
        remote(id, &["chmod", "+x", &next], None).await?;
        remote(id, &["mv", "-f", &next, &binary], None).await?;
        remote(id, &["rm", "-f", &format!("{home}/.local/share/stampede/port")], None).await?;
        forward.take();
        remote(id, &["sprite-env", "services", "restart", "stampede"], None).await?;
        Ok(())
    }
}

async fn remote(id: &str, args: &[&str], stdin: Option<&[u8]>) -> Result<String, String> {
    let output = cloud::checked(cloud::exec(id, args, &[], stdin).await?)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
}

async fn remote_port(id: &str) -> Result<u16, String> {
    remote(id, &["sh", "-c", "cat \"$HOME/.local/share/stampede/port\""], None).await?.parse::<u16>().map_err(|error| error.to_string())
}

pub async fn create(state: AppState, project_id: String, creation: Creation, service: Service) -> Result<Response, files::Failure> {
    let refuse = |error: String| (StatusCode::BAD_REQUEST, error);
    let project = state.runs.repository_root(&project_id).await.map_err(refuse)?;
    let tip = git::run(FsPath::new(&project.path), &["rev-parse", "--verify", &format!("{}^{{commit}}", creation.from)]).await.map_err(refuse)?;
    let setup = cloud_setup::read(FsPath::new(&project.path), &tip, &creation.from).await.map_err(refuse)?;
    let (sender, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let result = provision(&state, project, tip, setup, creation, service, &sender).await;
        let _ = sender.send(match result {
            Ok(host) => json!({ "projectId": host.project, "host": host.id }),
            Err(error) => json!({ "error": error }),
        });
    });
    let lines = futures_util::stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|line| (Ok::<_, Infallible>(format!("{line}\n")), receiver))
    });
    Ok(([(header::CONTENT_TYPE, "application/x-ndjson")], Body::from_stream(lines)).into_response())
}

async fn provision(state: &AppState, project: Project, tip: String, setup: cloud_setup::File, creation: Creation, service: Service, sender: &mpsc::UnboundedSender<Value>) -> Result<CloudHost, String> {
    let _turn = state.cloud_hosts.creating.lock().await;
    let checkout = FsPath::new(&project.path);
    let (branch, existing, track) = runs::branch_spec(checkout, creation.branch.as_deref(), &creation.from).await?;
    let parent = project.worktree.as_ref().map_or(&project.id, |worktree| &worktree.parent);
    let projects = state.projects.list();
    let parent_project = projects.iter().find(|project| project.id == *parent).ok_or("The parent Project is not open")?;
    let name = FsPath::new(&parent_project.path).file_name().and_then(|name| name.to_str()).ok_or("The Project has no directory name")?;
    if !existing && git::run(checkout, &["show-ref", "--verify", &format!("refs/heads/{branch}")]).await.is_ok() {
        return Err(format!("a branch named '{branch}' already exists"));
    }
    if git::run_alone(checkout, &["worktree", "list", "--porcelain"]).await?.lines().any(|line| line == format!("branch refs/heads/{branch}"))
        || state.cloud_hosts.records.borrow().iter().any(|host| host.parent == *parent && host.branch == branch) {
        return Err(format!("{branch} is already checked out"));
    }
    let id = format!("stampede-{}-{}", &state.cloud_hosts.home_id[..8], &Uuid::new_v4().simple().to_string()[..6]);
    let step = |step| { let _ = sender.send(json!({ "step": step })); };
    let result = async {
        step("create");
        cloud::create(&id).await?;
        step("install");
        let home = remote(&id, &["sh", "-c", "printf '%s' \"$HOME\""], None).await?;
        let binary = format!("{home}/.local/bin/stampede");
        cloud::push(&id, &std::env::current_exe().map_err(|error| error.to_string())?, FsPath::new(&binary)).await?;
        remote(&id, &["chmod", "+x", &binary], None).await?;
        let shared = config::shared(&state.config_path).map_err(|error| error.to_string())?;
        remote(&id, &["sh", "-c", "mkdir -p \"$HOME/.config/stampede\" && cat > \"$HOME/.config/stampede/config.toml\""], Some(shared.as_bytes())).await?;
        remote(&id, &["sprite-env", "services", "create", "stampede", "--cmd", &binary, "--args", "serve,--cloud,sprite,--port,0"], None).await?;
        step("repository");
        let path = format!("{home}/{name}");
        let unborn = format!("stampede-bootstrap-{}", Uuid::new_v4().simple());
        remote(&id, &["git", "init", "-q", "--initial-branch", &unborn, &path], None).await?;
        git::run(checkout, &["-c", "protocol.ext.allow=always", "push", &cloud::git_remote(&id, &path), &format!("{tip}:refs/heads/{branch}")]).await?;
        remote(&id, &["git", "-C", &path, "checkout", "-q", branch], None).await?;
        for name in git::run(checkout, &["remote"]).await?.lines() {
            let urls = git::run(checkout, &["config", "--get-all", &format!("remote.{name}.url")]).await?;
            let mut urls = urls.lines();
            if let Some(url) = urls.next() {
                remote(&id, &["git", "-C", &path, "remote", "add", "--", name, url], None).await?;
                for url in urls {
                    remote(&id, &["git", "-C", &path, "remote", "set-url", "--add", "--", name, url], None).await?;
                }
            }
            if let Ok(urls) = git::run(checkout, &["config", "--get-all", &format!("remote.{name}.pushurl")]).await {
                for url in urls.lines() {
                    remote(&id, &["git", "-C", &path, "remote", "set-url", "--add", "--push", "--", name, url], None).await?;
                }
            }
        }
        if track == "--track" {
            remote(&id, &["git", "-C", &path, "update-ref", &format!("refs/remotes/{}", creation.from), &tip], None).await?;
            remote(&id, &["git", "-C", &path, "branch", &format!("--set-upstream-to={}", creation.from), branch], None).await?;
        }
        for key in ["user.name", "user.email"] {
            if let Ok(value) = git::run(checkout, &["config", "--get", key]).await {
                remote(&id, &["git", "-C", &path, "config", key, &value], None).await?;
            }
        }
        step("carry");
        let mut skipped = Vec::new();
        for carried in &setup.carry {
            if fs::symlink_metadata(FsPath::new(&parent_project.path).join(carried)).is_err() {
                skipped.push(carried.clone());
                continue;
            }
            let archive = tokio::process::Command::new("tar").arg("-C").arg(&parent_project.path).args(["-cf", "-", "--", carried]).output().await.map_err(|error| error.to_string())?;
            let archive = cloud::checked(archive)?.stdout;
            remote(&id, &["tar", "-xf", "-", "-C", &path], Some(&archive)).await?;
        }
        let home_name = state.config.borrow().name.clone().unwrap_or_else(|| "Stampede".to_owned());
        let label = if projects.iter().any(|other| other.id != *parent && other.worktree.is_none() && FsPath::new(&other.path).file_name() == Some(name.as_ref())) {
            FsPath::new(&parent_project.path).parent().and_then(FsPath::file_name).map_or_else(|| name.to_owned(), |dir| format!("{}/{name}", dir.to_string_lossy()))
        } else { name.to_owned() };
        let identity = json!({ "homeId": state.cloud_hosts.home_id, "homeName": home_name, "project": label, "branch": branch });
        remote(&id, &["sh", "-c", "cat > \"$HOME/.stampede-cloud.json\""], Some(identity.to_string().as_bytes())).await?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        let port = loop {
            match remote_port(&id).await {
                Ok(port) => break port,
                Err(error) if tokio::time::Instant::now() >= deadline => return Err(error),
                Err(_) => sleep(Duration::from_millis(100)).await,
            }
        };
        let forward = cloud::forward(&id, port).await?;
        step("toolkit");
        let logins = cloud_link::provision_logins(state, forward.port).await?;
        let codex = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| crate::home().join(".codex"));
        match fs::read(codex.join("auth.json")) {
            Ok(auth) => {
                let output = cloud::exec(&id, &["sh", "-c", "umask 077; dir=${CODEX_HOME:-$HOME/.codex}; mkdir -p \"$dir\" && cat > \"$dir/auth.json\" && chmod 600 \"$dir/auth.json\""], &[], Some(&auth)).await;
                if !output.is_ok_and(|output| output.status.success()) { return Err("Could not provision Codex login.".to_owned()); }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err("Could not read Codex login.".to_owned()),
        }
        if let Some(token) = &logins.gh {
            let input = format!("{token}\n");
            let output = cloud::exec(&id, &["sh", "-c", "IFS= read -r GH_TOKEN; export GH_TOKEN; exec git -C \"$1\" submodule update --init --recursive", "sh", &path], &[], Some(input.as_bytes())).await;
            if !output.is_ok_and(|output| output.status.success()) { return Err("Could not initialize submodules.".to_owned()); }
        } else {
            remote(&id, &["git", "-C", &path, "submodule", "update", "--init", "--recursive"], None).await?;
        }
        for agent in ["claude", "codex"] {
            let _ = cloud::exec(&id, &[agent, "update"], &[], None).await;
        }
        let path = remote(&id, &["git", "-C", &path, "rev-parse", "--show-toplevel"], None).await?;
        let project = open_project(forward.port, &path).await?;
        let start = json!({ "projectId": project, "command": setup.run, "skipped": skipped });
        let started = state.cloud_hosts.client.post(format!("http://127.0.0.1:{}/api/cloud-setup", forward.port))
            .header(header::CONTENT_TYPE, "application/json").body(start.to_string()).send().await.map_err(|error| error.to_string())?;
        if !started.status().is_success() {
            return Err(started.text().await.unwrap_or_default());
        }
        let started: cloud_setup::Run = serde_json::from_slice(&started.bytes().await.map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
        let host = CloudHost {
            id: id.clone(), service, parent: parent.clone(), branch: branch.to_owned(), existing, project, path,
            setup: Setup::Running, setup_terminal: Some(started.terminal_id), tip: Some(tip), in_sync: true,
        };
        *state.cloud_hosts.reach(&id).forward.lock().await = Some(forward);
        if let Err(error) = state.cloud_hosts.record(host.clone()) {
            state.cloud_hosts.reaches.lock().unwrap().remove(&id);
            return Err(error);
        }
        Ok(host)
    }.await;
    if result.is_err() && let Err(error) = cloud::destroy(&id).await {
        eprintln!("cannot destroy {id}: {error}");
    }
    result
}

async fn open_project(port: u16, path: &str) -> Result<String, String> {
    timeout(Duration::from_secs(10), async {
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws")).await.map_err(|error| error.to_string())?;
        socket.send(Message::Text(json!({ "type": "project.open", "path": path }).to_string().into())).await.map_err(|error| error.to_string())?;
        while let Some(message) = socket.next().await {
            if let Message::Text(text) = message.map_err(|error| error.to_string())? {
                let message: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
                match message["type"].as_str() {
                    Some("project.opened") => return message["projectId"].as_str().map(str::to_owned).ok_or("Missing Project id".to_owned()),
                    Some("project.error") => return Err(message["error"].as_str().unwrap_or("Cannot open Project").to_owned()),
                    _ => {}
                }
            }
        }
        Err("Cloud Host closed its connection".to_owned())
    }).await.map_err(|error| error.to_string())?
}

pub async fn proxy(State(state): State<AppState>, request: Request) -> Result<Response, (StatusCode, String)> {
    let bad_gateway = |error: String| (StatusCode::BAD_GATEWAY, error);
    let path = request.uri().path_and_query().map_or("/", |path| path.as_str());
    let (id, rest) = path.strip_prefix("/cloud/").and_then(|path| path.split_once('/')).ok_or((StatusCode::NOT_FOUND, String::new()))?;
    if !state.cloud_hosts.records.borrow().iter().any(|host| host.id == id) {
        return Err((StatusCode::NOT_FOUND, format!("Unknown Cloud Host {id}")));
    }
    if rest.split('?').next() == Some("api/home-link") { return Err((StatusCode::NOT_FOUND, String::new())); }
    let port = state.cloud_hosts.port(id).await.map_err(bad_gateway)?;
    let reach = state.cloud_hosts.reach(id);
    let target = format!("127.0.0.1:{port}/{rest}");
    let (mut parts, body) = request.into_parts();
    if parts.headers.get(header::UPGRADE).is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"websocket")) {
        let upgrade = WebSocketUpgrade::from_request_parts(&mut parts, &()).await.map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
        let mut request = format!("ws://{target}").into_client_request().map_err(|error| bad_gateway(error.to_string()))?;
        for name in [header::COOKIE, header::AUTHORIZATION, header::ORIGIN, header::SEC_WEBSOCKET_PROTOCOL] {
            if let Some(value) = parts.headers.get(&name) {
                request.headers_mut().insert(name, value.clone());
            }
        }
        let (upstream, response) = timeout(Duration::from_secs(10), connect_async(request)).await.map_err(|error| bad_gateway(error.to_string()))?.map_err(|error| bad_gateway(error.to_string()))?;
        let protocol = response.headers().get(header::SEC_WEBSOCKET_PROTOCOL).and_then(|value| value.to_str().ok());
        let connected = reach.devices.connect();
        reach.reached.notify_one();
        return Ok(upgrade.protocols(protocol.into_iter().map(str::to_owned)).on_upgrade(async move |socket| {
            let _connected = connected;
            browser::bridge(socket, upstream).await;
        }));
    }
    let mut headers = parts.headers;
    browser::clean(&mut headers);
    headers.remove(header::HOST);
    let response = state.cloud_hosts.client.request(parts.method, format!("http://{target}")).headers(headers).body(reqwest::Body::wrap_stream(body.into_data_stream())).send().await.map_err(|error| bad_gateway(error.to_string()))?;
    reach.reached.notify_one();
    let status = response.status();
    let mut headers = response.headers().clone();
    browser::clean(&mut headers);
    let mut result = Response::new(Body::from_stream(response.bytes_stream()));
    *result.status_mut() = status;
    *result.headers_mut() = headers;
    Ok(result)
}
