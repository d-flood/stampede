use std::{
    collections::HashMap,
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;

#[derive(clap::Args)]
pub struct InstallArgs {
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long, default_value_t = 4580)]
    pub port: u16,
    #[arg(long, default_value_t = 8443)]
    pub https_port: u16,
    #[arg(long)]
    pub local_only: bool,
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    #[arg(long, hide = true, default_value = "stampede")]
    pub unit_name: String,
    #[arg(long, hide = true, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,
}

#[derive(clap::Args)]
pub struct UninstallArgs {
    #[arg(long, hide = true, default_value = "stampede")]
    pub unit_name: String,
}

struct Installed {
    port: u16,
    https_port: u16,
    added_linger: bool,
}

pub fn install(args: InstallArgs) -> io::Result<()> {
    let user = user()?;
    let path = unit_path(&args.unit_name);
    let lingering = Path::new("/var/lib/systemd/linger").join(&user).exists();
    let kept_linger = lingering && installed(&path).is_ok_and(|installed| installed.added_linger);
    let https = format!("--https={}", args.https_port);
    let target = format!("http://127.0.0.1:{}", args.port);
    let linger = ["loginctl", "enable-linger", &user];
    let serve = ["tailscale", "serve", "--bg", &https, &target];
    let commands = [
        &["systemctl", "--user", "daemon-reload"][..],
        &["systemctl", "--user", "enable", "--now", &args.unit_name],
    ];
    if args.dry_run {
        let unit = unit(&args, !lingering || kept_linger)?;
        let dir = path.parent().unwrap().display();
        print!("mkdir -p {dir}\ncat > {} <<'EOF'\n{unit}EOF\n", path.display());
        println!();
        for command in commands.into_iter().chain([&linger[..]]).chain((!args.local_only).then_some(&serve[..])) {
            println!("{}", command.join(" "));
        }
        return Ok(());
    }
    let added_linger = kept_linger || (!lingering && attempt(&linger, &format!("sudo loginctl enable-linger {user}")));
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(&path, unit(&args, added_linger)?)?;
    commands.iter().try_for_each(|command| run(command))?;
    if !args.local_only {
        attempt(&serve, &format!("sudo tailscale set --operator={user} && {}", serve.join(" ")));
    }
    Ok(())
}

pub fn uninstall(args: UninstallArgs) -> io::Result<()> {
    let path = unit_path(&args.unit_name);
    let installed = installed(&path)?;
    if installed.https_port != 0 && serving(&installed)? {
        run(&["tailscale", "serve", &format!("--https={}", installed.https_port), "off"])?;
    }
    run(&["systemctl", "--user", "disable", "--now", &args.unit_name])?;
    fs::remove_file(&path)?;
    run(&["systemctl", "--user", "daemon-reload"])?;
    if installed.added_linger {
        run(&["loginctl", "disable-linger", &user()?])?;
    }
    Ok(())
}

fn unit(args: &InstallArgs, added_linger: bool) -> io::Result<String> {
    let mut exec = format!("{} serve --port {}", argument(&env::current_exe()?.to_string_lossy()), args.port);
    if let Some(dir) = &args.data_dir {
        exec += &format!(" --data-dir {}", argument(&std::path::absolute(dir)?.to_string_lossy()));
    }
    if let Some(config) = &args.config {
        exec += &format!(" --config {}", argument(&std::path::absolute(config)?.to_string_lossy()));
    }
    let path = quoted(&format!("PATH={}", env::var("PATH").unwrap_or_default()));
    Ok(format!(
        "[Unit]
Description=Stampede

[Service]
ExecStart={exec}
Environment={path}
Restart=on-failure
KillMode=process

[Install]
WantedBy=default.target

[X-Stampede]
Port={}
HttpsPort={}
AddedLinger={}
",
        args.port,
        if args.local_only { 0 } else { args.https_port },
        if added_linger { "yes" } else { "no" },
    ))
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%").replace('\n', "\\n").replace('\r', "\\r"))
}

fn argument(value: &str) -> String {
    quoted(&value.replace('$', "$$"))
}

fn installed(path: &Path) -> io::Result<Installed> {
    let text = fs::read_to_string(path)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))?;
    let fields: HashMap<_, _> = text
        .lines()
        .skip_while(|line| *line != "[X-Stampede]")
        .filter_map(|line| line.split_once('='))
        .collect();
    let field = |key| {
        fields.get(key).copied().ok_or_else(|| {
            let path = path.display();
            io::Error::other(format!("{path} has no {key}; stampede install did not write it"))
        })
    };
    let port = |key| field(key)?.parse().map_err(io::Error::other);
    Ok(Installed {
        port: port("Port")?,
        https_port: port("HttpsPort")?,
        added_linger: field("AddedLinger")? == "yes",
    })
}

fn serving(installed: &Installed) -> io::Result<bool> {
    let output = Command::new("tailscale").args(["serve", "status", "--json"]).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!("`tailscale serve status` {}", output.status)));
    }
    let status: Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
    let host = format!(":{}", installed.https_port);
    let target = format!("http://127.0.0.1:{}", installed.port);
    Ok(status["Web"].as_object().is_some_and(|web| {
        web.iter()
            .any(|(name, server)| name.ends_with(&host) && server["Handlers"]["/"]["Proxy"] == target)
    }))
}

fn run(command: &[&str]) -> io::Result<()> {
    let status = Command::new(command[0]).args(&command[1..]).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("`{}` {status}", command.join(" "))))
    }
}

fn attempt(command: &[&str], remedy: &str) -> bool {
    run(command)
        .inspect_err(|error| eprintln!("stampede: warning: {error}; to finish, run: {remedy}"))
        .is_ok()
}

fn unit_path(name: &str) -> PathBuf {
    crate::home().join(".config/systemd/user").join(format!("{name}.service"))
}

fn user() -> io::Result<String> {
    env::var("USER").map_err(|_| io::Error::other("USER is not set"))
}
