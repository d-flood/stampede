# Developing Stampede inside Stampede

## New machine

```sh
./setup
```

Needs Linux with a systemd user session. It installs missing toolchains without sudo: Rust via rustup, Zig 0.16 in `~/.local/zig`, and Node with pnpm in `~/.local`. Tools already present at a suitable version are reused. When Tailscale is running with HTTPS certificates enabled, it serves Stampede at `https://<machine>.<tailnet>.ts.net:8443`. It then runs `./rebuild` and prints the remaining sudo commands (base packages, lingering, Tailscale operator) and agent logins. Rerun it after those steps; it accepts the same options as `./rebuild`.

## Rebuilding

Use a debug binary for the local development loop:

```sh
./serve --dev --port 4581
```

This reuses frontend dependencies and output while unchanged, builds Rust in debug mode, and runs in the foreground. It shares Cargo dependency artifacts across the repository's worktrees; switching checkouts invalidates Stampede's own compilation so binaries and tests use the correct sources and asset directory. Debug builds read frontend assets from disk, so rebuilding the frontend does not recompile Rust. Restart the development Host after Rust changes.

Use `./rebuild` to deploy the release binary to the systemd service. It needs Linux with a systemd user manager, Python 3.9+, Rust, and pnpm. Stop any manually launched `./serve` on the service's port once before switching.

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

Deployment checks: `./check runner`. Browser and agent recovery checks: `./check e2e rebuild.spec.ts` and `./check rust --test integration terminal_survives_restart::` (also `chat_claude_restart::` and `chat_codex_restart::`).


## Checks

```sh
./check
./check rust --test integration files::
./check e2e editor.spec.ts
./check components text.test.ts
./check runner
./check perf
./check all
./check prune
```

`./check` runs frontend type checks, Vitest unit/component tests, functional Rust tests, deployment tests, and desktop browser tests. `./check perf` runs Rust stress/memory checks and browser timing and memory budgets, serially, on an otherwise idle machine. `./check all` runs both suites and adds the phone browser tests. Rust performance cases are marked ignored in the functional suite; they remain part of the complete check.

`./check components` runs browser component tests without a Rust build or a production frontend build. It uses Vitest Browser Mode to mount production Svelte components with their styles and fonts, alongside ordinary Node unit tests. These tests also run with the default check. Filter by filename or use `--project unit`, `--project desktop-components`, or `--project pixel-components`. Keep rendering, formatting, and isolated interaction cases here; reserve Playwright E2E for Host/provider wiring, persistence, and navigation. Browser suites request reduced motion and assert settled behaviour rather than animation timing or frame counts.

`./check runner` runs the Python check-runner, build-storage, and deployment tests without frontend dependencies or Rust compilation.

An overlapping check in the same repository, including another worktree, exits immediately. Interrupting a check stops its children and detached test processes. Each E2E test has its own Host, temporary data, and port, cleaned up immediately after the test; its fixture repositories are copied from a template prepared once per worker. Vitest allocates its own free local ports. `./check e2e` and `./check all` run browser tests on both the desktop and phone projects unless tagged `@desktop` or `@phone`; a plain `./check` runs only the desktop project, while Vitest component tests still cover both form factors. Tag tests whose behaviour doesn't depend on the form factor so they run once.

Frontend dependencies and builds are reused while their content is unchanged. Additions, deletions, configuration/version changes, and missing or modified build output invalidate the frontend build. Each stage reports elapsed time separately, including Rust compilation and test execution. Logs are replaced on each run in `target/check/`; failures print the log tail. Rust integration tests compile into one `integration` test binary; filter by file with its module path, such as `files::`. E2E uses the same development binary as Rust tests, with line tables for Stampede and no debug information for dependencies. Release builds remain part of `./rebuild` and continue to embed frontend assets.

Run `./check prune` for cache maintenance. It removes abandoned temporary build directories and build artifacts that no build has read for a week, skipping a profile while Cargo is building it. Builds and ordinary checks do not scan or prune the build cache at startup.

Defaults leave room for the desktop: four Cargo jobs, four Rust test threads, two Tokio workers per runtime, and three Playwright workers, balancing individual isolated tests across files. Override with `CARGO_BUILD_JOBS`, `RUST_TEST_THREADS`, `TOKIO_WORKER_THREADS`, or Playwright's `--workers` argument. Vitest also defaults to three workers.

Browser tests require a real GPU. Install full Chromium with `pnpm -C web exec playwright install chromium --no-shell`, and run checks with GPU access outside the agent sandbox. The [Chromium channel](https://playwright.dev/docs/browsers#chromium-new-headless-mode) uses full headless Chrome; Linux uses the [Vulkan ANGLE backend](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/using-gpu-hardware-in-headless-chrome.md) without requiring `DISPLAY`. SwiftShader fallback is disabled, and hardware compositing, rasterization, and WebGL are checked before and after each Playwright worker or Vitest browser test file. Software rendering fails the check.
