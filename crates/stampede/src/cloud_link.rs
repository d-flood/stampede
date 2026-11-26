use std::{collections::HashSet, time::Duration};

use axum::{extract::{State, WebSocketUpgrade, ws}, http::StatusCode, response::{IntoResponse, Response}};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpStream, sync::{Mutex, broadcast::{Receiver, error::RecvError}, watch}, time::{Instant, interval, timeout}};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::{Bytes, Message}};

use crate::{AppState, attention::{Attention, Feed, Source}, cloud_hosts::{Reach, Setup}, cloud_logins::Logins, cloud_setup::Run, config, device::{PING_EVERY, SILENT_FOR}};

const CONNECT_WITHIN: Duration = Duration::from_secs(60);
const RETRY_FIRST: Duration = Duration::from_secs(1);
const RETRY_LAST: Duration = Duration::from_secs(60);

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", rename_all_fields = "camelCase")]
enum CloudLink {
    Hello { build_id: String, busy: bool },
    Busy { busy: bool },
    Attention { items: Vec<Attention> },
    Push { item: Attention, skip: HashSet<String> },
    Setup {
        state: Setup,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        terminal_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        started_at: Option<u64>,
    },
    Config { toml: String },
    Logins { claude: Option<String>, gh: Option<String> },
    LoginsApplied,
}

impl From<&Run> for CloudLink {
    fn from(run: &Run) -> Self {
        CloudLink::Setup { state: run.state.clone(), status: run.status, terminal_id: Some(run.terminal_id.clone()), started_at: Some(run.started_at) }
    }
}

impl CloudLink {
    fn logins(logins: &Logins) -> Self { Self::Logins { claude: logins.claude.clone(), gh: logins.gh.clone() } }

    fn text(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

/// A Cloud Host's end of the link. Calls made while no link is open wait in `calls` for the next one.
pub struct HomeLink {
    calls: Mutex<Receiver<(Attention, HashSet<String>)>>,
    newest: watch::Sender<u64>,
}

impl HomeLink {
    pub fn new(attention: &Feed) -> Self {
        Self { calls: Mutex::new(attention.calls()), newest: watch::Sender::default() }
    }
}

pub async fn upgrade(State(state): State<AppState>, upgrade: WebSocketUpgrade) -> Response {
    let Some(link) = state.home_link.clone() else { return StatusCode::NOT_FOUND.into_response() };
    upgrade.on_upgrade(async move |socket| {
        let mut this = 0;
        link.newest.send_modify(|newest| {
            *newest += 1;
            this = *newest;
        });
        let mut newest = link.newest.subscribe();
        tokio::select! {
            _ = newest.wait_for(|newest| *newest != this) => {}
            _ = report(socket, &link, &state) => {}
        }
    })
}

async fn report(mut socket: ws::WebSocket, link: &HomeLink, state: &AppState) -> Result<(), axum::Error> {
    let mut calls = link.calls.lock().await;
    let (mut chats, mut setup, mut attention) = (state.chats.busy(), state.cloud_setup.subscribe(), state.attention.subscribe());
    let busy = |chats: bool, setup: &Option<Run>| chats || setup.as_ref().is_some_and(Run::running);
    let mut was = busy(*chats.borrow_and_update(), &setup.borrow_and_update());
    let hello = CloudLink::Hello { build_id: env!("STAMPEDE_BUILD_ID").to_owned(), busy: was };
    socket.send(ws::Message::Text(hello.text().into())).await?;
    attention.mark_changed();
    if setup.borrow().is_some() {
        setup.mark_changed();
    }
    loop {
        let message = tokio::select! {
            // The Home Host may close the link on hearing the Cloud Host is no longer busy, so that goes last.
            biased;
            call = calls.recv() => match call {
                Ok((item, skip)) => Some(CloudLink::Push { item, skip }),
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return Ok(()),
            },
            Ok(()) = attention.changed() => Some(CloudLink::Attention { items: attention.borrow_and_update().clone() }),
            Ok(()) = setup.changed() => setup.borrow_and_update().as_ref().map(CloudLink::from),
            Ok(()) = chats.changed() => {
                chats.borrow_and_update();
                None
            }
            message = socket.recv() => {
                match message.transpose()? {
                    Some(ws::Message::Text(text)) => {
                        match serde_json::from_str(&text) {
                            Ok(CloudLink::Config { toml }) => {
                                if let Err(error) = tokio::fs::write(&state.config_path, toml).await {
                                    eprintln!("cannot save {}: {error}", state.config_path.display());
                                }
                            }
                            Ok(CloudLink::Logins { claude, gh }) => {
                                let logins = Logins { claude, gh };
                                let previous = Logins::read(&state.logins.data).unwrap_or_default();
                                if previous.gh != logins.gh && !logins.configure_git().await {
                                    eprintln!("cannot configure GitHub credential helper");
                                    return Ok(());
                                }
                                if state.logins.save(&logins).is_err() {
                                    eprintln!("cannot save Cloud Host logins");
                                    return Ok(());
                                }
                                socket.send(ws::Message::Text(CloudLink::LoginsApplied.text().into())).await?;
                            }
                            _ => {}
                        }
                    }
                    Some(ws::Message::Close(_)) | None => return Ok(()),
                    Some(_) => {}
                }
                continue;
            }
        };
        if let Some(message) = message {
            socket.send(ws::Message::Text(message.text().into())).await?;
        }
        let now = busy(*chats.borrow(), &setup.borrow());
        if now != was {
            was = now;
            socket.send(ws::Message::Text(CloudLink::Busy { busy: now }.text().into())).await?;
        }
    }
}

/// Links the Home Host to each of its Cloud Hosts, while they have something to say or a Device is showing them.
pub fn spawn(state: AppState) {
    let mut records = state.cloud_hosts.subscribe();
    tokio::spawn(async move {
        let mut linked = HashSet::new();
        loop {
            let ids: Vec<_> = records.borrow_and_update().iter().map(|host| host.id.clone()).collect();
            for id in ids {
                if linked.insert(id.clone()) {
                    tokio::spawn(hold(state.clone(), id));
                }
            }
            if records.changed().await.is_err() {
                break;
            }
        }
    });
}

enum Follow { Idle, Restarted }

async fn hold(state: AppState, id: String) {
    let reach = state.cloud_hosts.reach(&id);
    let mut wait = RETRY_FIRST;
    let mut replaced = false;
    let mut logins = state.logins.subscribe();
    loop {
        match follow(&state, &id, &reach, &mut wait, &mut replaced, &mut logins).await {
            Ok(Follow::Restarted) => continue,
            Ok(Follow::Idle) => {
                tokio::select! {
                    () = reach.reached.notified() => {},
                    _ = logins.changed() => {},
                }
                wait = RETRY_FIRST;
            }
            Err(error) => {
                eprintln!("no link to {id}: {error}");
                let _ = timeout(wait, reach.reached.notified()).await;
                wait = (wait * 2).min(RETRY_LAST);
            }
        }
    }
}

/// Follows a Cloud Host until it is neither busy nor shown, or fails if the link drops before that.
async fn follow(state: &AppState, id: &str, reach: &Reach, wait: &mut Duration, replaced: &mut bool, logins: &mut watch::Receiver<u64>) -> Result<Follow, String> {
    let mut socket = connect(state, id, reach).await?;
    let mut devices = reach.devices.subscribe();
    let mut config = state.config.clone();
    config.mark_unchanged();
    let (mut busy, mut listed, mut closing, mut greeted) = (true, false, false, false);
    let mut awaiting_logins = 0usize;
    let mut sent_logins = None;
    let mut login_check = interval(Duration::from_secs(30));
    login_check.tick().await;
    let (mut heartbeat, mut heard) = (interval(PING_EVERY), Instant::now());
    loop {
        if !closing && !busy && listed && sent_logins.is_some() && awaiting_logins == 0 && !logins.has_changed().unwrap_or(false) && !config.has_changed().unwrap_or(false) && *devices.borrow() == 0 {
            socket.close(None).await.map_err(|error| error.to_string())?;
            closing = true;
        }
        tokio::select! {
            message = socket.next() => {
                heard = Instant::now();
                match message {
                    Some(Ok(Message::Text(text))) => match serde_json::from_str(&text) {
                        Ok(CloudLink::Hello { build_id, busy: now }) => {
                            if build_id != env!("STAMPEDE_BUILD_ID") {
                                if *replaced { return Err("Cloud Host still reports a different build after replacement".to_owned()); }
                                drop(socket);
                                state.cloud_hosts.replace_build(id).await?;
                                *replaced = true;
                                return Ok(Follow::Restarted);
                            }
                            *replaced = false;
                            *wait = RETRY_FIRST;
                            busy = now;
                            greeted = true;
                            config.mark_changed();
                            logins.mark_changed();
                        }
                        Ok(CloudLink::Busy { busy: now }) => busy = now,
                        Ok(CloudLink::Attention { mut items }) => {
                            for item in &mut items { item.host = Some(id.to_owned()); }
                            state.attention.publish(Source::Cloud(id.to_owned()), items);
                            listed = true;
                        }
                        Ok(CloudLink::Push { mut item, skip }) => {
                            item.host = Some(id.to_owned());
                            state.attention.call(item, skip);
                        }
                        Ok(CloudLink::Setup { state: setup, terminal_id, .. }) => {
                            if let Err(error) = state.cloud_hosts.set_setup(id, setup, terminal_id) {
                                eprintln!("cannot record the Cloud Setup of {id}: {error}");
                            }
                        }
                        Ok(CloudLink::LoginsApplied) => awaiting_logins = awaiting_logins.saturating_sub(1),
                        Ok(CloudLink::Logins { .. } | CloudLink::Config { .. }) | Err(_) => {}
                    },
                    Some(Ok(Message::Close(_)) | Err(_)) | None => {
                        return if closing && !busy { Ok(Follow::Idle) } else { Err("it closed".to_owned()) };
                    }
                    Some(Ok(_)) => {}
                }
            }
            // A reach while closing is left to ask for the next link.
            () = reach.reached.notified(), if !closing => {}
            Ok(()) = devices.changed(), if !closing => {}
            Ok(()) = config.changed(), if !closing => {
                config.mark_unchanged();
                match config::shared(&state.config_path) {
                    Ok(toml) => socket.send(Message::Text(CloudLink::Config { toml }.text().into())).await.map_err(|error| error.to_string())?,
                    Err(error) => eprintln!("config {}: {error}", state.config_path.display()),
                }
            }
            Ok(()) = logins.changed(), if greeted && !closing => {
                if send_logins(state, &mut socket, &mut sent_logins).await? { awaiting_logins += 1; }
            }
            _ = login_check.tick(), if !closing && sent_logins.is_some() => {
                if send_logins(state, &mut socket, &mut sent_logins).await? { awaiting_logins += 1; }
            }
            _ = heartbeat.tick() => {
                if heard.elapsed() > SILENT_FOR {
                    return Err("it went silent".to_owned());
                }
                if !closing {
                    socket.send(Message::Ping(Bytes::new())).await.map_err(|error| error.to_string())?;
                }
            }
        }
    }
}

async fn connect(state: &AppState, id: &str, reach: &Reach) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, String> {
    timeout(CONNECT_WITHIN, async {
        let port = state.cloud_hosts.port(id).await?;
        match connect_async(format!("ws://127.0.0.1:{port}/api/home-link")).await {
            Ok((socket, _)) => Ok(socket),
            Err(error) => {
                // Its Stampede takes another port when it restarts.
                if *reach.devices.subscribe().borrow() == 0 {
                    reach.forward.lock().await.take();
                }
                Err(error.to_string())
            }
        }
    }).await.map_err(|error| error.to_string())?
}

async fn send_logins(state: &AppState, socket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>, sent: &mut Option<Logins>) -> Result<bool, String> {
    let logins = Logins::home(&state.logins.data).await;
    if sent.as_ref() == Some(&logins) { return Ok(false); }
    socket.send(Message::Text(CloudLink::logins(&logins).text().into())).await.map_err(|_| "cannot send Cloud Host logins".to_owned())?;
    *sent = Some(logins);
    Ok(true)
}

pub async fn provision_logins(state: &AppState, port: u16) -> Result<Logins, String> {
    timeout(CONNECT_WITHIN, async {
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}/api/home-link")).await.map_err(|error| error.to_string())?;
        let mut sent = None;
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            match serde_json::from_str(&text) {
                Ok(CloudLink::Hello { .. }) => { send_logins(state, &mut socket, &mut sent).await?; }
                Ok(CloudLink::LoginsApplied) => return sent.ok_or("Cloud Host did not receive logins".to_owned()),
                _ => {}
            }
        }
        Err("Cloud Host did not apply logins".to_owned())
    }).await.map_err(|_| "Cloud Host login provisioning timed out".to_owned())?
}
