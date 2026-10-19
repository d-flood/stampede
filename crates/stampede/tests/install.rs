use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

use tempfile::TempDir;

fn fake(bin: &Path, name: &str, status: u8) {
    let path = bin.join(name);
    fs::write(&path, format!("#!/bin/sh\necho \"{name} $*\" >> \"$(dirname \"$0\")/calls\"\nexit {status}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn install_survives_refused_linger_and_tailscale() {
    let home = TempDir::new().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    fake(&bin, "systemctl", 0);
    fake(&bin, "loginctl", 1);
    fake(&bin, "tailscale", 1);
    let output = Command::new(env!("CARGO_BIN_EXE_stampede"))
        .args(["install", "--unit-name", "stampede-install-test", "--port", "4999", "--https-port", "8443"])
        .env("HOME", home.path())
        .env("USER", "stampede-install-test")
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("run: sudo loginctl enable-linger stampede-install-test"), "{stderr}");
    assert!(stderr.contains("run: sudo tailscale set --operator=stampede-install-test && tailscale serve --bg --https=8443 http://127.0.0.1:4999"), "{stderr}");

    let unit = fs::read_to_string(home.path().join(".config/systemd/user/stampede-install-test.service")).unwrap();
    assert!(unit.contains("\nHttpsPort=8443\nAddedLinger=no\n"), "{unit}");
    assert_eq!(fs::read_to_string(bin.join("calls")).unwrap(), [
        "loginctl enable-linger stampede-install-test",
        "systemctl --user daemon-reload",
        "systemctl --user enable --now stampede-install-test",
        "tailscale serve --bg --https=8443 http://127.0.0.1:4999",
        "",
    ].join("\n"));
}
