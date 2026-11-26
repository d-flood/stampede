use std::{env, path::PathBuf, time::Duration};

use reqwest::{Client, Method};
use serde_json::{Value, json};
use tokio::{sync::watch, time::{Instant, sleep, sleep_until}};

use crate::cloud_setup::Run;

/// How long a Terminal's output keeps a Cloud Host awake.
pub const QUIET: Duration = Duration::from_secs(10 * 60);
const RENEW_EVERY: Duration = Duration::from_secs(60);
const TASK: &str = "stampede";

/// Whether a Cloud Host must hold itself awake. A Device showing it needs nothing from this: its connection is enough.
pub fn keep_awake(chats: bool, setup: Option<&Run>, printed: Option<Duration>, quiet: Duration) -> bool {
    chats || setup.is_some_and(Run::running) || printed.is_some_and(|since| since < quiet)
}

/// Holds the Sprite's keep-awake task while `keep_awake` holds, and reports its value through `busy`.
pub fn spawn(busy: watch::Sender<bool>, mut chats: watch::Receiver<bool>, mut setup: watch::Receiver<Option<Run>>, mut printed: watch::Receiver<Option<Instant>>) {
    let quiet = env::var("STAMPEDE_TERMINAL_QUIET_MS").ok().and_then(|ms| ms.parse().ok()).map_or(QUIET, Duration::from_millis);
    let mut task = Task::new(env::var_os("STAMPEDE_SPRITE_API").map_or_else(|| "/.sprite/api.sock".into(), PathBuf::from));
    tokio::spawn(async move {
        loop {
            let now = Instant::now();
            let last = *printed.borrow_and_update();
            let awake = keep_awake(*chats.borrow_and_update(), setup.borrow_and_update().as_ref(), last.map(|at| now - at), quiet);
            busy.send_if_modified(|current| std::mem::replace(current, awake) != awake);
            if awake { task.hold().await } else { task.release().await }
            let lapse = last.map(|at| at + quiet).filter(|lapse| *lapse > now);
            tokio::select! {
                Ok(()) = chats.changed() => {}
                Ok(()) = setup.changed() => {}
                Ok(()) = printed.changed() => {}
                () = sleep_until(lapse.unwrap_or(now)), if lapse.is_some() => {}
                () = sleep(RENEW_EVERY), if awake => {}
                else => return,
            }
        }
    });
}

/// The Sprite's keep-awake task, reached over its local API.
struct Task {
    socket: PathBuf,
    client: Client,
    renewed: Option<Instant>,
    missing: bool,
}

impl Task {
    fn new(socket: PathBuf) -> Self {
        let client = Client::builder().unix_socket(socket.clone()).timeout(Duration::from_secs(10)).build().expect("an HTTP client over a unix socket");
        Self { socket, client, renewed: None, missing: false }
    }

    async fn hold(&mut self) {
        if self.renewed.is_some_and(|at| at.elapsed() < RENEW_EVERY) { return; }
        let create = json!({ "name": TASK, "expire": "5m" });
        let extend = json!({ "expire": "5m" });
        let path = format!("/v1/tasks/{TASK}");
        // A task left by an earlier run of Stampede refuses creation, and an expired one refuses extending.
        let held = if self.renewed.is_some() {
            self.call(Method::PUT, &path, Some(&extend)).await || self.call(Method::POST, "/v1/tasks", Some(&create)).await
        } else {
            self.call(Method::POST, "/v1/tasks", Some(&create)).await || self.call(Method::PUT, &path, Some(&extend)).await
        };
        self.renewed = held.then(Instant::now);
    }

    async fn release(&mut self) {
        if self.renewed.take().is_some() {
            self.call(Method::DELETE, &format!("/v1/tasks/{TASK}"), None).await;
        }
    }

    async fn call(&mut self, method: Method, path: &str, body: Option<&Value>) -> bool {
        if !self.socket.exists() {
            if !std::mem::replace(&mut self.missing, true) {
                eprintln!("no Sprite API at {}; the Cloud Host cannot hold itself awake", self.socket.display());
            }
            return false;
        }
        let mut request = self.client.request(method.clone(), format!("http://sprite{path}"));
        if let Some(body) = body {
            request = request.header("content-type", "application/json").body(body.to_string());
        }
        match request.send().await {
            Ok(response) if response.status().is_success() => true,
            Ok(response) => {
                eprintln!("Sprite API {method} {path}: {} {}", response.status(), response.text().await.unwrap_or_default());
                false
            }
            Err(error) => {
                eprintln!("Sprite API {method} {path}: {error}");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{chats::{Status, working}, cloud_hosts::Setup};

    #[test]
    fn keep_awake_holds_for_work_and_never_for_an_idle_host() {
        let run = |state| Run { project_id: String::new(), command: String::new(), skipped: Vec::new(), state, status: None, terminal_id: String::new(), started_at: 0 };
        let minutes = |minutes: u64| Some(Duration::from_secs(minutes * 60));
        assert!(keep_awake(working(Status::Working, false), None, None, QUIET));
        assert!(keep_awake(working(Status::Done, true), None, None, QUIET));
        assert!(!keep_awake(working(Status::Waiting, false), None, None, QUIET));
        assert!(keep_awake(false, Some(&run(Setup::Running)), None, QUIET));
        assert!(!keep_awake(false, Some(&run(Setup::Ready)), None, QUIET));
        assert!(keep_awake(false, None, minutes(9), QUIET));
        assert!(!keep_awake(false, None, minutes(11), QUIET));
    }
}
