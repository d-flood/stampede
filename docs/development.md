# Developing Stampede inside Stampede

## New machine

```sh
./setup
```

Needs Linux with a systemd user session. It installs missing toolchains without sudo: Rust via rustup, Zig 0.16 in `~/.local/zig`, and Node with pnpm in `~/.local`. Tools already present at a suitable version are reused. When Tailscale is running with HTTPS certificates enabled, it serves Stampede at `https://<machine>.<tailnet>.ts.net:8443`. It then runs `./rebuild` and prints the remaining sudo commands (base packages, lingering, Tailscale operator) and agent logins. Rerun it after those steps; it accepts the same options as `./rebuild`.

## Rebuilding

Run `./rebuild` instead of `./serve`. It needs Linux with a systemd user manager, Python 3.9+, Rust, and pnpm. Stop any manually launched `./serve` once before switching.

```sh
./rebuild
```

The first run installs and enables `stampede.service`, enables lingering, and starts the server at http://127.0.0.1:4580. Agent commands inherit the installing shell's PATH. To configure Tailscale Serve too:

```sh
./rebuild --https-port 8443
```

`--port`, `--data-dir`, and `--config` accept the same values as `serve`. Later runs remember the service settings; `--local-only` skips Tailscale setup without removing existing Tailscale routes.

Every run locks deployment, builds the frontend and Rust while the old server stays available, atomically replaces `~/.local/lib/stampede/stampede`, and restarts the user service. It waits up to 30 seconds for `/api/version` to report the new build ID. Startup failure restores the previous binary and service configuration and restarts it. Build failure leaves the running server untouched. This does not undo data migrations or project edits.

Terminal holders, Claude holders, and the Codex daemon survive the service restart. The browser reconnects and offers Reload when its build differs; save dirty editors before reloading. Authorize the complete `./rebuild` command before the restart so recovery cannot get stuck awaiting an approval through the unavailable server. The agent also needs access to the user systemd bus and installation directory.

```sh
systemctl --user status stampede
journalctl --user -u stampede -f
```

The last binary is retained at `~/.local/lib/stampede/stampede.previous`. To recover manually, copy it to a temporary sibling, rename that over `stampede`, then run `systemctl --user restart stampede`.

Deployment checks: `python3 tests/rebuild_test.py`. Browser and agent recovery checks: `./check e2e rebuild.spec.ts` and `cargo test --test terminal_survives_restart --test chat_claude_restart --test chat_codex_restart`.


## Checks

```sh
./check
./check rust --test files
./check e2e editor.spec.ts
```

`./check` runs frontend type checks, runner/GPU guard tests, Rust tests, deployment tests, and desktop/phone E2E tests. An overlapping check in the same repository, including another worktree, exits immediately. Interrupting a check stops its children and detached test processes; each E2E test has its own server, temporary data, and port, cleaned up immediately after the test.

Frontend dependencies and builds are reused while their content is unchanged. Additions, deletions, configuration/version changes, and missing or modified build output invalidate the build. Frontend build, Rust test, and runner test logs are replaced on each run in `target/check/`; failures print the log tail. E2E uses the same development binary as Rust tests, with line tables instead of full debug information. Release builds remain part of `./rebuild`.

Defaults leave room for the desktop: four Cargo jobs, four Rust test threads, two Tokio workers per runtime, and two Playwright workers, balancing individual isolated tests across files. Override with `CARGO_BUILD_JOBS`, `RUST_TEST_THREADS`, `TOKIO_WORKER_THREADS`, or Playwright's `--workers` argument.

E2E requires a real GPU. Install full Chromium with `pnpm -C web exec playwright install chromium --no-shell`, and run checks with GPU access outside the agent sandbox. The [Chromium channel](https://playwright.dev/docs/browsers#chromium-new-headless-mode) uses full headless Chrome; Linux uses the [Vulkan ANGLE backend](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/using-gpu-hardware-in-headless-chrome.md) without requiring `DISPLAY`. SwiftShader fallback is disabled, and each test checks hardware compositing, rasterization, and WebGL before and after running. Software rendering fails the test.
