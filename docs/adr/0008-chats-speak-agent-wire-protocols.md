# Chats speak each Agent's wire protocol from Rust, not through the official SDKs

The server drives Claude Code through its SDK's stdio protocol in a holder process per Chat, and drives Codex as a client of the `codex app-server` daemon.

- Both Agents are native binaries that speak these protocols directly, so no Node runs (ADR 0002).
- Claude's SDK is a library that spawns one process per session over stdio with no reattach, so a holder keeps a Chat alive across server restarts, as Terminals do (ADR 0005).
- Codex's daemon keeps threads running while clients come and go, and the TUI attaches to the same daemon, so a Chat and a TUI can share one live thread.
- Accepted cost: Claude's protocol has no published spec beyond the SDK's types; the Codex daemon is experimental, and restarting it ends every running Codex turn.
- Rejected: the official TypeScript SDKs, because Claude's needs Node and Codex's lacks approvals, streaming deltas, and edit diffs.
- Rejected: running Codex in a holder too, because it gives up live sharing with the TUI.
