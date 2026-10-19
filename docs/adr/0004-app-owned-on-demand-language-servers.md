# Language servers are app-owned, pinned, and started only on demand

Stampede brings its own pinned language servers and runs one only while the Project's switch is on and an editor shows a file in its language; Agents never share them.

- Pinned, app-owned copies make behaviour deterministic.
- Nothing runs by default, and servers stop shortly after the last editor for their language closes.
- The TypeScript server is the Go version, so Node runs only for Svelte, only while a Svelte file is open.
- Rejected: sharing one server with Agents through an LSP multiplexer, because LSP is one client per server, multiplexers drop server-to-client requests, and a shared server would always be running.
- Rejected: using whatever is installed on the Host, because it isn't deterministic.
