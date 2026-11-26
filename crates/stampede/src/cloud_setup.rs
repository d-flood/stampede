use std::{fs, io, path::{Component, Path, PathBuf}, sync::Arc, time::{Duration, SystemTime, UNIX_EPOCH}};

use axum::{Json, extract::{Query, State}, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Mutex, watch};

use crate::{AppState, cloud_hosts::Setup, files, git, terminals::Terminals};

pub const PATH: &str = ".stampede/cloud-setup.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    #[serde(default)]
    pub carry: Vec<String>,
    pub run: String,
}

/// Reads the Cloud Setup committed at `commit`, which `from` names, refusing one that is absent or invalid.
pub async fn read(checkout: &Path, commit: &str, from: &str) -> Result<File, String> {
    let Ok(text) = git::run(checkout, &["show", &format!("{commit}:{PATH}")]).await else {
        return Err(format!("{from} has no Cloud Setup. Type /cloud-setup in a Chat to draft one."));
    };
    let file: File = toml::from_str(&text).map_err(|error| format!("{PATH}: {}", error.message()))?;
    for path in &file.carry {
        if path.is_empty() || !Path::new(path).components().all(|component| matches!(component, Component::Normal(_) | Component::CurDir)) {
            return Err(format!("{PATH}: carry path {path:?} must be relative and stay inside the repository"));
        }
    }
    Ok(file)
}

#[derive(Deserialize)]
pub struct Starting {
    from: String,
}

pub async fn check(State(state): State<AppState>, axum::extract::Path(id): axum::extract::Path<String>, Query(query): Query<Starting>) -> Result<Json<Value>, files::Failure> {
    let project = state.runs.repository_root(&id).await.map_err(|error| (StatusCode::BAD_REQUEST, error))?;
    Ok(Json(match read(Path::new(&project.path), &query.from, &query.from).await {
        Ok(_) => json!({ "present": true }),
        Err(error) => json!({ "present": false, "error": error }),
    }))
}

/// The Cloud Setup a Cloud Host runs, kept in its data directory.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub project_id: String,
    pub command: String,
    pub skipped: Vec<String>,
    pub state: Setup,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
    pub terminal_id: String,
    pub started_at: u64,
}

impl Run {
    pub fn running(&self) -> bool {
        matches!(self.state, Setup::Running)
    }
}

pub struct CloudSetup {
    data_dir: PathBuf,
    exe: PathBuf,
    run: watch::Sender<Option<Run>>,
    turn: Mutex<()>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    project_id: String,
    command: String,
    skipped: Vec<String>,
}

impl CloudSetup {
    pub fn load(data_dir: &Path) -> io::Result<Self> {
        let run = match fs::read(data_dir.join("cloud-setup.json")) {
            Ok(bytes) => Some(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        Ok(Self { data_dir: data_dir.to_owned(), exe: std::env::current_exe()?, run: watch::Sender::new(run), turn: Mutex::new(()) })
    }

    pub fn subscribe(&self) -> watch::Receiver<Option<Run>> {
        self.run.subscribe()
    }

    fn save(&self, run: Run) -> io::Result<Run> {
        let path = self.data_dir.join("cloud-setup.json");
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&run)?)?;
        fs::rename(temporary, path)?;
        self.run.send_replace(Some(run.clone()));
        Ok(run)
    }

    /// Writes the script the Terminal runs, so that the line typed there is short and means the same in any shell.
    fn script(&self, run: &Run, root: &str) -> io::Result<PathBuf> {
        let mut script = format!("report() {{ {} cloud-setup-exit \"$1\" --data-dir {}; exit \"$1\"; }}\n",
            quote(&self.exe.to_string_lossy()), quote(&self.data_dir.to_string_lossy()));
        // Ctrl-C or closing the Tab would otherwise end the script before it reports.
        script += "trap 'report 129' HUP\ntrap 'report 130' INT\ntrap 'report 143' TERM\n";
        for path in &run.skipped {
            script += &format!("printf '%s\\n' {}\n", quote(&format!("Skipped {path}: not on the Home Host")));
        }
        script += &format!("cd {} && sh -c {}\nreport $?\n", quote(root), quote(&run.command));
        let path = self.data_dir.join("cloud-setup.sh");
        fs::write(&path, script)?;
        Ok(path)
    }

    async fn submit(&self, state: &AppState, mut run: Run) -> Result<Run, String> {
        let project = state.projects.list().iter().find(|project| project.id == run.project_id).cloned().ok_or("The Project is not open")?;
        if !state.terminals.live(&run.terminal_id) {
            state.terminals.dismiss(&run.terminal_id);
            let scrollback = state.config.borrow().scrollback;
            run.terminal_id = state.terminals.create(Some(&project), scrollback, "").await.map_err(|error| error.to_string())?.id;
            // A line typed before the shell shows its prompt is echoed twice, once by the terminal and once by the shell.
            let mut attachment = state.terminals.attach(&run.terminal_id).await.map_err(|error| error.to_string())?;
            let _ = tokio::time::timeout(Duration::from_secs(5), async {
                let mut printed = false;
                loop {
                    match tokio::time::timeout(Duration::from_millis(300), attachment.output.next()).await {
                        Ok(Some(bytes)) => printed |= !bytes.is_empty(),
                        Ok(None) => break,
                        Err(_) if printed => break,
                        Err(_) => {}
                    }
                }
            }).await;
        }
        let script = self.script(&run, &project.path).map_err(|error| error.to_string())?;
        run.state = Setup::Running;
        run.status = None;
        run.started_at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64);
        let run = self.save(run).map_err(|error| error.to_string())?;
        state.terminals.input(&run.terminal_id, "", format!("sh {}\r", quote(&script.to_string_lossy())).into_bytes());
        Ok(run)
    }

    /// Fails a run whose Terminal didn't survive a cold stop, since it can no longer report.
    pub fn interrupted(&self, terminals: &Terminals) -> io::Result<()> {
        let Some(mut run) = self.run.borrow().clone().filter(|run| run.running() && !terminals.live(&run.terminal_id)) else { return Ok(()) };
        run.state = Setup::Failed;
        run.status = None;
        self.save(run).map(drop)
    }

    pub fn exit(&self, status: i32) -> Result<String, String> {
        let mut run = self.run.borrow().clone().ok_or("No Cloud Setup has run here")?;
        run.state = if status == 0 { Setup::Ready } else { Setup::Failed };
        run.status = Some(status);
        self.save(run).map_err(|error| error.to_string())?;
        Ok(String::new())
    }
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

pub async fn start(State(state): State<AppState>, Json(start): Json<Start>) -> Result<Json<Run>, files::Failure> {
    let setup = Arc::clone(&state.cloud_setup);
    let _turn = setup.turn.lock().await;
    let run = Run { project_id: start.project_id, command: start.command, skipped: start.skipped, state: Setup::Running, status: None, terminal_id: String::new(), started_at: 0 };
    setup.submit(&state, run).await.map(Json).map_err(|error| (StatusCode::BAD_REQUEST, error))
}

pub async fn again(State(state): State<AppState>) -> Result<Json<Run>, files::Failure> {
    let setup = Arc::clone(&state.cloud_setup);
    let _turn = setup.turn.lock().await;
    let run = setup.run.borrow().clone().ok_or((StatusCode::NOT_FOUND, "No Cloud Setup has run here".to_owned()))?;
    setup.submit(&state, run).await.map(Json).map_err(|error| (StatusCode::BAD_REQUEST, error))
}
