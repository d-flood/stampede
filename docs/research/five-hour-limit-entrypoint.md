# Five-hour limits: the 2026 increases, and Agent SDK sessions draining faster

Researched 2026-11-02 against the installed Claude Code 2.1.316 (`~/.local/bin/claude` -> `~/.local/share/claude/versions/2.1.316`, built 2026-10-30, git `cd7ceb4`). I also checked one string in the 2.1.308, 2.1.309 and 2.1.312 binaries. Anthropic web pages were fetched on 2026-11-02. GitHub state is as of 2026-11-02: anthropics/claude-code#97074 is open with 62 comments.

## Short answers

**1. "Very recently doubled": not as stated. The only doubling was on 2026-05-06, and nothing since has doubled the limit again.**

On 2026-05-06 Anthropic doubled "Claude Code's five-hour rate limits for Pro, Max, Team, and seat-based Enterprise plans". There have been two five-hour changes since, and neither is a doubling:
- **2026-09-22 (Opus 5.5 launch):** an increase of unstated size for Pro, Max, Team and seat-based Enterprise.
- **2026-10-30:** verified Ultra accounts (the renamed Max 20x) no longer have a five-hour limit at all.

The recent news is probably the SDK drain in question 2. Theo blamed it on "the recent boost" not reaching Agent SDK traffic.

Who the May doubling covered:
- **Pro, Max, Team, seat-based Enterprise:** yes.
- **Usage-based Enterprise:** not applicable. Those seats have "no per-seat usage limits", and usage is billed at API rates.
- **API:** not applicable. The same post raised Opus API rate limits, which is a separate change.

Scope and duration:
- **Scope:** Claude Code's five-hour window only. Weekly limits and claude.ai were not part of it.
- **Duration:** "effective today", with no end date. Anthropic never called it temporary or permanent, and I found nothing rolling it back.

**Your account:** the local Claude config shows an Enterprise Premium seat (`seatTier: enterprise_bendep_premium`, `userRateLimitTier: default_claude_max_5x`). The help center classes Premium seats as seat-based, so the doubling should apply to you. This is an inference from config fields, not an account lookup.

**2. SDK-labelled sessions do use the five-hour window faster. At least ten independent measurements show it, but Anthropic has not acknowledged it.**
- **The effect:** for the same tokens, sessions labelled `sdk-ts`, `sdk-cli` or `sdk-py` use about 1.6–2.9× as much of the five-hour window as sessions labelled `cli`, `claude-vscode` or `claude-desktop`.
- **What it follows:** the label, not whether the session is headless. The VS Code extension also runs headless stream-json and is not affected.
- **Weekly limits:** they appear unaffected.
- **Plans tested:** Pro, Max 5x, Max 20x and Team. Nobody has reported a test on Enterprise.

What Anthropic has said:
- No Anthropic staff member has replied in the issue tracker, and the main issue has no assignee.
- The one relevant staff post (Boris Cherny, 2026-10-28) does not mention entrypoints.

The workaround is to set `CLAUDE_CODE_ENTRYPOINT` to your own tool's name. Controlled A/B tests by several people show this brings usage back to the interactive rate.
- **`cli` doesn't work:** in `-p`/stream-json mode, Claude Code rewrites `cli` to `sdk-cli`.
- **Any other string works:** it is sent through verbatim.
- **Safe names:** the 2.1.316 binary validates host-chosen request names (only for its `claude-desktop-3p` path) with `^(?!sdk)[a-z][a-z0-9_-]{0,31}$`. It also rejects known entrypoint names, names starting with `remote`, and `local_projects`. `stampede` passes all of these rules.

Downsides of the workaround:
- **It is undocumented.** The variable is not in the env-var reference, and Anthropic hasn't said whether the weighting is intended.
- **Policy direction.** Anthropic announced on 2026-05-13, then paused on 2026-06-15, a plan to move Agent SDK and `claude -p` usage off subscription limits. It does treat "programmatic" use as a separate category.
- **Other clients' names.** No document forbids setting the entrypoint, but using another client's name (`claude-vscode`, `claude-desktop`) misreports the caller and switches on that client's code paths.
- **Telemetry.** It reports the custom name and a `cli` client type.

T3 Code (run by Theo) switched to `t3-code` on 2026-11-02. opencode's maintainers declined to change theirs.

**3. Stampede: moving from `sdk-ts` to a name like `stampede` carries little functional risk.**
- **Unchanged:** the `session_state_changed` events, which Stampede gets via `CLAUDE_CODE_SDK_READS_SESSION_STATE` and which do not depend on the label. The Artifact tool also stays as it is, because Stampede already sets `CLAUDE_CODE_ARTIFACT=1`.
- **Changed:**
  - AskUserQuestion option previews turn on in markdown.
  - The `claude-code-guide` built-in subagent appears.
  - Stampede sessions show up in the terminal's `claude --resume` picker.
  - Background memory consolidation ("autoDream") becomes eligible to run, though it is still opt-in or server-flagged.

The details are in the last section.

## Method

- **Official sources:**
  - anthropic.com posts.
  - support.claude.com articles, fetched raw with their `lastUpdatedDate`.
  - code.claude.com docs (`.md` pages and `llms-full.txt`).
  - The Claude Code `CHANGELOG.md` on `main`.
- **X posts:** fetched through `api.fxtwitter.com` to get the verbatim text, UTC timestamp and author bio.
- **GitHub:** read with `gh api`, including each commenter's `author_association`. Anthropic staff would show as `MEMBER` or `COLLABORATOR`.
- **Installed binary:** strings from the 2.1.316 binary, read only. Byte offsets are given below, and anything about server-side behaviour is labelled as inference. I made no model calls.
- **Your plan:** read from non-secret fields of `~/.claude/.credentials.json` and `~/.claude.json`.

## Evidence: five-hour limit changes

### Confirmed by official sources

1. **2026-05-06, "Higher usage limits and a SpaceX compute deal"** (https://www.anthropic.com/news/higher-limits-spacex). It says the changes are "all effective today" and lists:
   - "First, we're doubling Claude Code's five-hour rate limits for Pro, Max, Team, and seat-based Enterprise plans."
   - "Second, we're removing the peak hours limit reduction on Claude Code for Pro and Max accounts."
   - "Third, we're raising our API rate limits considerably for Claude Opus models."

   The post says nothing about weekly limits, claude.ai, an end date or usage-based Enterprise.
2. **Same day on X:**
   - @ClaudeDevs ("Official updates for developers building with @ClaudeAI") said: "Doubling Claude Code's 5-hour limits for Pro, Max, Team and seat-based Enterprise plans" (https://x.com/ClaudeDevs/status/2052064938840228237, 2026-05-06 16:36 UTC).
   - @claudeai left Enterprise out: "Doubling Claude Code's 5-hour rate limits for Pro, Max, and Team plans" (https://x.com/claudeai/status/2052060693269008586, 16:19 UTC).
3. **2026-09-22, Opus 5.5 launch:**
   - The launch page says "In addition to the price drop, we're increasing five-hour usage limits on Pro, Max, Team, and seat-based Enterprise plans," and adds a saveable rate-limit reset (https://www.anthropic.com/claude-opus-5-5). It gives no size and doesn't say "Claude Code".
   - @claudeai again left Enterprise out: "we're increasing five-hour usage limits on Pro, Max, and Team plans" (https://x.com/claudeai/status/2102435538120691886, 2026-09-22 16:31 UTC).
4. **2026-10-30, @claudeai** (https://x.com/claudeai/status/2116220869479428500, 17:29 UTC):
   - "Max 5x → Max / Max 20x → Ultra".
   - "We're also removing the 5-hour limit for verified Ultra accounts, starting today with most existing subscribers."
   - "Weekly limits remain unchanged."
5. **Help center, "Use Claude Code with your Pro, Max, or Ultra plan"** (updated 2026-10-30, https://support.claude.com/en/articles/11145838-using-claude-with-your-max-plan):
   - "Pro and Max plans have a five-hour session limit and a weekly limit. Ultra plans have a weekly limit, and verified Ultra accounts don't have a five-hour session limit."
   - "These limits are shared across Claude and Claude Code."
6. **Help center, Team/Enterprise** (updated 2026-10-30, https://support.claude.com/en/articles/11845131-using-claude-code-with-your-team-or-enterprise-plan):
   - "If your organization is on a usage-based Enterprise plan (including self-serve Enterprise), there are no per-seat usage limits—usage is based on consumption and billed at API rates."
   - "On older Enterprise plans, Claude Code is available on Chat + Claude Code seats (usage-based billing) and Premium seats (seat-based billing)."
7. **Weekly limits are a separate track** (help center, "Claude Code May–August 2026 weekly limits promotion", updated 2026-10-30, https://support.claude.com/en/articles/15910845-claude-code-mayaugust-2026-weekly-limits-promotion):
   - A +50% weekly promotion ran from 2026-05-13 to 2026-09-13 and was "Claude Code only, everywhere you use it: the CLI, IDE extensions, desktop, and the web".
   - "5-hour usage limits were not affected by this promotion."
   - From 2026-09-14, weekly limits are "25% higher than they were before the promotion for Pro, Max, Ultra, Team, and seat-based Enterprise plans".
   - Consumption-based Enterprise seats were excluded.

### Stated by an Anthropic employee

- **Amol Avasare** (X bio: "Head of Growth @AnthropicAI"), 2026-05-06: "From today, we're doubling Claude Code's 5-hour rate limits on Pro, Max, Team, seat-based Enterprise." (https://x.com/TheAmolAvasare/status/2052061308913414628)

### Secondary sources only, unverified

- **A "20%" size for the 2026-09-22 increase.** It appears only in third-party write-ups. I found no Anthropic source for it.
- **The May doubling described as "permanent".** Only third parties say this.

### Not found

- Any five-hour doubling announced after 2026-05-06.
- Any Anthropic statement on whether the May doubling or the September increase covers Agent SDK or `claude -p` traffic.

## Evidence: SDK entrypoints and the five-hour window

### Community reports (no Anthropic confirmation)

The main report is anthropics/claude-code#97074, "Headless `claude -p` (entrypoint `sdk-cli`) uses ~1.8× more of the 5-hour window per token than interactive `cli`" (https://github.com/anthropics/claude-code/issues/97074). It was opened 2026-09-25 and is still open. It has automatic labels only and no assignee, and all 62 commenters are `NONE` (not Anthropic staff).

Controlled A/B runs reported there and in related trackers:

| Who, date | Plan, version | Comparison | Five-hour use per token |
|---|---|---|---|
| qvarker, 09-25 ([body](https://github.com/anthropics/claude-code/issues/97074), [proxy rerun](https://github.com/anthropics/claude-code/issues/97074#issuecomment-5836037406)) | Max, 2.1.282 | `sdk-cli` vs `cli` | ≈1.8–2.3× |
| Z3l20, 09-27 ([link](https://github.com/anthropics/claude-code/issues/97074#issuecomment-5854738845)) | Pro, 2.1.283 | `sdk-cli` vs `cli` | ≈2.0× |
| kler, 10-17 ([link](https://github.com/anthropics/claude-code/issues/97074#issuecomment-6232567215)) | Team, 2.1.300 | `sdk-ts` and `sdk-cli` vs `claude-vscode` | 2.97× and 2.83× |
| elmariodev, 10-21 ([link](https://github.com/anthropics/claude-code/issues/97074#issuecomment-6328924327)) | Max 5x, 2.1.280–2.1.307 | `sdk-ts` and `sdk-cli` vs `cli` and `claude-desktop` | ≈2.4× |
| Theo (t3dotgg), 10-25 ([link](https://github.com/anthropics/claude-code/issues/97074#issuecomment-6392296206)) | Max | `sdk-ts` and `claude -p` vs TUI | 1.6–1.7×; "Weekly is not affected" |
| frostfire575, 10-26 ([link](https://github.com/anthropics/claude-code/issues/97074#issuecomment-6426128541)) | 2.1.310 | `sdk-cli` vs custom label vs `cli` | 1.7× vs `cli`; custom ≈ `cli` |
| qvarker, 10-26 ([link](https://github.com/anthropics/claude-code/issues/97074#issuecomment-6426935193)) | Max, 2.1.304 | custom vs `sdk-cli` vs custom | custom 1.9× cheaper |
| oviano, 10-26 ([link](https://github.com/anthropics/claude-code/issues/97074#issuecomment-6427026376)) | macOS, 2.1.310 | custom vs `sdk-cli` | 2.4× |
| erikmackinnon, 10-27 ([link](https://github.com/anthropics/claude-code/issues/97074#issuecomment-6443179221)) | Team, `default_claude_max_5x` tier, 2.1.310 | `sdk-cli` vs TUI | 1.86× (90% interval 1.51–2.31) |
| 0oAstro, 10-29 ([link](https://github.com/pingdotgg/t3code/issues/24649#issuecomment-6508208895)) | Team standard seat, 2.1.311 | `sdk-ts` vs `cli`; `t3-code` label | ≈2.3×; `t3-code` ≈ `cli` |
| harry924, 10-30 ([link](https://github.com/anthropics/claude-code/issues/97074#issuecomment-6512995747)) | Max 20x, 2.1.312 | `sdk-cli` vs `cli` and real Desktop (`claude-desktop`) | ≈2.5× |
| Peteralm, 10-26 ([t3code#25597](https://github.com/pingdotgg/t3code/pull/25597)) | Max, 2.1.310 | `sdk-ts` vs `t3-code` vs arbitrary `local-probe-97074` | non-`sdk` names ≈1.8× cheaper |

Points that recur across the reports:
- **The label causes it, not headless mode.** `claude-vscode` is also headless stream-json and is billed like `cli` (kler, harry924).
- **Weekly limits look unaffected,** though every meter involved moves only in 1% steps (Theo, erikmackinnon, harry924).
- **Inherited labels.** A process started from inside another Claude session inherits its `CLAUDE_CODE_ENTRYPOINT` (Z3l20, albertofa, and [claude-agent-sdk-typescript#520](https://github.com/anthropics/claude-agent-sdk-typescript/issues/520)).
- **A counterpoint.** One tester saw no weekly-point difference on a cache-read-heavy real workload ([jv2222](https://github.com/anthropics/claude-code/issues/97074#issuecomment-6436089337)).

Theo said it plainly on X: "I confirmed it's a real bug on Anthropic's side … this does not affect weekly limits … I have reported this to Anthropic" (https://x.com/theo/status/2114166439813017873, 2026-10-25). His guess was "I suspect it's from the recent boost to 5h limits. Seems like they forgot to apply it to Agents SDK?" (https://x.com/theo/status/2114166577608478826). He is not an Anthropic employee.

### Anthropic statements

- **None on the weighting itself.**
- **Boris Cherny** (X bio: "Claude @anthropicai"), 2026-10-28, on reports of fast usage in general: "We didn't find a single root cause or any regression in Claude Code. In most cases, users who saw faster usage against limits were traced back to one or two heavy sessions (xhigh effort, /goal, Workflows, Agent Teams, long SDK jobs)" (https://x.com/bcherny/status/2115592371224346734). It does not mention entrypoints or `sdk-*`.
- **Help center, "Use the Claude Agent SDK with your Claude plan"** (updated 2026-10-30, https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan):
  - The 2026-10-07 update says that, signed in with a plan, "the Claude Agent SDK and claude -p still draw from your subscription limits".
  - The 2026-06-15 update says "We've paused the previously-announced changes to Claude Agent SDK usage."
  - It says nothing about weighting.
- **@ClaudeDevs, 2026-05-13** (the announcement later paused): "Starting June 15, paid Claude plans can claim a dedicated monthly credit for programmatic usage", covering the Agent SDK, `claude -p`, GitHub Actions and "Third-party apps built on the Agent SDK" (https://x.com/ClaudeDevs/status/2054610152817619388). The thread, read via a mirror (https://threadreaderapp.com/thread/2054610152817619388), adds "Your subscription limits don't change, they're now reserved for interactive use." That is a statement of intent to treat programmatic use separately. It is not evidence about the current weighting.
- **Closed without an answer:** anthropics/claude-code#81692, "Authentication docs don't state which usage pool `claude -p` draws from", was closed as `not_planned` (stale) on 2026-09-15.

### What the 2.1.316 binary does with the entrypoint (verified by reading the code)

1. **How the label is chosen** (offset 273916437):
   - If `CLAUDE_CODE_ENTRYPOINT` is set, it is kept as is, except that `local_agent` becomes `local-agent` and `cli` in non-interactive mode becomes `sdk-cli`.
   - If it is unset, the label is `mcp` (for `mcp serve`), `claude-code-github-action`, or otherwise `sdk-cli` for `-p` and `cli` for the TUI.
   - The Agent SDK sets `sdk-ts` only when the variable is empty: `if(!I.CLAUDE_CODE_ENTRYPOINT)I.CLAUDE_CODE_ENTRYPOINT="sdk-ts"` (301010931).
   - Nothing validates the value. Known names are listed in `$do` (273907027): `cli, mcp, sdk-cli, sdk-ts, sdk-py, bench, claude-vscode, claude-code-github-action, local-agent, claude-desktop, remote*, claude-in-slack, claude-in-teams, claude-desktop-3p, claude-security, ssh-remote, claude-coworker*`.
2. **Where the label goes on every model request:**
   - **Billing header:** `x-anthropic-billing-header: cc_version=…; cc_entrypoint=${h};`, with `h=b7()??process.env.CLAUDE_CODE_ENTRYPOINT??"unknown"` (277958693, 277960134).
   - **User-Agent:** `claude-cli/<ver> (external, <entrypoint>[, agent-sdk/…][, client-app/…])` (277101626).
   - **Client-platform header:** `anthropic-client-platform` comes from `o_()`, which maps `sdk-cli`, `sdk-ts` and `sdk-py` to `claude_code_sdk`, `claude-vscode` to `claude_code_vscode`, and any unknown value to `claude_code_cli` (273266765). It is used on claude.ai API calls.

   The docs confirm the billing block is read server-side: "Anthropic and the cloud providers' Claude endpoints read the block for attribution" (https://code.claude.com/docs/en/llm-gateway-protocol#system-prompt-attribution-block).
3. **New in 2.1.316: Anthropic's own Remote Control sends attended `sdk-cli` children as `cli`.**
   - The bridge sets `CLAUDE_CODE_BRIDGE_CHILD_CLI_ENTRYPOINT=1` when `cliEntrypoint:e.roles.mayBeAttended&&d4t()` (300649878, 300662223). `d4t()` is the feature flag `tengu_gentle_orchard`, which defaults to true.
   - The child then runs `hbi()`, which, when `CLAUDE_CODE_ENTRYPOINT` is `sdk-cli`, makes requests go out under the name `"cli"` (277960357, telemetry `bridge_cli_entrypoint_name`).
   - These strings are absent from 2.1.308, 2.1.309 and 2.1.312, and no CHANGELOG entry mentions them.
4. **A supported host-naming path exists, but only for Desktop 3P.** `CLAUDE_CODE_ENTRYPOINT_ON_REQUESTS` (277960481) is read only when `CLAUDE_CODE_ENTRYPOINT` is `claude-desktop-3p`. The name it accepts must:
   - be "1 to 32 characters of a-z, 0-9, _ and -, start with a letter and not start with sdk";
   - not be an existing entrypoint name, start with `remote`, or be `local_projects`.
5. **The documented way to identify an SDK app** is `CLAUDE_AGENT_SDK_CLIENT_APP`: "Set `CLAUDE_AGENT_SDK_CLIENT_APP` to identify your app in the User-Agent header" (https://code.claude.com/docs/en/agent-sdk/python). It adds `client-app/<name>` to the User-Agent only and leaves `cc_entrypoint` unchanged. Nobody has tested whether it affects metering.
6. **`CLAUDE_CODE_ENTRYPOINT` is undocumented.** It is absent from https://code.claude.com/docs/en/env-vars. The docs mention entrypoint values only as the opt-in OpenTelemetry attribute `app.entrypoint` ("such as `cli`, `sdk-cli`, `sdk-ts`, `sdk-py`, `claude-vscode`", https://code.claude.com/docs/en/monitoring-usage).
7. **`CLAUDE_CODE_ATTRIBUTION_HEADER=0` is not a workaround.** It drops the whole block. The docs describe it as existing "for gateway and third-party caching compatibility", and on direct connections the block is kept on auto-mode classifier requests anyway (https://code.claude.com/docs/en/env-vars, gateway page above). Nobody has tested its effect on metering.

### Inferred

- **The server weights five-hour usage by label category.** Three things support this:
  - every A/B above;
  - `o_()` grouping the three `sdk-*` labels together;
  - Anthropic's own Remote Control now relabelling attended children from `sdk-cli` to `cli`, and its Desktop-3P validator banning `sdk*` names.

  The server-side rule itself was not observed.
- **Theo's "boost not applied to the SDK" theory fits, but is unproven.** The May post scoped the doubling to "Claude Code's" five-hour limits, while the help center says limits are "shared across Claude and Claude Code". Raising one surface's share of a shared window implies Claude Code usage is weighted per surface. If `sdk-*` traffic is not classed as Claude Code for that weight, a gap of about 2× follows, plus whatever the September increase added. The measured 1.6–2.9× is consistent with that, given the meters' 1% resolution.

### Downstream responses

- **T3 Code changed its label.** PR #28410, "update the claude entrypoint identifier for accuracy" by t3dotgg, merged 2026-11-02 03:01 UTC (https://github.com/pingdotgg/t3code/pull/28410). It sets `CLAUDE_CODE_ENTRYPOINT=t3-code` for every Claude process. It reports that 2.1.316 then sends `User-Agent: claude-cli/2.1.316 (external, t3-code)` and `cc_entrypoint=t3-code`. It also says 2.1.316 ignores the variable when it is only set in `settings.json` `env`.
- **T3 Code closed its issue without explanation.** Theo closed t3code#24649 at 07:37 UTC the same day, with only "🤐" as a comment (https://github.com/pingdotgg/t3code/issues/24649).
- **An earlier community PR for the same change was turned down.** t3code#25597 was closed unmerged on 2026-10-31 under T3's own prior-approval rule.
- **opencode declined.** A maintainer (`MEMBER`) closed opencode#61072 on 2026-11-01 with: "This isn't up to us to fix. It's not documented by anthropic and usage limits likely change frequently with many factors so it's hard to say how reliable the reproductions are." (https://github.com/anomalyco/opencode/issues/61072)

### Policy context (official)

- **Legal and compliance** (https://code.claude.com/docs/en/legal-and-compliance):
  - "Advertised usage limits for Pro, Max, and Ultra plans assume ordinary, individual usage of Claude Code and the Agent SDK."
  - Developers "should use API key authentication … Anthropic does not permit third-party developers to offer Claude.ai login into their own applications, or to route requests through Free, Pro, Max, or Ultra plan credentials on behalf of their users."
  - The same page also says: "Nor does it prevent an end user from signing in to the unmodified Claude Code binary with their own Claude subscription."
  - Team and Enterprise use falls under the Commercial Terms.
- **Agent SDK overview** (https://code.claude.com/docs/en/agent-sdk/overview): "Unless previously approved, Anthropic does not allow third party developers to offer claude.ai login or rate limits for their products, including agents built on the Claude Agent SDK."
- **The two pages may conflict.** A user has asked Anthropic to reconcile them, with no answer yet ([claude-agent-sdk-typescript#512](https://github.com/anthropics/claude-agent-sdk-typescript/issues/512)).
- **Stampede's own pattern.** Stampede runs the unmodified binary on the user's machine under the user's own login and never handles credentials. That matches the legal page's carve-out.
- **No document mentions setting `CLAUDE_CODE_ENTRYPOINT`.**

## What changing the entrypoint means for Stampede

Today `crates/stampede/src/chats/claude.rs:50` sets `SDK_ENV = [CLAUDE_CODE_ENTRYPOINT=sdk-ts, CLAUDE_CODE_SDK_READS_SESSION_STATE=1]`. Stampede applies it after the user's environment for:
- chats (`launch`, `:199-202`);
- the `options` probe (`:113`);
- `usage` (`:474`);
- `ask` (`:519`).

Because it overwrites the variable rather than defaulting it, Stampede already avoids the inherited-label trap described above. Your plan is seat-based, so only the five-hour window is at stake. Usage-based Enterprise seats have no five-hour window.

Here is what a custom value such as `stampede` changes in 2.1.316, with byte offsets in the binary:

| Behaviour | `sdk-ts` (today) | Custom label |
|---|---|---|
| Five-hour metering (community-measured) | ≈1.6–2.9× the `cli` rate per token | About the `cli` rate in every A/B that tested a custom label |
| `cc_entrypoint`, User-Agent | `sdk-ts`, `(external, sdk-ts)` | `stampede`, `(external, stampede)` |
| Telemetry client type (293716590) | `sdk-typescript` | `cli` |
| `session_state_changed` events (283077640) | Emitted (`CLAUDE_CODE_SDK_READS_SESSION_STATE`) | Same; this gate does not check the entrypoint |
| Artifact tool default (282878746) | Off (`sdk_default_off`) unless `CLAUDE_CODE_ARTIFACT=1` | On by default. No change for chats, which already set it |
| AskUserQuestion `preview` (293717379) | No preview format set, so Claude emits no previews (docs: "When unset, Claude does not emit previews", https://code.claude.com/docs/en/agent-sdk/typescript) | Defaults to `markdown` previews. Stampede already renders `preview` (`chats.rs:508`, `QuestionOption.svelte:12`). `CLAUDE_CODE_QUESTION_PREVIEW_FORMAT` pins either behaviour |
| Built-in `claude-code-guide` subagent (285539756) | Omitted | Listed. Slightly larger Agent tool description |
| Terminal `claude --resume` picker (274568579, `isHiddenFromSessionPicker`) | Stampede sessions hidden | Stampede sessions listed |
| autoDream memory consolidation (291981835) | Never runs | Eligible, but still needs `autoDreamEnabled` or a server flag. It makes background model calls when it runs |
| Feedback relay (`tengu_juniper_relay`, 291700506) | Off | Flag-gated, default off |
| `CLAUDE_CODE_QUESTION_EXTENDED` / `…_OPTIONAL_DESCRIPTIONS` | Honoured if set | Ignored. Stampede sets neither |
| Attended/background-command treatment, `KU()` | Not attended | Not attended (unchanged) |

Other points:
- **Avoid `cli`.** With `-p` it is rewritten to `sdk-cli`, which keeps the higher rate.
- **Avoid `claude-vscode` or `claude-desktop`.** Both turn on host-specific paths (for example `Em()` VS Code checks, Desktop skill and connector handling) and misreport the caller.
- **Use a lowercase name of up to 32 characters that doesn't start with `sdk`.** That matches Anthropic's own validator for host request names.
- **The variable must be in the process environment.** T3 reports that 2.1.316 ignores it in `settings.json` `env`.
- **This could stop working.** Anthropic has not documented the label or said the weighting is intended. It could fix the weighting server-side, extend it to unknown labels, or start treating relabelling as circumvention. If that happens, switching back is a one-line revert.

## Verified vs inferred, in one place

- **Verified from official sources:**
  - The May 6 doubling: date, plans (seat-based Enterprise included in the blog and @ClaudeDevs, missing from @claudeai), Claude Code five-hour scope, no stated end.
  - The Sept 22 increase, size not stated.
  - The Oct 30 Ultra change and plan renames, with weekly limits unchanged.
  - Usage-based Enterprise having no per-seat limits.
  - The weekly promotion and the +25% permanent weekly increase.
  - The SDK help-article updates of May 13, June 15 and Oct 7.
  - The legal and SDK-overview policy text.
- **Verified from the 2.1.316 binary:**
  - How the entrypoint is chosen and rewritten.
  - Its presence in `cc_entrypoint`, the User-Agent and `anthropic-client-platform`.
  - The new Remote Control `cli` relabel.
  - The Desktop-3P request-name validator.
  - Every Stampede-relevant gate in the table above.
- **Stated by Anthropic staff:** Amol Avasare on the May doubling. Boris Cherny on fast usage in general, without mentioning entrypoints.
- **Community reports only:**
  - The `sdk-*` five-hour weighting of about 1.6–2.9×.
  - Weekly limits being unaffected.
  - Custom labels restoring the interactive rate.
  - 2.1.316 ignoring `settings.json` for the variable.
- **Inferred:**
  - That the server weights by label category.
  - That the May "Claude Code" doubling is a per-surface weight that `sdk-*` traffic misses.
  - That your account is seat-based and included (from config fields).
- **Not found:**
  - Any doubling after May 6.
  - Any Anthropic acknowledgement, fix, or documented policy on the `sdk-*` weighting or on setting `CLAUDE_CODE_ENTRYPOINT`.
  - Any Enterprise-plan A/B test.
