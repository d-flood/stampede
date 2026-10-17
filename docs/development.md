# Developing Oikonomos inside Oikonomos

Run `./rebuild` instead of `./serve`. It needs Linux with a systemd user manager, Python 3.9+, Rust, and pnpm. Stop any manually launched `./serve` once before switching.

```sh
./rebuild
```

The first run installs and enables `oikonomos.service`, enables lingering, and starts the server at http://127.0.0.1:4580. Agent commands inherit the installing shell's PATH. To configure Tailscale Serve too:

```sh
./rebuild --https-port 8443
```

`--port`, `--data-dir`, and `--config` accept the same values as `serve`. Later runs remember the service settings; `--local-only` skips Tailscale setup without removing existing Tailscale routes.

Every run locks deployment, builds the frontend and Rust while the old server stays available, atomically replaces `~/.local/lib/oikonomos/oikonomos`, and restarts the user service. It waits up to 30 seconds for `/api/version` to report the new build ID. Startup failure restores the previous binary and service configuration and restarts it. Build failure leaves the running server untouched. This does not undo data migrations or project edits.

Terminal holders, Claude holders, and the Codex daemon survive the service restart. The browser reconnects and offers Reload when its build differs; save dirty editors before reloading. Authorize the complete `./rebuild` command before the restart so recovery cannot get stuck awaiting an approval through the unavailable server. The agent also needs access to the user systemd bus and installation directory.

```sh
systemctl --user status oikonomos
journalctl --user -u oikonomos -f
```

The last binary is retained at `~/.local/lib/oikonomos/oikonomos.previous`. To recover manually, copy it to a temporary sibling, rename that over `oikonomos`, then run `systemctl --user restart oikonomos`.

Deployment checks: `python3 tests/rebuild_test.py`. Browser and agent recovery checks: `./check e2e rebuild.spec.ts` and `cargo test --test terminal_survives_restart --test chat_claude_restart --test chat_codex_restart`.
