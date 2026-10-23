# Stampede

A self-hosted IDE for directing AI coding agents, served from your development machine and used from a browser on any computer or phone.

## Requirements

- Linux with a systemd user session
- A C compiler, `git`, `curl`, `xz` and Python 3.9+ (`./setup` prints the install command if any are missing)
- Optional: [Claude Code](https://claude.com/claude-code) or [Codex](https://github.com/openai/codex) to run agents, and [Tailscale](https://tailscale.com) to reach Stampede from other devices

`./setup` installs everything else (Rust, Zig, Node and pnpm) into your home directory without sudo.

## Setup

```sh
git clone https://github.com/d-flood/stampede.git
cd stampede
./setup
```

This builds Stampede, installs it as a systemd user service and starts it at http://127.0.0.1:4580. It ends by listing any sudo commands or agent logins still needed. Run those, then run `./setup` again.

## Updating

```sh
git pull
./rebuild
```

The old server keeps running while the new one builds. If the new build fails to start, the previous one is restored.

## Running without the service

```sh
./serve
```

Builds and runs Stampede in the foreground at the same address. Stop it before using `./rebuild`.

## Other devices

With Tailscale running, enable MagicDNS and HTTPS Certificates for your tailnet, then run `./setup` again. Stampede is then served at `https://<machine>.<tailnet>.ts.net:8443`.

## Logs

```sh
systemctl --user status stampede
journalctl --user -u stampede -f
```

For development and checks, see [docs/development.md](docs/development.md).
