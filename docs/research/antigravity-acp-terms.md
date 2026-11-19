# Driving `agy acp` under Antigravity's terms

Researched 2026-11-19. Recommendation: **gray area.** It is acceptable for Chats where a person starts each turn, and prohibited under consumer sign-in when another Agent writes the prompts. Google's own docs say "any ACP client" may launch `agy acp`. The binding terms, however, still ban third-party software outright, and the only permission specific to ACP comes from non-binding staff and collaborator answers. The reported "Antigravity Starter Quota" tier means a Workspace account, which may fall under the administrator's terms instead.

## Bottom line

The collaborator answers draw the line at who starts each turn, not at who wrote the words.

- **Acceptable (per Google docs and collaborator answers).** A person starts each turn of a Stampede Chat. Stampede sends the prompt over `session/prompt` to the user-installed, unmodified `agy acp`, which keeps its own sign-in, history and system identity. Stampede tools arrive through ACP `mcpServers`, and permission requests are shown to the user. Per-call approval is not required: "accept-edits, auto mode, or permission rules in your own session is supported" ([#1493](https://github.com/google-antigravity/antigravity-cli/issues/1493#issuecomment-6814901159)).
- **Acceptable (inference).** Prompts Stampede composes when the user clicks: Templates, Clips, and actions on selected code. These are sent as the user prompt or as ACP resource blocks, never as a replacement system prompt.
- **Acceptable (per #1493).** Reading quota with `agy -p /quota --output-format json` from deterministic code. #1493 supports "agy -p directly from your own deterministic scripts", and #1505 points users to `/usage`.
- **Gray.** Runs and their Reviews. After one click, Stampede starts Chats for Tickets as they unblock, in parallel, and resumes after usage resets. This is deterministic, but it resembles a "background task queue" and "unattended batch processing" ([#1505](https://github.com/google-antigravity/antigravity-cli/issues/1505#issuecomment-6814941004)).
- **Prohibited on consumer sign-in.** Prompts another Agent writes, such as `delegate_to_rig` into an Antigravity Rig. Also prohibited: wrapping agy with another framework's system prompt, tool schemas or replayed history, and touching tokens.
- **Why it is still a gray area.**
  - §6 contains no ACP carve-out.
  - Google staff say they "cannot provide formal legal interpretations".
  - The guidance has shifted within weeks.
  - Enforcement is automated.

## Governing terms (verified)

Antigravity Additional Terms. They name three governing documents: Google ToS, these Additional Terms and the Privacy Policy. Clause 6 says: "You must not abuse, harm, interfere with, or disrupt the Service. This includes, but is not limited to, using the Service in connection with products not provided by us. Using third party software, tools, or services to access the Service (e.g. using OpenClaw with Antigravity OAuth) is a breach of this Agreement. Such actions may be grounds for suspension or termination of your Antigravity and/or Gemini CLI accounts." The terms say nothing about ACP, reverse engineering, quota circumvention or resale. [Terms](https://antigravity.google/terms).

History of clause 6, taken from the archived page bundle:
- **2025-12-10:** absent. The governing list then also included the Generative AI Additional Terms. [Wayback](https://web.archive.org/web/20251210063438/https://antigravity.google/terms)
- **By 2026-02-28:** present, ending "violation of applicable terms and policies … suspension or termination of your account". [Wayback](https://web.archive.org/web/20260228143750/https://antigravity.google/terms)

Google Terms of Service, effective 2026-07-30. Relevant items under "Don't abuse our services":
- "bypassing our systems or protective measures"
- "reverse engineering our services or underlying technology … to extract trade secrets"
- "providing services that appear to originate from you (or someone else) when they actually originate from us"

The software license adds: "You may not copy, modify, distribute, sell, or lease any part of our services or software." Google may suspend or terminate access if "you materially or repeatedly breach these terms, service-specific additional terms, or policies". [Google ToS](https://policies.google.com/terms).

Other policies:
- **Generative AI Prohibited Use Policy:** content rules, plus "Circumvention of abuse protections or safety filters". Nothing about clients. [Policy](https://policies.google.com/terms/generative-ai/use-policy)
- **Generative AI Additional Terms:** "no longer apply" since 2024-05-22, except for business partners. [Terms](https://policies.google.com/terms/generative-ai)
- **Gemini API Additional Terms:** govern Gemini API and AI Studio, so they cover agy's `gemini-api-key` method and not `oauth-personal`. They add "not for consumer use" and "may not attempt to reverse engineer, extract or replicate any component". [Gemini API terms](https://ai.google.dev/gemini-api/terms)

## Workspace caveat (verified)

"Antigravity Starter Quota" is the tier shown when a Workspace account signs in with **Continue with Google**. Administrators can turn it off. [Workspace accounts](https://antigravity.google/docs/workspace#what-happens-if-i-sign-in-with-my-workspace-account-using-continue-with-google).

The terms' preamble says users who come through Gemini Enterprise, "a Google Workspace subscription on the Google Cloud Pre-GA Offering Terms", or a business API key "are subject to the terms of use accepted or signed by your administrator". [Terms](https://antigravity.google/terms). No primary source says which branch covers starter quota. Inference: the organization's terms and policies may apply on top of clause 6.

## What Google documents about ACP (verified)

The docs say:
- "Any ACP client can launch Antigravity CLI. Configure it to run this command … `agy acp`."
- `oauth-personal` is a documented ACP sign-in method.
- VS Code is set up through a community extension, "maintained by the community, not by Google". So Google itself documents a third-party client.

[Other editors (ACP)](https://antigravity.google/docs/ide/extensions/acp), [CLI ACP](https://antigravity.google/docs/cli/acp).

The local `agy changelog` (1.5.12) records:
- **1.5.0:** added `agy acp` "so editors can use it as their agent".
- **1.5.1:** changed it "to offer every model available to your account to every ACP client, where previously only a fixed list of editors were offered" non-Gemini models.

The docs' models section still restricts third-party models to named editors, so the docs page is stale relative to 1.5.1.

The ACP Registry entry `antigravity-acp` lists its authors as "Google LLC", its license as "proprietary", and its `license_url` as the Antigravity terms. [agent.json](https://github.com/agentclientprotocol/registry/blob/main/antigravity-acp/agent.json).

The Python SDK documents only API-key and Vertex/ADC auth. [SDK README](https://github.com/google-antigravity/antigravity-sdk-python).

The FAQ names Claude Code, OpenClaw and OpenCode as third-party software that violates the terms, and recommends an API key for third-party agents. It also says "Wrapping `agy` inside third-party coding agents or applications, or automating account rotation, is not supported under consumer plans." That second entry appeared between the [2026-10-04](https://web.archive.org/web/20261004204211/https://antigravity.google/docs/faq.md) and [2026-11-14](https://web.archive.org/web/20261114193116/https://antigravity.google/docs/faq.md) snapshots. The FAQ never mentions ACP. [FAQ](https://antigravity.google/docs/faq#can-i-use-antigravity-cli-headless-mode-in-third-party-coding-agents-or-automate-account-switching).

## Official clarifications distinguishing ACP

**GitHub, `google-antigravity/antigravity-cli`.** The author, `manirajc`, has the `COLLABORATOR` association on Google's repo. Their Google employment was not independently verified.

- [#1493](https://github.com/google-antigravity/antigravity-cli/issues/1493#issuecomment-6814901159), 2026-11-12:
  - "Any interactive ACP v1 client, including … custom or non-editor interactive ACP UIs, may launch the unmodified agy acp binary when a human user is directly driving the session."
  - The following is not permitted on `oauth-personal`: using it "as a backend worker inside a third-party coding agent, autonomous agent harness, or background orchestrator", or "another agent, autonomous loop, or background task queue generate prompts".
  - There is "no carve-out for single-account or personal use".
- [#1505](https://github.com/google-antigravity/antigravity-cli/issues/1505#issuecomment-6814941004), 2026-11-12, about a non-editor app (Obsidian Copilot). It is permitted when:
  - The setup is "single-user, local, interactive".
  - Sign-in stays inside agy.
  - Prompts are surfaced to the user.
  - No background or scheduled jobs and no "autonomous multi-agent loops" run on `oauth-personal`.
  - The client sends accurate `clientInfo`.
  - The client does not "replace or override Antigravity's core system identity". Appending via `_meta.antigravity.systemPrompt.append` is allowed.
  - Nothing implies Google endorsement.
  - Registry download from `dl.google.com` is allowed if the binary is not re-hosted or modified.
- [#1542](https://github.com/google-antigravity/antigravity-cli/issues/1542#issuecomment-6941368658), 2026-11-17: a framework that wraps each typed message with its own system prompt, tool definitions and history is prohibited, "even when you personally initiate every turn". The client should stay a "thin ACP UI client": send only the raw user prompt, let agy own the session, and register tools via `mcpServers`.

**Google AI forum.** `chunduriv` is marked staff, with `google` flair.

- 2026-09-30 and 2026-10-07: §6 targets "extracting, intercepting, or reusing Antigravity OAuth tokens … (for example, configuring OpenClaw, OpenCode, or Claude Code to call Antigravity backend APIs directly)". Having a local assistant spawn `agy -p` was called permitted. [185992](https://discuss.ai.google.dev/t/185992/2), [186246](https://discuss.ai.google.dev/t/186246/2)
- 2026-10-08: "we cannot provide formal legal interpretations"; "we do not support using third-party coding agents … to orchestrate or wrap agy under a consumer login". [187241](https://discuss.ai.google.dev/t/187241/2)

So staff reversed on agent-spawned `agy -p` within a week. Both answers recommend ACP-compatible editors as a supported option.

Not relied on: replies from `Rebbanpalli_Naveen`, who is not marked staff, saying a personal `agy acp` app is "permitted". [198889](https://discuss.ai.google.dev/t/198889/2).

## Enforcement record

- **February 2026 (verified).** Varun Mohan posted on 2026-02-23: "a massive increase in malicious usage of the Anitgravity backend … We needed to find a path to quickly shut off access to these users that are not using the product as intended". [X post](https://x.com/_mohansolo/status/2025766889205739899). Mohan was named as Google's lead by press; that role was not checked against a primary source.
  - The targeted conduct is defined only by the terms' example, OpenClaw with Antigravity OAuth, and by the staff description above.
  - Staff on 2026-03-06: "rolling out a system-wide automated unban for all affected accounts".
  - Staff on 2026-03-09: a background quota-monitor browser extension was "accurately flagged". [128747](https://discuss.ai.google.dev/t/128747/5)
- **The `agy` binary.** It shows Terms-of-Service blocks with an appeal link (`agy changelog` 1.2.14, 1.3.1).
- **No primary source** ties any suspension to `agy acp`. Users' appeals citing Zed or T3 Code ACP use (for example [198956](https://discuss.ai.google.dev/t/198956)) give no confirmed cause.

## Stampede implications (inference)

- Never make an Antigravity Rig on `oauth-personal` a `delegate_to_rig` target. Treat Runs and Reviews on it as gray until Google answers. `gemini-api-key` and `agent-platform` have no such limits. See [GLOSSARY](../../GLOSSARY.md) for these terms.
- Agy's own Subagents and swarms inside a supervised Chat were asked about in #1505 but not answered.
- Fork, resume and context must use ACP session load and resume. Never replay transcripts or override the system prompt. Stampede already prepends no system prompt ([audit](stampede-agent-context.md)).
- Require a user-installed agy and never bundle it. Read no files under `~/.gemini`. Read quota only through `agy -p /quota`, never by calling Google's quota API. Send honest `clientInfo`. Label the Agent as Google Antigravity.

## Verified vs inferred

- **Verified:** all quoted terms, docs, FAQ, changelog and registry text; the clause 6 and FAQ timing via Wayback; the GitHub association labels and forum staff flags; the first sentences of the X post.
- **Inferred:**
  - that collaborator answers reflect Google's position;
  - that Stampede Chats fall on the permitted side, including Templates and actions the user clicks;
  - that reading quota via `agy -p /quota` counts as deterministic scripting;
  - the Workspace terms branch;
  - that clause 6 is enforced as the clarifications describe.

## Independent verification, 2026-11-19

Rechecked the current terms, FAQ, ACP guide, and collaborator replies in #1493, #1505 and #1542; their central distinctions above still match those sources. One authentication detail matters for automated features: setting `GEMINI_API_KEY` alone does not override cached `oauth-personal` credentials. Send ACP `authenticate` with `methodId: "gemini-api-key"` before `session/new`; likewise explicitly select `agent-platform` for Google Cloud. Otherwise an intended API-backed Run could silently use consumer OAuth. [#1542 clarification](https://github.com/google-antigravity/antigravity-cli/issues/1542#issuecomment-6941368658).
