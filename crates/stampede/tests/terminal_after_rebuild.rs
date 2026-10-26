use crate::common;

use std::{fs, net::TcpListener, process::Stdio, time::Duration};

use common::View;
use tokio::{net::TcpStream, process::Command, time::{Instant, sleep, timeout}};

#[tokio::test]
async fn replacing_the_running_executable_does_not_prevent_new_terminals() {
    let root = tempfile::tempdir().unwrap();
    let executable = root.path().join("stampede");
    let replacement = root.path().join("replacement");
    fs::copy(env!("CARGO_BIN_EXE_stampede"), &executable).unwrap();
    fs::copy(&executable, &replacement).unwrap();
    let home = root.path().join("home");
    fs::create_dir(&home).unwrap();
    let config = root.path().join("config.toml");
    fs::write(&config, "").unwrap();
    let log = root.path().join("server.log");
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let mut server = Command::new(&executable)
        .args(["serve", "--port", &port.to_string(), "--data-dir"])
        .arg(root.path().join("data"))
        .arg("--config")
        .arg(config)
        .env("HOME", &home)
        .env("SHELL", "/bin/bash")
        .stdout(Stdio::null())
        .stderr(fs::File::create(&log).unwrap())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while TcpStream::connect(("127.0.0.1", port)).await.is_err() {
        assert!(Instant::now() < deadline, "the server never listened");
        sleep(Duration::from_millis(50)).await;
    }

    let mut before = View::open(port, "before", None).await;
    before.settle(|text| text.contains('$')).await;
    before.send("exit\r").await;
    fs::rename(replacement, &executable).unwrap();

    let mut after = timeout(Duration::from_secs(5), View::open(port, "after", None))
        .await
        .unwrap_or_else(|_| panic!("terminal creation stalled:\n{}", fs::read_to_string(&log).unwrap()));
    after.settle(|text| text.contains('$')).await;
    after.send("printf 'created-after-rebuild\\n'\r").await;
    after.settle(|text| text.contains("\ncreated-after-rebuild\n")).await;
    after.send("exit\r").await;

    let terminals = root.path().join("data/terminals");
    let deadline = Instant::now() + Duration::from_secs(5);
    while fs::read_dir(&terminals).unwrap().next().is_some() {
        assert!(Instant::now() < deadline, "terminal holders did not exit");
        sleep(Duration::from_millis(50)).await;
    }
    server.kill().await.unwrap();
}
