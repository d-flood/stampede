use std::{io::ErrorKind, net::{Ipv4Addr, TcpListener}, path::Path, process::{Output, Stdio}, time::Duration};

use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{io::{self, AsyncWriteExt}, net::TcpStream, process::{Child, Command}, time::{Instant, sleep}};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    Sprite,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Availability {
    Missing,
    SignedOut(String),
    Ready { organization: String },
}

#[derive(Debug, Deserialize)]
pub struct Listed {
    pub name: String,
    #[serde(rename = "created_at")]
    pub created: String,
}

#[derive(Deserialize)]
struct Listing {
    sprites: Vec<Listed>,
    org: Organization,
}

#[derive(Deserialize)]
struct Organization {
    name: String,
}

fn cli() -> Command {
    let mut command = Command::new("sprite");
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    command
}

fn message(output: &Output) -> String {
    let bytes = if output.stderr.is_empty() { &output.stdout } else { &output.stderr };
    String::from_utf8_lossy(bytes).trim_end().to_owned()
}

pub(crate) fn checked(output: Output) -> Result<Output, String> {
    if output.status.success() { Ok(output) } else { Err(message(&output)) }
}

async fn listing() -> Result<Listing, Availability> {
    let output = cli().args(["list", "--json"]).output().await.map_err(|error| {
        if error.kind() == ErrorKind::NotFound { Availability::Missing } else { Availability::SignedOut(error.to_string()) }
    })?;
    let output = checked(output).map_err(Availability::SignedOut)?;
    serde_json::from_slice(&output.stdout).map_err(|error| Availability::SignedOut(error.to_string()))
}

pub async fn availability() -> Availability {
    match listing().await {
        Ok(listing) => Availability::Ready { organization: listing.org.name },
        Err(availability) => availability,
    }
}

pub async fn status() -> Json<Value> {
    Json(match availability().await {
        Availability::Missing => json!({ "cli": "missing" }),
        Availability::SignedOut(_) => json!({ "cli": "signed-out" }),
        Availability::Ready { organization } => json!({ "cli": "ready", "organization": organization }),
    })
}

pub async fn list() -> Result<Vec<Listed>, String> {
    listing().await.map(|listing| listing.sprites).map_err(|failure| match failure {
        Availability::Missing => "sprite is not installed".to_owned(),
        Availability::SignedOut(message) => message,
        Availability::Ready { .. } => unreachable!(),
    })
}

async fn run(command: &mut Command) -> Result<(), String> {
    checked(command.output().await.map_err(|error| error.to_string())?).map(|_| ())
}

pub async fn create(name: &str) -> Result<(), String> {
    run(cli().args(["create", "--skip-console", name])).await
}

pub async fn destroy(name: &str) -> Result<(), String> {
    run(cli().args(["destroy", "--force", name])).await
}

pub async fn exec(name: &str, command: &[&str], env: &[(&str, &str)], stdin: Option<&[u8]>) -> Result<Output, String> {
    let mut process = cli();
    process.args(["exec", "-s", name, "--no-port-forward", "--stdin"]);
    if !env.is_empty() {
        let values = env.iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join(",");
        process.args(["--env", &values]);
    }
    let mut child = process.arg("--").args(command).stdin(Stdio::piped()).spawn().map_err(|error| error.to_string())?;
    let mut pipe = child.stdin.take().unwrap();
    let write = async move {
        if let Some(input) = stdin {
            let _ = pipe.write_all(input).await;
        }
    };
    let (_, output) = tokio::join!(write, child.wait_with_output());
    output.map_err(|error| error.to_string())
}

pub async fn push(name: &str, local: &Path, remote: &Path) -> Result<(), String> {
    run(cli().args(["file", "push", "-p", "-s", name]).arg(local).arg(remote)).await
}

pub struct Forward {
    pub port: u16,
    _child: Child,
}

impl Forward {
    pub fn running(&mut self) -> bool {
        matches!(self._child.try_wait(), Ok(None))
    }
}

pub async fn forward(name: &str, remote_port: u16) -> Result<Forward, String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    drop(listener);
    let mut child = cli().args(["proxy", "-s", name, &format!("{port}:{remote_port}")])
        .spawn().map_err(|error| error.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if child.try_wait().map_err(|error| error.to_string())?.is_some() {
            let output = child.wait_with_output().await.map_err(|error| error.to_string())?;
            return Err(message(&output));
        }
        if TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await.is_ok() {
            let mut stdout = child.stdout.take().unwrap();
            let mut stderr = child.stderr.take().unwrap();
            tokio::spawn(async move {
                let (mut out, mut err) = (io::sink(), io::sink());
                let _ = tokio::join!(io::copy(&mut stdout, &mut out), io::copy(&mut stderr, &mut err));
            });
            return Ok(Forward { port, _child: child });
        }
        if Instant::now() >= deadline {
            return Err("sprite proxy did not listen".to_owned());
        }
        sleep(Duration::from_millis(25)).await;
    }
}

pub fn git_remote(name: &str, path: &str) -> String {
    let escape = |arg: &str| arg.replace('%', "%%").replace(' ', "% ");
    format!("ext::sprite exec -s {} --no-port-forward --stdin -- %S {}", escape(name), escape(path))
}
