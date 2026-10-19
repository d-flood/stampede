# Agent hooks are injected per Rig launch, not installed globally

Superseded by [ADR 0011](0011-rigs-run-only-chats.md). TUI lifecycle management and injected hooks have been removed.

Status hooks reach each Agent through its Rig's launch flags; Stampede never edits the user's global agent configuration.

- Launch-time settings layer on top of global settings rather than replacing them.
- Rigs are how Agents get started anyway.
- Accepted cost: an Agent started by hand gets no status and no Resume.
- Rejected: installing hooks globally so any Agent in any Terminal reports, because it modifies global agent config.
