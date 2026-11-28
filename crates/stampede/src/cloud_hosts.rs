use std::{collections::HashMap, convert::Infallible, fs, io, path::{Path as FsPath, PathBuf}, sync::{self, Arc}, time::Duration};

use axum::{Json, body::Body, extract::{FromRequestParts, Path, Query, Request, State, WebSocketUpgrade}, http::{StatusCode, header}, response::{IntoResponse, Response}};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{io::AsyncBufReadExt, sync::{Mutex, Notify, mpsc, watch}, time::{sleep, timeout}};
use tokio_tungstenite::{connect_async, tungstenite::{Message, client::IntoClientRequest}};
use uuid::Uuid;

use crate::{AppState, Creation, browser, cloud::{self, Forward, Service}, cloud_setup, cloud_link, config, device::Devices, files, git, projects::{Project, Projects}, runs};

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
    saving: sync::Mutex<()>,
    creating: Mutex<()>,
    reaches: sync::Mutex<HashMap<String, Arc<Reach>>>,
    /// How many connections the Home Host holds to each Cloud Host it holds any to.
    held: watch::Sender<HashMap<String, usize>>,
    failures: watch::Sender<HashMap<String, Option<String>>>,
    client: reqwest::Client,
}

/// Counts one connection to a Cloud Host until it drops.
pub struct Held(watch::Sender<HashMap<String, usize>>, String);

impl Drop for Held {
    fn drop(&mut self) {
        self.0.send_if_modified(|held| {
            let count = held.get_mut(&self.1).unwrap();
            *count -= 1;
            *count == 0 && held.remove(&self.1).is_some()
        });
    }
}

/// How a Cloud Host is reached now, and by whom.
#[derive(Default)]
pub struct Reach {
    operation: Mutex<()>,
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
            home_id, file, records: watch::Sender::new(records), saving: sync::Mutex::default(), creating: Mutex::new(()), reaches: sync::Mutex::default(), held: watch::Sender::default(), failures: watch::Sender::default(),
            client: reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).build().map_err(io::Error::other)?,
        })
    }

    pub fn subscribe(&self) -> watch::Receiver<Vec<CloudHost>> {
        self.records.subscribe()
    }

    pub fn hold(&self, id: &str) -> Held {
        self.held.send_if_modified(|held| {
            let count = held.entry(id.to_owned()).or_default();
            *count += 1;
            *count == 1
        });
        Held(self.held.clone(), id.to_owned())
    }

    pub fn subscribe_held(&self) -> watch::Receiver<HashMap<String, usize>> {
        self.held.subscribe()
    }

    /// Whether the Home Host holds any connection to the Cloud Host, which keeps a Sprite awake.
    pub fn awake(&self, id: &str) -> bool {
        self.held.borrow().contains_key(id)
    }

    pub fn subscribe_failures(&self) -> watch::Receiver<HashMap<String, Option<String>>> {
        self.failures.subscribe()
    }

    fn state(&self, id: &str) -> (&'static str, Option<String>) {
        match self.failures.borrow().get(id) {
            Some(None) => ("gone", None),
            Some(Some(reason)) => ("unreachable", Some(reason.clone())),
            None if self.awake(id) => ("awake", None),
            None => ("asleep", None),
        }
    }

    pub fn gone(&self, id: &str) -> bool {
        self.state(id).0 == "gone"
    }

    pub fn connected(&self, id: &str) {
        self.failures.send_if_modified(|failures| failures.remove(id).is_some());
    }

    pub async fn failed(&self, id: &str, reason: String) {
        if self.gone(id) { return }
        let failure = if cloud::list().await.is_ok_and(|list| !list.iter().any(|host| host.name == id)) {
            None
        } else { Some(reason) };
        if self.contains(id) {
            self.failures.send_if_modified(|failures| {
                if failures.get(id) == Some(&failure) { return false }
                failures.insert(id.to_owned(), failure);
                true
            });
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.records.borrow().iter().any(|host| host.id == id)
    }

    /// The Project entries of the records whose parent Project is open, since a tile shows only under its parent.
    pub fn projects(&self, open: &[Project]) -> Vec<Value> {
        self.records.borrow().iter().filter(|host| open.iter().any(|project| project.id == host.parent)).map(|host| {
            let (state, reason) = self.state(&host.id);
            json!({
                "id": host.project, "host": host.id, "path": host.path,
                "worktree": { "parent": host.parent, "branch": host.branch },
                "cloud": { "setup": host.setup, "state": state, "reason": reason, "tip": host.tip, "inSync": host.in_sync, "setupTerminal": host.setup_terminal },
            })
        }).collect()
    }

    fn record(&self, host: CloudHost) -> Result<(), String> {
        let _saving = self.saving.lock().unwrap();
        let mut records = self.records.borrow().clone();
        records.push(host);
        self.save(records)
    }

    pub fn set_setup(&self, id: &str, setup: Setup, terminal: Option<String>) -> Result<(), String> {
        let _saving = self.saving.lock().unwrap();
        let mut records = self.records.borrow().clone();
        let Some(host) = records.iter_mut().find(|host| host.id == id) else { return Ok(()) };
        host.setup = setup;
        host.setup_terminal = terminal.or(host.setup_terminal.take());
        self.save(records)
    }

    fn set_tip(&self, id: &str, tip: Option<String>, in_sync: bool) -> Result<(), String> {
        let _saving = self.saving.lock().unwrap();
        let mut records = self.records.borrow().clone();
        let Some(host) = records.iter_mut().find(|host| host.id == id) else { return Ok(()) };
        host.tip = tip;
        host.in_sync = in_sync;
        self.save(records)
    }

    pub async fn guard_branch(&self, projects: &Projects, checkout: &FsPath, branch: &str) -> Result<(), String> {
        let parents: Vec<_> = self.records.borrow().iter().filter(|host| host.branch == branch).map(|host| host.parent.clone()).collect();
        if !parents.is_empty() {
            let common = git::common_dir(checkout).await;
            for parent in projects.list().iter().filter(|project| parents.contains(&project.id)) {
                if common.is_some() && git::common_dir(FsPath::new(&parent.path)).await == common {
                    return Err(format!("{branch} is on a Cloud Host."));
                }
            }
        }
        Ok(())
    }

    fn parent(&self, projects: &Projects, id: &str) -> Option<(CloudHost, Project)> {
        let host = self.records.borrow().iter().find(|host| host.id == id)?.clone();
        let parent = projects.list().iter().find(|project| project.id == host.parent)?.clone();
        Some((host, parent))
    }

    pub fn is_parent(&self, id: &str, project: &str) -> bool {
        self.records.borrow().iter().any(|host| host.id == id && host.parent == project)
    }

    pub async fn mirror_tip(&self, state: &AppState, id: &str, tip: &str) -> Result<(), String> {
        let reach = self.reach(id);
        let _operation = reach.operation.lock().await;
        self.mirror(state, id, tip, false).await
    }

    async fn mirror(&self, state: &AppState, id: &str, tip: &str, fetch: bool) -> Result<(), String> {
        let Some((host, parent)) = self.parent(&state.projects, id) else { return Ok(()) };
        if !host.in_sync { return Ok(()) }
        let checkout = FsPath::new(&parent.path);
        let branch = format!("refs/heads/{}", host.branch);
        let current = git::run(checkout, &["rev-parse", "--verify", "--quiet", &branch]).await.ok();
        if current.is_some() && current != host.tip {
            return self.set_tip(id, host.tip, false);
        }
        if !fetch && current.as_deref() == Some(tip) { return Ok(()) }
        let remote = cloud::git_remote(id, &host.path);
        let incoming = format!("refs/stampede/cloud/{id}");
        git::run_alone(checkout, &["-c", "protocol.ext.allow=always", "fetch", "--no-tags", "--no-recurse-submodules", "--no-write-fetch-head", &remote, &format!("+{branch}:{incoming}")]).await?;
        let fetched = git::run(checkout, &["rev-parse", "--verify", &incoming]).await?;
        let worktrees = git::run_alone(checkout, &["worktree", "list", "--porcelain"]).await?;
        if worktrees.lines().any(|line| line == format!("branch {branch}")) {
            let _ = git::run_alone(checkout, &["update-ref", "-d", &incoming, &fetched]).await;
            return Err(format!("{} is checked out on the Home Host.", host.branch));
        }
        // Compare-and-swap even if another git process moved the branch during the fetch.
        let result = git::run_alone(checkout, &["update-ref", "--no-deref", &branch, &fetched, current.as_deref().unwrap_or("")]).await;
        let _ = git::run_alone(checkout, &["update-ref", "-d", &incoming, &fetched]).await;
        if let Err(error) = result {
            let now = git::run(checkout, &["rev-parse", "--verify", "--quiet", &branch]).await.ok();
            if now != current { return self.set_tip(id, host.tip, false); }
            return Err(error);
        }
        self.set_tip(id, Some(fetched), true)?;
        state.git.reread(&parent.id);
        Ok(())
    }

    pub async fn push_branches(&self, projects: &Projects, id: &str, sent: &mut Option<String>) -> Result<(), String> {
        let Some((host, parent)) = self.parent(projects, id) else { return Ok(()) };
        let checkout = FsPath::new(&parent.path);
        let refs = git::run(checkout, &["for-each-ref", "--format=%(refname) %(objectname)", "refs/heads"]).await?;
        if sent.as_ref() == Some(&refs) { return Ok(()) }
        let remote = cloud::git_remote(id, &host.path);
        git::run_alone(checkout, &["-c", "protocol.ext.allow=always", "push", "--prune", &remote, "+refs/heads/*:refs/remotes/home/*"]).await?;
        *sent = Some(refs);
        Ok(())
    }

    async fn delete(&self, state: &AppState, id: &str, force: bool) -> Result<bool, runs::Refusal> {
        use runs::Refusal::{Refused, Unmerged};
        let reach = self.reach(id);
        let _operation = reach.operation.lock().await;
        let (host, parent) = self.parent(&state.projects, id).ok_or_else(|| Refused("Open the parent Project before deleting its Cloud Host worktree.".to_owned()))?;
        let checkout = FsPath::new(&parent.path);
        let branch = (!host.existing).then_some(host.branch.as_str());
        let (mut dirty, mut elsewhere) = (false, 0);
        let mut reached = false;
        if !self.gone(id) {
            let checked = async {
                let tip = remote(id, &["git", "-C", &host.path, "rev-parse", "--verify", &format!("refs/heads/{}", host.branch)], None).await?;
                self.mirror(state, id, &tip, true).await?;
                self.push_branches(&state.projects, id, &mut None).await?;
                let status = remote(id, &["git", "-C", &host.path, "status", "--porcelain", "--ignore-submodules=none"], None).await?;
                let inside = remote(id, &["git", "-C", &host.path, "submodule", "foreach", "--quiet", "git status --porcelain"], None).await?;
                // Only the worktree's branch is mirrored; home/* holds the Home Host's branches there.
                let others = remote(id, &["git", "-C", &host.path, "rev-list", "--count", &format!("--exclude={}", host.branch), "--branches", "--not", "--remotes"], None).await?;
                Ok::<_, String>((!status.is_empty() || !inside.is_empty(), others.parse::<usize>().map_err(|error| error.to_string())?))
            }.await;
            match checked {
                Ok(_) if !force && self.records.borrow().iter().any(|host| host.id == id && !host.in_sync) => {
                    return Err(Unmerged("Its Cloud Host's branch is out of sync. Deleting it discards anything that didn't reach the Home Host.".to_owned()));
                }
                Ok(found) => {
                    (dirty, elsewhere) = found;
                    reached = true;
                },
                Err(reason) => {
                    self.failed(id, reason).await;
                    if !force && !self.gone(id) {
                        return Err(Unmerged("Its Cloud Host can't be reached, so its work can't be checked or its Sessions kept. Deleting it discards anything that didn't reach the Home Host.".to_owned()));
                    }
                }
            }
        }
        if !force {
            let mirrored = match branch {
                Some(branch) if git::run(checkout, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).await.is_ok() => {
                    let count = git::run(checkout, &["rev-list", "--count", &format!("refs/heads/{branch}"), "--not", &format!("--exclude={branch}"), "--branches", "--remotes"]).await.map_err(Refused)?;
                    count.parse::<usize>().map_err(|error| Refused(error.to_string()))?
                }
                _ => 0,
            };
            if let Some(warning) = runs::discard_warning(dirty, mirrored + elsewhere, branch) {
                return Err(Unmerged(warning));
            }
        }
        if reached {
            self.pull_sessions(state, id, &parent).await.map_err(|reason| Refused(format!("Cannot keep its Sessions: {reason}")))?;
        } else {
            eprintln!("Cannot keep Sessions from Cloud Host {id}: unreachable or gone");
        }
        if !self.gone(id) {
            cloud::destroy(id).await.map_err(Refused)?;
        }
        {
            let _saving = self.saving.lock().unwrap();
            let mut records = self.records.borrow().clone();
            records.retain(|host| host.id != id);
            self.save(records).map_err(Refused)?;
        }
        self.connected(id);
        reach.forward.lock().await.take();
        state.browsers.release(id).await;
        state.attention.publish(crate::attention::Source::Cloud(id.to_owned()), vec![]);
        if let Some(branch) = branch {
            if let Err(error) = git::run_alone(checkout, &["branch", "-D", branch]).await {
                eprintln!("cannot delete {branch}: {error}");
            }
        }
        state.git.reread(&parent.id);
        Ok(reached)
    }

    async fn pull_sessions(&self, state: &AppState, id: &str, parent: &Project) -> Result<(), String> {
        let _held = self.hold(id);
        let port = self.port(id).await?;
        let base = format!("http://127.0.0.1:{port}");
        let response = self.client.get(format!("{base}/api/sessions-export"))
            .timeout(Duration::from_secs(300)).send().await.map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("Export returned {}: {}", response.status(), response.text().await.unwrap_or_default()));
        }
        let stream = response.bytes_stream().map(|chunk| chunk.map_err(io::Error::other));
        let mut lines = tokio::io::BufReader::new(tokio_util::io::StreamReader::new(stream)).lines();
        while let Some(line) = lines.next_line().await.map_err(|error| error.to_string())? {
            let session = tokio::task::spawn_blocking(move || {
                serde_json::from_str::<Option<crate::chats::sessions::ExportedSession>>(&line)
                    .map(|session| session.map(|session| {
                        let assets = session.assets();
                        (session, assets)
                    }))
            }).await.map_err(|error| error.to_string())?.map_err(|error| error.to_string())?;
            let Some((session, assets)) = session else { return Ok(()); };
            for asset in assets {
                crate::media::pull(&self.client, &base, &state.media_dir, &asset).await.map_err(|error| format!("Media {asset}: {error}"))?;
            }
            state.sessions.import_cloud(session, parent.clone()).await.map_err(|error| error.to_string())?;
        }
        Err("Session export ended before completion".to_owned())
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
        started_port(id).await.map(drop)
    }
}

async fn remote(id: &str, args: &[&str], stdin: Option<&[u8]>) -> Result<String, String> {
    let output = cloud::checked(cloud::exec(id, args, &[], stdin).await?)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_owned())
}

pub(crate) async fn remote_port(id: &str) -> Result<u16, String> {
    remote(id, &["sh", "-c", "cat \"$HOME/.local/share/stampede/port\""], None).await?.parse::<u16>().map_err(|error| error.to_string())
}

/// Checks out submodules in the repository at `path` there. The GitHub token travels on stdin to stay out of process lists.
async fn update_submodules(id: &str, path: &str, token: Option<&str>, args: &[&str]) -> Result<(), String> {
    let mut command = vec!["sh", "-c", "IFS= read -r GH_TOKEN; [ -n \"$GH_TOKEN\" ] && export GH_TOKEN; dir=$1; shift; exec git -C \"$dir\" submodule update --init \"$@\"", "sh", path];
    command.extend(args);
    let input = format!("{}\n", token.unwrap_or_default());
    remote(id, &command, Some(input.as_bytes())).await.map(drop)
        .map_err(|error| match token { Some(token) => error.replace(token, "[redacted]"), None => error })
}

/// Waits for a Stampede that was just started there to record its port.
async fn started_port(id: &str) -> Result<u16, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match remote_port(id).await {
            Ok(port) => return Ok(port),
            Err(error) if tokio::time::Instant::now() >= deadline => return Err(error),
            Err(_) => sleep(Duration::from_millis(100)).await,
        }
    }
}

pub async fn create(state: AppState, project_id: String, creation: Creation, service: Service) -> Result<Response, files::Failure> {
    let refuse = |error: String| (StatusCode::BAD_REQUEST, error);
    let project = state.runs.repository_root(&project_id).await.map_err(refuse)?;
    let tip = git::run(FsPath::new(&project.path), &["rev-parse", "--verify", &format!("{}^{{commit}}", creation.from)]).await.map_err(refuse)?;
    let setup = cloud_setup::read(FsPath::new(&project.path), &tip, &creation.from).await.map_err(refuse)?;
    if git::run(FsPath::new(&project.path), &["remote"]).await.map_err(refuse)?.lines().any(|name| name == "home") {
        return Err(refuse("A Cloud Host keeps the remote name home for the Home Host's branches. Rename this Project's remote home to use one.".to_owned()));
    }
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
        } else if existing
            && let Ok(upstream) = git::run(checkout, &["config", "--get", &format!("branch.{branch}.remote")]).await
            && upstream != "."
            && let Ok(merge) = git::run(checkout, &["config", "--get", &format!("branch.{branch}.merge")]).await
        {
            remote(&id, &["git", "-C", &path, "config", &format!("branch.{branch}.remote"), &upstream], None).await?;
            remote(&id, &["git", "-C", &path, "config", &format!("branch.{branch}.merge"), &merge], None).await?;
            if let Ok(tracking) = git::run(checkout, &["rev-parse", "--symbolic-full-name", &format!("{branch}@{{upstream}}")]).await {
                git::run(checkout, &["-c", "protocol.ext.allow=always", "push", "-q", &cloud::git_remote(&id, &path), &format!("{tracking}:{tracking}")]).await?;
            }
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
        let forward = cloud::forward(&id, started_port(&id).await?).await?;
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
        let submodules = runs::checked_out(checkout).await?;
        if !submodules.is_empty() {
            let token = logins.gh.as_deref();
            let paths: Vec<&str> = submodules.iter().map(String::as_str).collect();
            if update_submodules(&id, &path, token, &[&["--"], &paths[..]].concat()).await.is_err() {
                let mut tree = vec!["ls-tree", "-z", &tip, "--"];
                tree.extend(&paths);
                for entry in git::run(checkout, &tree).await?.split('\0') {
                    let Some((meta, sub)) = entry.split_once('\t') else { continue };
                    let ["160000", _, commit] = meta.split(' ').collect::<Vec<_>>()[..] else { continue };
                    let Ok(clone) = remote(&id, &["sh", "-c", "test -e \"$1/.git\" && git -C \"$1\" rev-parse --absolute-git-dir", "sh", &format!("{path}/{sub}")], None).await else { continue };
                    let _ = git::run(&checkout.join(sub), &["-c", "protocol.ext.allow=always", "push", "-q", &cloud::git_remote(&id, &clone), &format!("{commit}:refs/stampede/submodule")]).await;
                }
                update_submodules(&id, &path, token, &[&["--no-fetch", "--"], &paths[..]].concat()).await?;
            }
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
    let private = |path: &str| matches!(path, "/api/home-link" | "/api/sessions-export");
    if reqwest::Url::parse(&format!("http://localhost/{rest}")).is_ok_and(|url| private(url.path())) {
        return Err((StatusCode::NOT_FOUND, String::new()));
    }
    // Held before the forward is reached, so the Home link cannot stop the forward under this request.
    let held = state.cloud_hosts.hold(id);
    let port = match state.cloud_hosts.port(id).await {
        Ok(port) => port,
        Err(error) => {
            state.cloud_hosts.failed(id, error.clone()).await;
            return Err(bad_gateway(error));
        }
    };
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
            let _held = held;
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
    let mut result = Response::new(Body::from_stream(response.bytes_stream().inspect(move |_| { let _ = &held; })));
    *result.status_mut() = status;
    *result.headers_mut() = headers;
    Ok(result)
}

pub async fn retry(State(state): State<AppState>, Path(id): Path<String>) -> Result<StatusCode, files::Failure> {
    if !state.cloud_hosts.contains(&id) {
        return Err((StatusCode::NOT_FOUND, "Unknown Cloud Host".to_owned()));
    }
    state.cloud_hosts.reach(&id).reached.notify_one();
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Value>>, files::Failure> {
    let listed = cloud::list().await.map_err(|error| (StatusCode::BAD_REQUEST, error))?;
    let hosts = &state.cloud_hosts;
    let home_name = state.config.borrow().name.clone().unwrap_or_else(|| "Stampede".to_owned());
    let prefix = format!("stampede-{}-", &hosts.home_id[..8]);
    let records = hosts.records.borrow().clone();
    let projects = state.projects.list();
    let mut items = Vec::new();
    for item in listed.into_iter().filter(|item| item.name.starts_with("stampede-")) {
        let own = item.name.starts_with(&prefix);
        let mut entry = json!({"id": item.name, "createdAt": item.created, "home": {"name": home_name, "own": own}});
        if let Some(host) = records.iter().find(|host| host.id == item.name) {
            let path = projects.iter().find(|project| project.id == host.parent).map_or(host.path.as_str(), |project| project.path.as_str());
            let project = FsPath::new(path).file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
            entry["worktree"] = json!({"project": project, "branch": host.branch});
            let status = hosts.state(&item.name).0;
            if status != "gone" { entry["state"] = json!(status); }
        } else if !own {
            match remote(&item.name, &["sh", "-c", "cat \"$HOME/.stampede-cloud.json\""], None).await
                .and_then(|identity| serde_json::from_str::<Value>(&identity).map_err(|error| error.to_string())) {
                Ok(identity) => {
                    entry["home"]["name"] = json!(identity["homeName"].as_str().unwrap_or("Unknown Home Host"));
                    if let (Some(project), Some(branch)) = (identity["project"].as_str(), identity["branch"].as_str()) {
                        entry["worktree"] = json!({"project": project, "branch": branch});
                    }
                }
                Err(_) => {
                    entry["home"]["name"] = json!("Unknown Home Host");
                    entry["state"] = json!("unreachable");
                }
            }
        }
        items.push(entry);
    }
    Ok(Json(items))
}

pub async fn destroy(State(state): State<AppState>, Path(id): Path<String>, Query(query): Query<crate::Deletion>) -> Result<Response, files::Failure> {
    let hosts = &state.cloud_hosts;
    let _creating = hosts.creating.lock().await;
    if hosts.contains(&id) {
        match hosts.delete(&state, &id, query.force).await {
            Ok(true) => Ok(StatusCode::NO_CONTENT.into_response()),
            Ok(false) => Ok((StatusCode::NO_CONTENT, [("warning", "299 Stampede \"Its Cloud Host could not be reached; Sessions could not be kept.\"")]).into_response()),
            Err(runs::Refusal::Unmerged(work)) => Err((StatusCode::CONFLICT, work)),
            Err(runs::Refusal::Refused(reason)) => Err((StatusCode::BAD_REQUEST, reason)),
        }
    } else if id.starts_with("stampede-") && cloud::list().await.map_err(|error| (StatusCode::BAD_REQUEST, error))?.iter().any(|item| item.name == id) {
        cloud::destroy(&id).await.map_err(|error| (StatusCode::BAD_REQUEST, error))?;
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        Err((StatusCode::NOT_FOUND, "Unknown Cloud Host".to_owned()))
    }
}
