# Session search is a disposable SQLite index, with Ended Sessions kept beside it

Each Host keeps a SQLite FTS5 index of its Sessions' snapshots for lists, search and Dormant Chats, rebuilt from native histories at will, plus an archive of Ended Sessions that is never rebuilt.

- One database serves the catalogue and full-text search, so warm lists and searches never wait on an Agent.
- Native histories stay authoritative; deleting the index costs only a reindex.
- Snapshots hold messages and Activity summaries, not tool payloads, so a Dormant Chat looks like the Chat it becomes.
- Claude Code deletes transcripts after 30 days by default, so Sessions it ages out become Ended Sessions; deliberate deletes are honoured.
- Rejected: separate caches for session lists and search.
- Rejected: mirroring native retention, because Claude Code's cleanup would empty search after a month.
- Rejected: copying native histories back from destroyed Cloud Hosts, because each Agent would need import code and Codex keeps state in its own database.
