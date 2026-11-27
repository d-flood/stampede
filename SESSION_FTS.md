# Session catalogue and full-text search

## Use case

Opening the Launchpad, New Tab menu, Chat list or Command Palette should immediately show recent sessions. Searching should find previous conversations by what David or the Agent said, not just by their titles. Results should include matching excerpts and preserve references to attached images, so a historical message can be displayed without shipping its image data over the WebSocket.

Today, `sessions.list` rebuilds the catalogue for a Project on every request (`device.rs`, `chats.rs`). The Command Palette does this for every open Project. The four Agents are queried sequentially:

- Claude Code reads and parses up to 2 MiB of JSONL per session, including unchanged sessions, on an async worker.
- Codex invokes the daemon startup command, establishes a proxy connection and fetches every page of `thread/list`.
- Copilot launches a CLI, negotiates its protocol and authentication, lists sessions and shuts down.
- Antigravity launches ACP, lists sessions across all directories and filters them by Project.

Search currently fuzzy-matches titles in the Device. The expensive part is obtaining the catalogue. Multiple surfaces and Devices repeat the same work, and one slow or failing Agent delays the others.

## Proposal: a Host-owned SQLite index

Create a session catalogue in Stampede's data directory, owned by a shared `chats/sessions.rs` service. Use SQLite with FTS5 rather than building separate caches for session lists and full-text search. One database per Host; Cloud Hosts index their own history. The Agents' native histories remain authoritative. The database is disposable and rebuildable.

| Table | Contents |
| --- | --- |
| `sessions` | Agent, native session ID, source identity, Project directory, title, modification time, indexed source revision |
| `messages` | Session reference, native message ID or replay position, order, role, text |
| `messages_fts` | FTS5 index of message text and searchable session titles |
| `attachments` | Message reference, ordered asset ID, media type, optional title |

Keep the session identity scoped to the Agent and history store. A session ID alone is insufficient when multiple Agents or accounts are involved. Index Project directories rather than transient open-Project IDs. Include the effective Rig environment and history/account identity in cache invalidation; don't persist credentials in cache keys.

List sessions by modification time directly from SQLite. Keep the existing recent-session and fuzzy-title affordances; add Host-side FTS queries for conversation content. Support phrase search, Project and Agent filters, and matching excerpts, deduplicating results by session while retaining the best matching message. All message searches should be available without invoking an Agent. Where stable native message identifiers exist, use them to navigate to the match after reopening the Chat.

## What to index

Use the existing distinction between `Item::User` and `Item::Text` in `chats.rs`. Include user prompts and Agent commentary and final responses. Exclude reasoning, tool calls and results, command output, diffs, system instructions and synthetic compaction summaries. Explicitly presented media is an attachment, not searchable tool output. Preserve message order and follow the same conversation branch and rewind semantics as normal Chat replay. Do not index abandoned messages merely because they remain in a native log.

Reuse the drivers' history decoding and classification where practical, but provide read-only extraction paths that skip unnecessary tool payloads, never start a turn and do not construct a live Chat:

| Agent | History source |
| --- | --- |
| Claude Code | Native JSONL; reuse the branch and message classification logic in `claude.rs`. |
| Codex | Native catalogue API and paginated `thread/items/list`. Avoid loading or resuming the thread to index it. |
| Copilot | Session catalogue and paginated persisted events; the driver already uses the persisted-events API for usage history. |
| Antigravity | ACP catalogue and history replay. Investigate a read-only history source before using `session/load` for bulk indexing. |

## Attachments

Reuse `media::cache_image` and the existing media store, which names user images by content hash and deduplicates them. Store only the resulting asset identifier in SQLite, not a blob, base64 string or arbitrary filesystem path. The Device resolves it through `/api/media/{asset}` using the existing Host-aware URL helpers, and loads images on demand.

Extract previously unseen embedded images when indexing their messages. Retain references to Agent-presented `Item::Media` assets as well. Preserve image-only messages even though they contain no searchable text. If an asset cannot be extracted or is missing, retain the message and mark the attachment unavailable; repair the reference on reconciliation when possible.

The media store and index have different lifetimes: rebuilding or deleting the index must not destroy assets still used by Chats. Any future media garbage collection must account for both indexed historical messages and other live or durable media references.

## Freshness and performance

- Serve the last successful catalogue immediately. On first use, index progressively in the background and expose incomplete coverage instead of claiming there are no matches.
- Coalesce simultaneous requests, and bound refresh concurrency across Projects and Agents. Refresh each provider independently; skip unsupported Agents on Cloud Hosts. Reuse a single Antigravity catalogue across directories where its source identity permits that.
- For Claude Code, enumerate/stat session files and compare a revision fingerprint (size, high-resolution modification time, inode/file identity and change time). Reparse only new or changed files. Check the revision again before committing results.
- For RPC providers, reconcile against successful native listings, and use any verified revision or change signal the provider exposes. Subscribe to Stampede's `Lifecycle::Session`, turn completion and `Lifecycle::Ended` events. Watch relevant native history stores to catch external TUI activity, renames and deletions; recover from missed events through reconciliation. An opaque provider without reliable change signals needs bounded periodic reconciliation while relevant history is in use.
- Initially reindex an entire changed session rather than attempting append-only parsing. Replace its messages, attachment references and FTS rows in one transaction. This handles rewrites, truncation and rewinds; optimize incremental replay later if measurements justify it.
- Remove sessions on confirmed deletion or an authoritative complete listing, not a failed or partial request. On refresh failure keep the last successful data, indicate its freshness and retry with backoff. Never promote an older refresh over a newer source revision.
- Filter live Chats from the past-session list at query time, not when indexing. Refresh visible results when the live set changes. Native resume remains authoritative: report a missing session or rejected resume, invalidate the stale entry and refresh.
- Keep scans, RPC calls, parsing and SQLite writes off UI-critical paths. Use a dedicated blocking worker for database/filesystem work, with short transactions and concurrent reads. Stop unnecessary reconciliation when history is not in use.

The target is a warm list or search in under 100 ms on the Host, without waiting for any Agent. The first full-text index may take time, particularly with image-heavy transcripts. The catalogue can populate before message indexing finishes; a session's metadata and message-index revisions should be tracked separately.

No cache can guarantee instantaneous knowledge of an external change hidden behind an Agent's RPC API. The correctness contract is a coherent last-known snapshot, prompt reconciliation, honest incomplete/stale state, and validation when reopening a session—not pretending that a TTL proves freshness.

## Verification

Measure cold indexing separately from warm list/search latency. Extend the existing session fixtures (`crates/stampede/tests/chat_sessions.rs`) to cover repeated and concurrent reads, external mutation/deletion, rewinds, failed refreshes and restart reconciliation. Verify that a term occurring only in tool output never matches and that attachment references display the same assets as Chat replay. Warm requests must not reread unchanged transcripts or launch provider processes.
