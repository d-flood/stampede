# Language servers are app-owned, pinned, and started only on demand

Stampede brings its own pinned language servers and runs one only while the Project's switch is on and an editor shows a file in its language; Agents never share them.

- Pinned, app-owned copies make behaviour deterministic.
- Nothing runs by default, and servers stop shortly after the last editor for their language closes.
- The TypeScript server is the Go version, so Node runs only for Svelte, Astro, or ESLint, only while a file it serves is open.
- Linters run as extra, diagnostics-only servers beside a file's main server, and only when the Project configures them (Ruff: `ruff.toml`, `.ruff.toml`, or `[tool.ruff]` in `pyproject.toml`; ESLint: `eslint.config.*` and `node_modules/eslint`).
- ESLint is the exception to pinning: its config loads the Project's plugins, so a small Stampede shim runs the Project's own ESLint on the pinned Node.
- Rejected: sharing one server with Agents through an LSP multiplexer, because LSP is one client per server, multiplexers drop server-to-client requests, and a shared server would always be running.
- Rejected: using whatever is installed on the Host, because it isn't deterministic.
