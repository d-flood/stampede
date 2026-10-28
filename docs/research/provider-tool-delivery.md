# Delivering Stampede tools to provider harnesses

Researched 2026-10-28. Recommendation: retain MCP as Stampede's shared tool surface. Native adapters can simplify individual providers, but changing transport alone does not remove descriptions, schemas, arguments, or results from model context. Deferring or reducing tools is the relevant context optimization. This is an architectural judgment based on the interfaces below.

## Current implementation

Stampede runs three different harness interfaces:

| Provider | Stampede integration | Tool registration |
| --- | --- | --- |
| Claude Code | CLI streaming protocol | `--mcp-config`, with allowed tool names |
| Codex | app-server protocol | `mcp_servers.stampede_chat` in thread config |
| Copilot | Rust SDK 1.0.23 | `with_mcp_servers` for create and resume |

Sources: [Claude launch](../../crates/stampede/src/chats/claude.rs#L189), [Codex thread config](../../crates/stampede/src/chats/codex.rs#L541), [Copilot sessions](../../crates/stampede/src/chats/copilot.rs#L283), [dependency](../../crates/stampede/Cargo.toml#L18).

All three point to `stampede chat-tools`, a stdio MCP server. Its initialization response contains server identity and tool capability, with no server instructions or resources. `tools/list` publishes seven tools. The HTML tool description includes the current chat's canvas directory. See [server](../../crates/stampede/src/chats/html.rs#L22).

## Provider alternatives

**Claude Code:** MCP fits the existing CLI integration. The official Agent SDK's custom tools also use MCP: `tool()`/`@tool`, an in-process `createSdkMcpServer`/`create_sdk_mcp_server`, then `mcpServers`. That option requires an SDK host and changes process plumbing rather than eliminating tool context. Official SDKs documented here are TypeScript and Python. [Custom tools](https://code.claude.com/docs/en/agent-sdk/custom-tools).

Current Claude Code defers MCP tools by default on supported models; only names and server instructions load initially. `ENABLE_TOOL_SEARCH=auto` instead loads them upfront below a 10% context threshold. Model, deployment, organization, and environment settings can disable search. `alwaysLoad: false` in server config keeps all its tools deferred, including tools otherwise marked upfront. Thus seven published tools do not imply seven full schemas on every initial model request. These are current documentation semantics; actual installed version and settings must be checked. [MCP tool search](https://code.claude.com/docs/en/mcp#scale-with-mcp-tool-search).

**Codex:** app-server supports client-owned tools using `dynamicTools` on `thread/start` and `item/tool/call` requests. They persist in rollout metadata and restore on resume. This remains explicitly experimental. It could remove the separate MCP child process for Codex, but requires handling calls, results, and lifecycle in Stampede. [Official OpenAI documentation: app-server](https://learn.chatgpt.com/docs/app-server#dynamic-tool-calls-experimental).

Codex also supports per-server `enabled_tools`/`disabled_tools`, so Stampede can narrow its MCP catalog without a transport rewrite. [MCP configuration](https://learn.chatgpt.com/docs/extend/mcp#other-configuration-options).

Installed Codex 0.164.0's source already defers ordinary MCP tools when `model_info.supports_search_tool` and deferred exposure is allowed. With default exposure configuration it removes direct exposure; unsupported models instead retain direct exposure. It automatically adds search when searchable deferred tools exist. This is source verification, not a live measurement of the selected model. [Pinned tool exposure policy](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/tools/spec_plan.rs#L198-L293), [search registration](https://github.com/openai/codex/blob/6aaac694803004a63c6f393d153c96d22521a6b9/codex-rs/core/src/tools/spec_plan.rs#L392-L418).

The installed app-server schema also exposes `deferLoading` for dynamic functions, plus dynamic namespaces whose member functions can defer. Verified by `codex app-server generate-json-schema --out /tmp/stampede-provider-tool-schema`, in `v2/ThreadStartParams.json`. Native tools offer explicit controls, but MCP already has the relevant automatic behavior; a rewrite is unnecessary just to obtain deferral. Do not copy Responses API request settings directly into app-server configuration. [Responses API tool search](https://developers.openai.com/api/docs/guides/tools-tool-search).

**Copilot:** the Rust SDK supports native `Tool` plus `ToolHandler`, installed with `SessionConfig::with_tools`; handlers execute in the host. Experimental `Session::set_tools` can replace the client's tools during a live session. This is a credible native alternative without switching languages. [Official Rust SDK](https://github.com/github/copilot-sdk/blob/main/rust/README.md#tool-registration).

Tool search applies to both MCP and external tools; changing MCP to native tools therefore provides no automatic deferral advantage. The official changelog documents default tool search. Installed SDK 1.0.23's `ToolSearchConfig` documents a runtime threshold of 30 total tools, and `Tool.defer` supports automatic deferral or always loading. Stampede currently sets neither. [Official changelog](https://github.com/github/copilot-sdk/blob/main/CHANGELOG.md#feature-tool-search-configuration); installed primary source: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/github-copilot-sdk-1.0.23/src/types.rs`, `Tool` and `ToolSearchConfig`.

## Practical choice

Caching is separate from context reduction. Stable tool definitions can be part of the cached rendered prompt, reducing repeated computation and cost; cached definitions still belong to the model's context. Changing definitions or order can disrupt reusable prefixes. [Official OpenAI documentation: prompt caching](https://developers.openai.com/api/docs/guides/prompt-caching).

Keep a single MCP implementation while Stampede supports all three harnesses. First measure effective model context per provider, shorten long descriptions, and hide tools that a workflow cannot use. Prefer provider-supported deferral where reliable. Native Codex or Copilot adapters become worthwhile for a concrete lifecycle, latency, or capability requirement; they are not inherently a context reduction. A shell CLI would still need discoverability and usage instructions, and would route structured app actions through a shell tool, so it is not an obvious improvement for these seven UI and coordination tools.
