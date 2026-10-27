# Context size after compaction: what Claude Code and Codex report

Researched 2026-10-27 against the installed versions: Claude Code 2.1.311 (`~/.local/bin/claude` -> `~/.local/share/claude/versions/2.1.311`), paired with `@anthropic-ai/claude-agent-sdk` 0.3.311 (its `claudeCodeVersion` is `2.1.311`), and codex-cli 0.164.0 (tag `rust-v0.164.0`, commit `6aaac694803004a63c6f393d153c96d22521a6b9`).

## Short answers

**Claude Code: the size is partly reported straight away, and the full figure is there if you ask for it.**
Claude Code emits the `compact_boundary` system message as soon as compaction finishes, before the next turn. Its `compact_metadata.post_tokens` field holds a post-compaction count. That field is typed in the SDK `.d.ts` but missing from the docs page. It is only an estimate of the new *message list*: the summary, re-attached files and hook output. It leaves out the system prompt and the tool definitions, so on its own it is not a context-meter value. In the recorded fixture, `post_tokens` was 2,521 while the next API call actually sent 21,595 tokens. `pre_tokens` is on a different basis: it is the full pre-compaction context, taken from API usage.

Claude Code never pushes the full post-compaction size. The `result` for `/compact` carries all-zero `usage`. You can still get the size straight away with the `get_context_usage` control request (`Query.getContextUsage()` in the SDK). Right after a compact, it returns an estimated `totalTokens` covering system prompt, tools and messages. In a live run that figure came within 3% of the next real call.

The docs also say the status line's `context_window.current_usage` is `null` after `/compact` until the next API call. So any meter built on "last API usage", including the official status line, stays stale or blank until the next model call.

**Codex: yes, the new size is pushed straight away (as an estimate).**
Every compaction path in codex-core calls `recompute_token_usage` just before the `contextCompaction` item completes. That call emits `thread/tokenUsage/updated` with these values:
- `last.totalTokens` is a local, byte-based estimate of the new history plus base instructions. Codex's own comment calls it "a coarse lower bound".
- Every other `last.*` field is `0`.
- `total` is unchanged.

The Codex TUI applies every `thread/tokenUsage/updated` to its footer, so going by the source its meter moves when compaction completes. Stampede's Codex driver already applies the update too. The estimate reads low: in the fixture it was 14,978 against 22,413 on the next real call.

**Verdict.** The delay comes mostly from how clients use the data, not from what the agents emit:
- **Codex** sends a usable estimate.
- **Claude Code** sends a partial number (messages only) and offers the full estimate on request.
- **Meters keyed to the last API response** go stale. That covers Stampede's Claude meter and the Claude status line, which the docs describe as going null after a compact.

## Method

- **Claude Code:**
  - SDK types: `sdk.d.ts` from the npm tarball `@anthropic-ai/claude-agent-sdk@0.3.311` (https://registry.npmjs.org/@anthropic-ai/claude-agent-sdk/-/claude-agent-sdk-0.3.311.tgz, also at https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.311/sdk.d.ts). No copy was in `node_modules`.
  - Docs: code.claude.com, fetched 2026-10-27.
  - Minified bundle: strings inside the installed 2.1.311 binary, for how `post_tokens` is computed. This part is labelled as inference throughout.
  - Wire traffic: the repo's recorded fixtures.
- **Codex:**
  - Source at `rust-v0.164.0`; every link below is pinned to commit `6aaac69`.
  - Generated JSON schema: `codex app-server generate-json-schema` from the installed 0.164.0, written to the scratchpad only.
  - Wire traffic: the repo's recorded fixtures (`cliVersion` 0.163.0).
- **Live experiments:**
  - **Claude: one, run.** Model `haiku`, with `--tools "" --setting-sources "" --strict-mcp-config --no-session-persistence`, in a fresh scratchpad directory. The run went:
    1. Prompt "Reply with the single word: ok".
    2. `get_context_usage` (`summary`).
    3. `/compact`.
    4. `get_context_usage` (`summary`), then `get_context_usage` (`full`).
    5. Prompt "Reply with the single word: done", then `get_context_usage` (`summary`).

    The final one-word prompt goes beyond the minimum (one prompt plus a compact). I added it so the post-compaction estimates could be compared with a real API count in the same session. In total it made three model calls (first prompt, compaction summary, final prompt) plus the `full` token-count requests.
  - **Codex: none.** The source and two recorded fixtures were enough.

## Evidence: Claude Code

### Verified

1. **The `compact_boundary` message type.** `SDKCompactBoundaryMessage` in SDK 0.3.311 carries ([sdk.d.ts:3844-3878](https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.311/sdk.d.ts)):
   - `type: 'system'`, `subtype: 'compact_boundary'`, `uuid`, `session_id`.
   - `compact_metadata.trigger: 'manual' | 'auto'`.
   - `compact_metadata.pre_tokens: number`.
   - `compact_metadata.post_tokens?: number` (line 3850).
   - `compact_metadata.duration_ms?: number`.
   - `compact_metadata.preserved_segment?: { head_uuid, anchor_uuid, tail_uuid }`.
   - `compact_metadata.preserved_messages?: { anchor_uuid, uuids }`.

   `post_tokens` and `duration_ms` have no doc comments. The declaration has blank-line gaps where members were stripped. On the wire the message also carries `cumulative_dropped_tokens`, `pre_compact_discovered_tools`, `preserved_messages.all_uuids` and a top-level `logical_parent_uuid`. All four appear in `crates/stampede/tests/fixtures/chats/claude-code/tools.recorded.jsonl:177` (2.1.301) and in the live 2.1.311 run.
2. **The docs list only `trigger` and `pre_tokens`.**
   - The TypeScript reference shows `SDKCompactBoundaryMessage` with just those two fields under `compact_metadata` (https://code.claude.com/docs/en/agent-sdk/typescript#sdkcompactboundarymessage).
   - The `/compact` example reads only `pre_tokens` (https://code.claude.com/docs/en/agent-sdk/slash-commands, "Compact history with /compact").
   - The agent-loop page says `compact_boundary` "fires after compaction" (https://code.claude.com/docs/en/agent-sdk/agent-loop#automatic-compaction).
3. **`post_tokens` is not new.** It first appears in SDK 0.2.105 (`claudeCodeVersion` 2.1.105) at `sdk.d.ts:2027` (https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.2.105/sdk.d.ts). It is absent from 0.2.104 (https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.2.104/sdk.d.ts).
4. **The recorded wire sequence for a manual `/compact`** (`claude-code/tools.recorded.jsonl`, Claude Code 2.1.301):
   - :125 The last main-loop `message_start` before the compact sent `2 + 153 + 25805 = 25960` input tokens.
   - :168 stdin `/compact`.
   - :172 `{"type":"system","subtype":"status","status":"compacting"}`.
   - :175 `{"subtype":"status","status":null,"compact_result":"success"}`.
   - :177 `compact_boundary` with `"pre_tokens": 26248, "post_tokens": 2521, "cumulative_dropped_tokens": 23727`.
   - :185 `result` with `local_command: "compact"`, `num_turns: 0`, every `usage` field `0`, and `modelUsage` still cumulative.
   - :195 The next turn's `message_start` sent `2 + 8326 + 13267 = 21595`. :203 confirms it as `first_request_input_tokens: 21595`.
   - :13 For comparison, the session's first call sent 21,714. That is the fixed system-prompt and tools overhead plus the first prompt.

   `commands.recorded.jsonl:40` is a second compact with `pre_tokens: 23631, post_tokens: 3144`. It has no later turn to compare with.
5. **The live run on 2.1.311** (raw capture at `/home/david/.local/share/stampede/storage/scratch/99785a69c5a3d856/claude-1000/-home-david-repos-stampede/10db5706-d1ed-446d-b95e-e76b917a9cdb/scratchpad/live-claude/raw.jsonl`, which is temporary):

   | Point | Signal | Value |
   |---|---|---|
   | After the first turn | last call input (`2 + 3396 + 649`) | 4,047 |
   | After the first turn | `get_context_usage` summary `totalTokens` (system_prompt 3,238 + messages 809) | 4,047 |
   | Compaction finished | `compact_boundary` `pre_tokens` / `post_tokens` / `cumulative_dropped_tokens` | 4,051 / **797** / 3,254 |
   | Compaction finished | `/compact` `result.usage` | all 0; `first_request_input_tokens: null` |
   | Right after compaction | `get_context_usage` summary `totalTokens` (system_prompt 3,238 + messages 1,198), `apiUsage: null` | **4,436** |
   | Right after compaction | `get_context_usage` full `totalTokens` (system_prompt 3,153 + messages 1,294) | 4,447 |
   | Next real call | `message_start` input (`2 + 1481 + 3104`), which includes the new prompt | 4,587 |

   In this tiny session the summary was longer than the original exchange, so compaction made the context bigger. `post_tokens` alone (797) would have shown the meter at about 17% of the true value. `get_context_usage` was within about 3%.
6. **Status messages carry no token counts.** `SDKStatusMessage` holds only `status: 'compacting' | 'requesting' | null`, `permissionMode?`, `compact_result?` and `compact_error?` ([sdk.d.ts:6192-6203](https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.311/sdk.d.ts)).
7. **The compaction hooks carry no token counts either.**
   - `PostCompactHookInput` has `trigger` and `compact_summary` only ([sdk.d.ts:2635-2642](https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.311/sdk.d.ts)).
   - `PreCompactHookInput` has `trigger` and `custom_instructions` (sdk.d.ts:2759-2763).
8. **Result `usage` can't serve as the meter value.**
   - `usage` is "MAIN AGENT LOOP ONLY … per-turn in streaming-input sessions" (sdk.d.ts:6082).
   - `modelUsage` is cumulative across turns and includes "internal calls such as compaction" (sdk.d.ts:6086-6088).
   - `ModelUsage.contextWindow` is the model's window (sdk.d.ts:1488).
9. **`getContextUsage()` / `get_context_usage`.**
   - The method is defined at sdk.d.ts:3103-3115 and the request type at 4047-4053 (`detail?: 'summary' | 'full'`).
   - The response type `SDKControlGetContextUsageResponse` (4058-4185) has `totalTokens`, `maxTokens`, `rawMaxTokens`, `percentage`, `categories[]` with `id`/`kind`, and `apiUsage | null`.
   - The docs say "`totalTokens` is the session's current context usage, and `maxTokens` is the window that usage is measured against. That window is the model's context window, or the lower auto-compaction window when one applies." They also say `apiUsage` "holds the usage from the latest API response" (https://code.claude.com/docs/en/agent-sdk/typescript#sdkcontrolgetcontextusageresponse).
   - `'summary'` "answers from the last response's usage and local estimates without the per-category token-count calls" (sdk.d.ts:4050). `'full'` makes token-counting requests, which "don't appear in the message stream" and, on the Anthropic API, "token counting isn't billed" (docs page above). In the live run, `summary` right after the compact returned `apiUsage: null` and a locally estimated total, so it did not reuse the stale pre-compact usage.
10. **The official status line goes blank after a compact.** The docs say `context_window.current_usage` is "`null` before the first API call in a session, and again immediately after `/compact` until the next API call repopulates it". They also say the status line re-runs when "`/compact` finishes", and that `used_percentage` is computed from the most recent API response's input tokens (https://code.claude.com/docs/en/statusline#context-window-fields). This is a first-party statement that Claude Code's own TUI surface has no fresh API-derived context number after a compact.

### Inferred (from the minified 2.1.311 bundle; byte offsets in `~/.local/share/claude/versions/2.1.311`)

- **How `post_tokens` is computed.** Near offset 250847202, the main compaction path does `po=mg([ln,...Kn,...Qt,...mn]); ln.compactMetadata.postTokens=po`. Here `ln` is the boundary marker, `Kn` the summary message(s), `Qt` the re-attached attachments and `mn` the hook results. `mg` is the message token estimator. No system prompt or tool schemas are included. Two other compaction paths assign `postTokens` the same way, at offsets 250574594 and 250578091. Both numeric data points are consistent with this: 2,521 vs 21,595, and 797 vs 4,436–4,587.
- **`pre_tokens` and `post_tokens` are on different bases.** `pre_tokens` (4,051) equals the last call's input plus output (4,047 + 4): the full context. `post_tokens` covers messages only. So `cumulative_dropped_tokens = pre_tokens - post_tokens` overstates the savings by the size of the system prompt plus tools.
- **Claude Code uses `post_tokens` internally.** After compaction it stores `post_tokens` as its own "last main-thread context tokens" (`$3t(Ae??0)` → `replaceLastMainThreadContextTokens`, near offsets 250848233 and 238767748). A helper near offset 250531258 returns, in order:
  1. the last assistant usage after the boundary;
  2. otherwise the boundary's `postTokens`;
  3. otherwise a local estimate.

  So Claude Code's internal stand-in for context size after a compact is the same messages-only number. I did not trace which UI surfaces read it, and I did not observe the TUI's built-in indicator live.
- **Auto-compaction.** The auto/reactive compaction path reads `boundaryMarker.compactMetadata.postTokens` too, so `post_tokens` should be present on `trigger: "auto"` boundaries. Only manual compaction was observed.

## Evidence: Codex

All links are pinned to `rust-v0.164.0` = [`6aaac69`](https://github.com/openai/codex/tree/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs).

### Verified

1. **The notification.** `ThreadTokenUsageUpdated => "thread/tokenUsage/updated"` is defined at [app-server-protocol/src/protocol/common.rs:1998](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server-protocol/src/protocol/common.rs#L1998). Its payload:
   - `{ threadId, turnId, tokenUsage }` ([v2/thread.rs:1940-1944](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server-protocol/src/protocol/v2/thread.rs#L1940-L1944)).
   - `tokenUsage` = `{ total, last, modelContextWindow: number | null }`, mapped from core's `total_token_usage` / `last_token_usage` ([thread.rs:1980-1997](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server-protocol/src/protocol/v2/thread.rs#L1980-L1997)).
   - Each breakdown has `totalTokens`, `inputTokens`, `cachedInputTokens`, `cacheWriteInputTokens`, `outputTokens` and `reasoningOutputTokens` ([thread.rs:2001-2015](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server-protocol/src/protocol/v2/thread.rs#L2001-L2015)).

   The schema generated from the installed 0.164.0 (`v2/ThreadTokenUsageUpdatedNotification.json`) matches.
2. **The compaction messages carry no sizes.**
   - The `contextCompaction` thread item has only `id` ([v2/item.rs:432-434](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server-protocol/src/protocol/v2/item.rs#L432-L434)).
   - `thread/compact/start` takes `{ threadId }` and returns `{}` ([common.rs:747-750](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server-protocol/src/protocol/common.rs#L747-L750), [thread.rs:1179-1186](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server-protocol/src/protocol/v2/thread.rs#L1179-L1186)).
   - The deprecated `thread/compacted` notification is just `{ threadId, turnId }` ([common.rs:2048-2049](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server-protocol/src/protocol/common.rs#L2048-L2049), [thread.rs:2174-2181](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server-protocol/src/protocol/v2/thread.rs#L2174-L2181)).

   The size arrives only through `thread/tokenUsage/updated`.
3. **The app-server forwards every core `TokenCount` event** as `thread/tokenUsage/updated`, with no compaction special case ([app-server/src/bespoke_event_handling.rs:1040-1043](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server/src/bespoke_event_handling.rs#L1040-L1043), [1599-1615](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/app-server/src/bespoke_event_handling.rs#L1599-L1615)).
4. **What `recompute_token_usage` emits** ([core/src/session/mod.rs:5223-5263](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/session/mod.rs#L5223-L5263)):
   - It estimates the new history with `estimate_token_count_with_base_instructions`.
   - It sets `last_token_usage` to `{ input: 0, cached_input: 0, cache_write_input: 0, output: 0, reasoning_output: 0, total: estimate }`.
   - It refreshes `model_context_window` and leaves `total_token_usage` alone.
   - It then calls `send_token_count_event` ([5299-5306](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/session/mod.rs#L5299-L5306)).
5. **Every compaction path calls it before the item completes:**
   - Local summarisation: [compact.rs:436-440](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/compact.rs#L436-L440).
   - Remote compaction v2: [compact_remote_v2_installation.rs:277-280](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/compact_remote_v2_installation.rs#L277-L280).
   - Token-budget "new context window": [session/mod.rs:4893-4948](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/session/mod.rs#L4893-L4948).

   Manual compaction picks remote v2 or local by provider capability ([tasks/compact.rs:63-95](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/tasks/compact.rs#L63-L95)). Pre-, mid- and post-turn auto-compaction pick the same functions ([session/turn.rs:1540-1620](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/session/turn.rs#L1540-L1620)).
6. **The local path emits an extra update first.** It first emits a `TokenCount` for the summarisation request's own usage ([compact.rs:895-896](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/compact.rs#L895-L896)), then the estimate. The remote v2 path records the request via `record_observed_response_completed` ([compact_remote_v2.rs:440-447](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/compact_remote_v2.rs#L440-L447)), which sends `RawResponseCompleted`, not `TokenCount` ([session/mod.rs:5108-5126](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/session/mod.rs#L5108-L5126)). Either way, the post-compaction size is the *last* `thread/tokenUsage/updated` before `item/completed` for the `contextCompaction` item.
7. **The estimate is a lower bound.** "Estimate token usage using byte-based heuristics … This is a coarse lower bound, not a tokenizer-accurate count" ([context_manager/history.rs:650-651](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/context_manager/history.rs#L650-L651)). Base instructions are added only when the history doesn't already record them or tool declarations; items are summed per envelope ([661-680](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/context_manager/history.rs#L661-L680)).
8. **The recorded wire sequence** (`crates/stampede/tests/fixtures/chats/codex/tools.recorded.jsonl`, `cliVersion` 0.163.0):
   - :82 last turn `last.totalTokens: 26364`, `total.totalTokens: 48554`, `modelContextWindow: 475000`.
   - :88 `turn/started`, then :89 `item/started` `contextCompaction`.
   - :90 `thread/tokenUsage/updated` with `last = { totalTokens: 14978, everything else 0 }`; `total` still 48554.
   - :92 `item/completed` `contextCompaction`, then :94 `turn/completed`.
   - :105 the next real turn, `last.totalTokens: 22413` (`inputTokens: 22408`).

   A second capture, `codex/commands.recorded.jsonl:39-51`, goes from `last` 29,298 to an estimate of 14,094 between `item/started` and `item/completed`.
9. **The TUI applies every update.**
   - It passes each `ThreadTokenUsageUpdated` to `set_token_info` ([tui/src/chatwidget/protocol.rs:61-65](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/tui/src/chatwidget/protocol.rs#L61-L65)), which calls `apply_token_info`.
   - `apply_token_info` computes "% left" from `last_token_usage` ([tui/src/chatwidget.rs:1132-1160](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/tui/src/chatwidget.rs#L1132-L1160)).
   - The percentage first subtracts a fixed `BASELINE_TOKENS = 12000` from both the used count and the window ([protocol/src/protocol.rs:2547-2591](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/protocol/src/protocol.rs#L2547-L2591)).
10. **Not a recent change.** The `recompute_token_usage` call in compaction is present at `rust-v0.60.1`, 0.70.0, 0.80.0, 0.100.0, 0.130.0, 0.150.0, 0.163.0 and 0.164.0. It is absent from `codex-rs/core/src/codex/compact.rs` at 0.40.0 and 0.50.0. I checked those tags directly, not every release.

### Inferred

- **The estimate reads low.** It typically misses tool schemas: fixture estimate 14,978 vs the next real call's 22,413, about 33% low. That fits "lower bound" and the tool-declaration condition in `history.rs:661-680`.
- **An estimated update has a recognisable shape:** `last.totalTokens > 0` with `last.inputTokens == 0 && last.outputTokens == 0`. Only `recompute_token_usage` writes that pattern. This is a client-side heuristic, not a protocol flag.
- **Why the TUI change can look like nothing.** The Codex TUI should move at compaction completion. On a large window the 12k baseline normalisation can make the change small (fixture: about 97% left before, 99% after). I did not watch the TUI live.

## How Stampede handles it today

**Claude Code driver** (`crates/stampede/src/chats/claude.rs`):

- **How the meter is computed:**
  - Each top-level `message_start` stream event calls `called()` (`claude.rs:1927-1930`).
  - `called()` stores `call = (model, input + cache_creation + cache_read)` (`claude.rs:1621-1626`).
  - On each `result`, `total()` (`claude.rs:1628-1642`) takes the window from `modelUsage[model].contextWindow` and token totals from the cumulative `modelUsage`.
  - `meter()` (`claude.rs:1644-1649`) sends `Event::Meters(Context { used: call.1, window }, tokens)`.
  - The fields are declared at `claude.rs:944-946`.
- **How compaction is handled:**
  - `/compact` is sent as a prompt (`claude.rs:566`).
  - A live `compact_boundary` only pushes the "Conversation compacted" notice (`claude.rs:1437-1439`, `Item::compacted()` at `chats.rs:174-175`), and `compact_metadata` is ignored.
  - The `/compact` `result` then runs `total()` (`claude.rs:1490`), which re-sends the *old* `call`, so `context.used` stays at the pre-compaction value.
  - It changes only at the next main-loop `message_start`, when the next response starts streaming (a little before the turn finishes).
  - Replay from the session file marks `compact_boundary` and defers the notice (`claude.rs:1279-1286`, `1422-1426`). It also reads no sizes.
- **Propagation to the web UI:** `chats.rs:1208-1219` merges `context.or(chat.context)`, so a missing context keeps the old figure, and `web/src/lib/chat/ContextBar.svelte:6-10` renders `used / window`.

**Codex driver** (`crates/stampede/src/chats/codex.rs`):

- `/compact` sends `thread/compact/start` (`codex.rs:421-427`) and handles its empty response (`codex.rs:1176-1182`).
- The activity label "Compacting conversation…" comes from `item/started` (`codex.rs:1235`), and the notice from `item/completed` (`codex.rs:1479-1481`).
- `thread/tokenUsage/updated` becomes `Context { used: last.totalTokens, window: modelContextWindow }` (`codex.rs:1244-1251`). Nothing filters it during a compact turn, so the post-compaction estimate reaches the meter as soon as it arrives. I verified this by reading the code against the fixture order, not by running the UI.
- No test asserts meter values around compaction. The compaction fixtures are replayed only for transcript items (`crates/stampede/tests/chat_transcript.rs`).

## What a client can do right after a compact

**Claude Code** (the data exists, but the full figure must be pulled):

1. **Best: pull after the boundary.** On `compact_boundary`, or on the `result` whose `local_command == "compact"`, send:
   ```json
   {"type":"control_request","request_id":"<id>","request":{"subtype":"get_context_usage","detail":"summary"}}
   ```
   Then use `response.response.totalTokens` as `used`.
   - Keep the window from `modelUsage[model].contextWindow`. `maxTokens` can be the smaller auto-compaction window, not the model's window (sdk.d.ts:3894 for the related `SDKContextUsage.raw_max_tokens`; docs quoted above).
   - `summary` makes no API calls. Live, it was within 3% of the next real call. `full` was equally close but makes token-count requests.
   - *Not verified:* whether the control request is answered promptly during a running turn, which is the auto-compaction mid-turn case. I only tested the idle, post-`/compact` case.
2. **Push-only fallback: `post_tokens` plus a fixed overhead.**
   - The overhead is system prompt, tools and MCP. One way to get it: one `get_context_usage` per session, summing the `kind: "used"` categories other than `messages`.
   - A cruder way: the session's first `message_start` input minus the first prompt.
   - Fixture check: about 19.1k overhead plus 2,521 gives roughly 21.6k, against 21,595 actual.
   - This is an inference, and it drifts whenever tools or MCP change.
3. **Avoid:** using `post_tokens` alone as `used`, which was 8.6 times low in the fixture. Also avoid `pre_tokens - post_tokens` as "tokens freed", because the two are on different bases.
4. **In every case:** treat the figure as an estimate until the next `message_start` replaces it with real usage. The official status line's `current_usage` is simply `null` in this window.

**Codex** (the data is already pushed):

1. Use the last `thread/tokenUsage/updated` before `item/completed` for `contextCompaction`, and read `last.totalTokens` against `modelContextWindow`. Stampede already does this.
2. Optionally mark it as an estimate: `last.inputTokens == 0 && last.outputTokens == 0`. Expect it to under-read by the size of the tool schemas (about 33% in the fixture) until the next turn's real usage arrives.

## Verified vs inferred, in one place

- **Verified from types, docs, source, fixtures or the live run:**
  - Claude's `compact_boundary` carries `pre_tokens` and an optional `post_tokens` the moment compaction finishes.
  - `post_tokens` is far below the next real input: 2,521 vs 21,595, and 797 vs 4,587.
  - The `/compact` result's `usage` is zero.
  - `get_context_usage` right after a compact returns an estimated total close to the next real call.
  - The status line's `current_usage` is null after `/compact`.
  - Codex emits `thread/tokenUsage/updated` with an estimated `last.totalTokens` before the `contextCompaction` item completes, on every compaction path, and has done so since at least 0.60.1.
  - The Codex TUI applies every such update (source).
  - Stampede's Claude meter ignores compaction data; its Codex meter applies the estimate.
- **Inferred:**
  - What `post_tokens` includes (from minified code).
  - That auto-compaction boundaries also carry it.
  - That Claude Code internally treats `post_tokens` as its context-token stand-in.
  - Why the Codex estimate under-reads.
  - The estimate-detection heuristic.
  - The overhead-plus-`post_tokens` approximation.
  - How the Codex TUI's change looks in practice.
