use std::{collections::HashSet, time::Duration};

use axum::{extract::{State, WebSocketUpgrade, ws}, http::StatusCode, response::{IntoResponse, Response}};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpStream, sync::{Mutex, broadcast::{Receiver, error::RecvError}, watch}, time::{Instant, interval, timeout}};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::{Bytes, Message}};

use crate::{AppState, attention::{Attention, Feed, Source}, cloud_hosts::Reach, config, device::{PING_EVERY, SILENT_FOR}};

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
    Config { toml: String },
}

impl CloudLink {
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
    let (mut busy, mut attention) = (state.chats.busy(), state.attention.subscribe());
    let hello = CloudLink::Hello { build_id: env!("STAMPEDE_BUILD_ID").to_owned(), busy: *busy.borrow_and_update() };
    socket.send(ws::Message::Text(hello.text().into())).await?;
    attention.mark_changed();
    loop {
        let message = tokio::select! {
            // The Home Host may close the link on hearing the Cloud Host is no longer busy, so that goes last.
            biased;
            call = calls.recv() => match call {
                Ok((item, skip)) => CloudLink::Push { item, skip },
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return Ok(()),
            },
            Ok(()) = attention.changed() => CloudLink::Attention { items: attention.borrow_and_update().clone() },
            Ok(()) = busy.changed() => CloudLink::Busy { busy: *busy.borrow_and_update() },
            message = socket.recv() => {
                match message.transpose()? {
                    Some(ws::Message::Text(text)) => {
                        if let Ok(CloudLink::Config { toml }) = serde_json::from_str(&text)
                            && let Err(error) = tokio::fs::write(&state.config_path, toml).await
                        {
                            eprintln!("cannot save {}: {error}", state.config_path.display());
                        }
                    }
                    Some(ws::Message::Close(_)) | None => return Ok(()),
                    Some(_) => {}
                }
                continue;
            }
        };
        socket.send(ws::Message::Text(message.text().into())).await?;
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

async fn hold(state: AppState, id: String) {
    let reach = state.cloud_hosts.reach(&id);
    let mut wait = RETRY_FIRST;
    loop {
        match follow(&state, &id, &reach, &mut wait).await {
            Ok(()) => {
                reach.reached.notified().await;
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
async fn follow(state: &AppState, id: &str, reach: &Reach, wait: &mut Duration) -> Result<(), String> {
    let mut socket = connect(state, id, reach).await?;
    let mut devices = reach.devices.subscribe();
    let mut config = state.config.clone();
    config.mark_unchanged();
    let (mut busy, mut listed, mut closing) = (true, false, false);
    let (mut heartbeat, mut heard) = (interval(PING_EVERY), Instant::now());
    loop {
        if !closing && !busy && listed && !config.has_changed().unwrap_or(false) && *devices.borrow() == 0 {
            socket.close(None).await.map_err(|error| error.to_string())?;
            closing = true;
        }
        tokio::select! {
            message = socket.next() => {
                heard = Instant::now();
                match message {
                    Some(Ok(Message::Text(text))) => match serde_json::from_str(&text) {
                        Ok(CloudLink::Hello { busy: now, .. }) => {
                            *wait = RETRY_FIRST;
                            busy = now;
                            config.mark_changed();
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
                        Ok(CloudLink::Config { .. }) | Err(_) => {}
                    },
                    Some(Ok(Message::Close(_)) | Err(_)) | None => {
                        return if closing && !busy { Ok(()) } else { Err("it closed".to_owned()) };
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
