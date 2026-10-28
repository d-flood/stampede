# Context Stampede adds to agents

Audited 2026-10-28 at `54a8449`. Ordinary chats have no Stampede-authored system/developer prompt prepended to each message. The shared baseline is seven MCP tool definitions. Other additions depend on explicit user actions or the run workflow. MCP is the best common tool interface for the three current harness integrations; changing transport alone does not remove tool context. See [provider interface research](provider-tool-delivery.md).

## Measured tool catalog

Baseline `tools/list` result: **7,706 UTF-8 bytes, 1,688 tokens** with tiktoken 0.14.0 / `o200k_base`. A simple equivalent array of function definitions (`type`, `name`, `description`, `parameters`) is 1,566 tokens. These measure serialized content, not actual provider prompt tokens or billing. Harnesses transform tool schemas, add names/wrappers, and may defer loading. Claude uses a different tokenizer. Canvas directory length also changes the count.

| Tool | Serialized MCP definition tokens |
| --- | ---: |
| `list_rigs` | 90 |
| `delegate_to_rig` | 257 |
| `ask_questions` | 577 |
| `stage_command` | 179 |
| `read_terminal` | 177 |
| `render_html` | 226 |
| `render_media` | 184 |

Descriptions alone total 822 tokens. `ask_questions` is about one third of the catalog, including its nested question/option/image schemas. `render_html` includes a chat-specific canvas directory and instructions to write editable HTML there. The other descriptions also carry behavioral instructions: how to delegate, ask questions, or wait for a staged command. Thus “no system prompt” does not mean “no Stampede instructions.” Sources: [catalog and MCP initialization](../../crates/stampede/src/chats/html.rs#L47), [delegation](../../crates/stampede/src/chats/delegation.rs#L57), [questions](../../crates/stampede/src/chats/questions.rs#L78), [terminal tools](../../crates/stampede/src/chats/terminal_tools.rs#L30).

The MCP initialization response advertises tools and server identity. It contains no server instructions; Stampede implements no MCP resource or prompt catalog. All seven definitions are returned without per-workflow filtering. The MCP JSON-RPC envelopes and executable arguments are transport/configuration data, not themselves model messages. Sources: [server](../../crates/stampede/src/chats/html.rs#L22).

## Additions outside MCP

| Route | What enters context |
| --- | --- |
| Ordinary message | User text and attached images; no Stampede prose prefix. |
| `/grill`, `/spec`, `/tickets` | Expanded templates: 1,872 / 604 / 1,685 tokens, plus supplied text. |
| Ticket run | 658-token template, ticket body/title/reference, branch, executable path. |
| Resolver / reviewer / remediator | 471 / 628 / 601-token template, plus workflow identifiers and paths. |
| Resumed ticket | Short continuation message instead of the ticket template. |
| Handoff | 172-token instruction plus optional focus goes to the old chat. Its generated handoff becomes an editable draft for the receiving chat. |
| Worktree relocation | Directory/branch/path-remapping notice is placed in the draft; it enters model context when sent. |
| User `!` shell command | Claude: captured command/stdout/stderr are prepended to the next message. Codex and Copilot use native harness shell-session requests. |
| Codex `/skill` | Matching skill is explicitly attached as a skill input; its content is supplied by the harness. |
| Commit-message generation | Separate one-shot prompt: commit instructions, 40-token commit template, recent subjects, staged summary and diff, with diff capped at 60,000 bytes. |

Template counts use the same tokenizer, trimming trailing whitespace. A nondefault data directory adds a sentence to template openers instructing the agent to pass `--data-dir`. The ticket/brief contents fetched later are additional tool results, not all automatically inserted at startup. Sources: [message/template expansion](../../crates/stampede/src/chats.rs#L1464), [templates](../../crates/stampede/src/templates.rs#L14), [run prompts](../../crates/stampede/src/runs.rs#L934), [handoff](../../crates/stampede/src/chats.rs#L1440), [relocation draft](../../crates/stampede/src/chats.rs#L1065), [Claude shell context](../../crates/stampede/src/chats/claude.rs#L288), [Codex input](../../crates/stampede/src/chats/codex.rs#L322), [Copilot shell](../../crates/stampede/src/chats/copilot.rs#L367), [commit drafting](../../crates/stampede/src/git/actions.rs#L165).

The file picker inserts a path into the composer; it does not automatically attach the file contents. [File selection](../../web/src/lib/chat/FileMenu.svelte#L28).

## Indirect context and growing history

Stampede also changes the execution environment. It sets scratch/cache/temp/build paths, cwd, permission mode, writable roots and model settings. These can affect harness-generated environment and permission instructions, but Stampede does not serialize them into an extra prose prompt itself. Codex enables native `update_plan`; Claude enables Todo tools through its environment. These alter available native tool context beyond the seven MCP tools. Sources: [storage environment](../../crates/stampede/src/storage.rs#L41), [Codex config](../../crates/stampede/src/chats/codex.rs#L540), [Codex setting changes](../../crates/stampede/src/chats/codex.rs#L920), [Claude launch](../../crates/stampede/src/chats/claude.rs#L199), [Copilot configuration](../../crates/stampede/src/chats/copilot.rs#L283).

Normal chats also retain the harness's own prompts, repository instructions, user settings, plugins, skills, built-in tools and resumed history. Those are not all Stampede-authored context. The isolated commit-generation paths disable much of that configuration separately. [One-shot calls](../../crates/stampede/src/rigs.rs#L109).

Tool arguments/results can exceed the static catalog substantially: HTML input permits 256 KiB; terminal snapshots permit 64 KiB; delegation returns a task and report. Terminal responses include both text JSON and `structuredContent`; actual duplication in model context depends on the harness. The MCP transport does not make these results free. Sources: [HTML input](../../crates/stampede/src/chats/html.rs#L12), [terminal results](../../crates/stampede/src/chats/terminal_tools.rs#L14), [delegation report](../../crates/stampede/src/chats/delegation.rs#L31).

## Method and limits

Extracted the schema-producing functions from the current Rust source into a temporary program linked to the existing `serde_json` dependency. Its output matched the existing debug binary's `stampede chat-tools --chat-id context-audit` output exactly. Tokenized compact JSON with `o200k_base`, using the default local data directory and sample chat ID. All extraction/counting programs and outputs were kept in `/tmp`; no source behavior changed.

Also tried Claude Code 2.1.311's `get_context_usage` immediately after initialization, with and without the MCP server, without sending a model prompt. Both reported 2,237 system tokens, no MCP tools, and `apiUsage: null`. This pre-turn probe does not establish MCP overhead or successful tool discovery and is excluded from the measured catalog estimate. No paid model requests were used. Effective initial and post-discovery context still needs a controlled per-provider model-turn comparison if exact harness token totals are required.

For reducing context, first shorten definitions and large results, then evaluate workflow filtering and supported deferred loading. Native adapters are justified by concrete capability/lifecycle needs, not by assuming MCP itself causes large context overhead.

## Applied reductions

The implementation keeps MCP and shortens all seven descriptions while preserving question features, sandbox restrictions, delegation authorization and cancellation semantics. With the same tokenizer and sample chat ID, the final parent catalog is **6,115 bytes / 1,366 tokens**, a **19.1%** reduction. Delegated children expose five tools, **1,102 tokens**; both delegation tools are hidden and reject direct calls there.

Terminal reads default to 50 lines and 8 KiB, with an optional `max_bytes` parameter allowing up to 64 KiB. Delegation replies contain the child ID, rig, outcome and report rather than echoing the task and UI metadata. Reports over 8 KiB return a Unicode-safe excerpt, a truncation flag and an absolute path to the full text. Full metadata and reports are retained locally for display and replay; legacy results still replay. Large HTML is directed to existing canvas files.

Regression coverage includes explicit terminal bounds, child catalogs, failed/stopped results, oversized reports, Claude/Codex restart replay, and Copilot live/replayed result handling.

Verification: 76 unit tests and all nine delegation integration tests passed. The full integration suite initially passed 232 tests and failed commit-message drafting and usage-window delivery; both failures were reproduced against a freshly compiled unchanged `54a8449` snapshot. The final run, excluding those two baseline failures, passed 231 tests but timed out in the terminal-flood stress test; that test also timed out on an isolated retry after passing in the initial run. No terminal-flood implementation was changed.
