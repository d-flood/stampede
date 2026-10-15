# The server is written in Rust, not Node

The long-running server that owns Terminals, language servers, files, and git is a Rust binary.

- Avoids paying Node's memory cost for a process that runs all the time.
- Node is then needed only by the Svelte language server, and only while it runs.
- Rejected: Node or Bun, even though the terminal and LSP libraries closest at hand are JavaScript, because of the memory cost.
