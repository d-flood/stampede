---
name: promo-video
description: Make or extend Stampede's promo video from real app footage. Stages a scene with the fake agents, captures it through ./check e2e, and cuts it in Remotion with code-generated music.
disable-model-invocation: true
---

# Promo video

Footage is the real app driven by Playwright and the fake agents; Remotion only frames, zooms, captions and scores it. Read `promo/README.md` for the commands.

| Path | Role |
|---|---|
| `promo/seed/tidepool` | Demo project (SvelteKit notes app). `web/promo/stage.ts` copies it and commits it in four steps. |
| `web/promo/chat.ts` | `Chat`: writes a fake Claude Code fixture. |
| `web/promo/codex.ts` | `Codex`: writes a fake Codex fixture from the recorded `approval` handshake. |
| `web/promo/record.ts` | `Director`: CDP screencast per Device plus `timeline.json` of marks, moves, clicks, taps, focus boxes and keys. |
| `web/promo/stage.ts` | `project()`, and `configure()` for config.toml: your Rigs (`agent`, `model`, `effort`, `mode`, `shortcut`, `gates`), plus "Claude Haiku" (commit drafts) and "Codex". |
| `web/promo/*.spec.ts` | One capture per scene; output in `promo/public/footage/<scene>/`. |
| `promo/src/edit.ts` | Tempo (100 BPM, bar = 2.4 s), `sections` with bars and music styles, and `cuts`: `Promo` (~90 s, for the README) and `Tour` (every feature, ~4 min). |
| `promo/src/timing.ts` | Retime anchors for the Promo scenes, the `scenes` list, and every scene's music cues. |
| `promo/src/Promo.tsx` | Promo scene components, `Title`, and `Edit`, which lays a cut's parts on the timeline. |
| `promo/src/Tour.tsx` | The Tour cut and its chapter rail. |
| `promo/src/tour/` | Tour-only scenes: `<scene>.ts` (keys and cues) and `<Name>.tsx` (component). |
| `promo/music/render.ts` | Dependency-free synth; `render.ts <Cut>` follows that cut's sections and cues. Peaks carry the supersaw hook. |

## Adding a feature

1. **Learn it.** Read GLOSSARY.md, the feature's `web/e2e/*.spec.ts` for selectors, and any fixture shapes in `crates/stampede/tests/fixtures/chats/`. For large features, research them with a subagent first.
2. **Storyboard.** Write 2–4 beats with one short caption each (at least ~2.5 s on screen). Pick a section length in whole bars.
3. **Capture spec** `web/promo/<scene>.spec.ts`:
   - `project(root, 'tidepool')`, then write fixtures with `Chat` or `Codex`, then `configure(page, root, rigs, { codex })` **before** `page.goto`.
   - Do all setup off camera: open Projects, tabs, splits, the Drawer shell. Then `new Director('<scene>')`, `record()` each Device, and `hold(0.8)`.
   - On camera, use only Director methods for anything the viewer should see: `click`, `tap`, `type`, `rest`, `keys`, and `focus(device, name, locator)` to record a box the camera can zoom to. `mark()` every anchor you will retime around. End with `mark('end')` and `finish()`.
   - Start agents remotely when the viewer shouldn't see a send: open a WebSocket in `page.evaluate`, then `hello`, `chat.create`, `chat.send` (see `fleet.spec.ts`, `split.spec.ts`).
4. **Capture** with `./check e2e --config promo/playwright.config.ts <scene>.spec.ts`. Failure artifacts are in `web/test-results/promo/`. Captures take 10–60 s each.
5. **Look before editing.** Pull frames at chosen times from `footage/<scene>/desktop/frames.json` (each has `t`) and tile them with ffmpeg `hstack`/`vstack`; Read the image. Fix staging problems in the spec, not in Remotion.
6. **Edit.** Add the section to `sections` and to the cuts that use it in `edit.ts`; put keys and cues in `promo/src/tour/<scene>.ts` and the component in `promo/src/tour/<Name>.tsx`; add the scene to `scenes` and its cues to `cues()` in `timing.ts`; add the part to `Tour` (and `Promo` if it belongs there).
   - `retime` anchors: interactions are `fixed: true` (real speed); waiting and streaming are `fixed: false` and stretch to fill the section. If the elastic rate goes above ~1.5×, add bars or tighten the fixture's pacing.
   - Shots: `zoom(t, z, focusEvent)` with z from 1 to 1.6. Zoom only to recorded focus boxes, return to 1 for overviews, and check that the zoom doesn't crop the subject.
   - Captions: `<Lower from to>` on desktop, the `side` style beside the phone. Use frames from `useClock(keys).at(footageTime)`.
   - Music styles: `intro`, `groove`, `lift`, `break` (no drums), `peak`, `outro`. Cues (`notify`, `tick`, `tap`, `sparkle`, `whoosh`) go on the key moments in `cues()`.
7. **Music.** Run `pnpm -C promo music` (renders both cuts). You can't hear it, so verify with `ffmpeg -af ebur128` and a `showspectrumpic`, and say that the user has to judge the sound.
8. **Render.** Typecheck with `pnpm -C promo exec tsc -p .`. Spot-check with `pnpm -C promo exec remotion still src/index.ts <Promo|Tour> out/stills/N.png --frame=N`, then run `pnpm -C promo render` (Promo) or `pnpm -C promo tour` (Tour).
9. **README copy.** GitHub attachments must stay under 10 MB. Encode two-pass x264 `-tune animation` at `-b:v ≈ (9.5 MB × 8 / seconds) − 96k`, AAC 96k, `+faststart`, into `promo/out/promo-readme.mp4`, and check one dense frame for readable text.
10. **Show it** with `mcp__stampede_chat__render_media` on `promo/out/promo.mp4` or `promo/out/tour.mp4`.

## Builders

- `Chat(seed)` methods: `.prompt()`, `.say(md)`, `.read(path)`, `.grep(pattern, files)`, `.agent(task, type, prompt, scout => …, report)`, `.todos([[text, status]])`, `.html(title, html)`, `.media(path, title, asset)`, `.ask([question, answer], …)`, `.delegate(rig, task)`, `.stage(command, gate)`, `.edit(path, before, after)`, `.create(path, text)`, `.bash(cmd, desc, out)`, `.approve(cmd, desc, rule, out)`, `.epic(action)`, `.run(...argv)`, `.sleep(s)`, `.end(summary)`, `.write(path)`.
  - Edits really write files, which fills the Changes sidebar. Paths are relative to the seed, or absolute (canvases live in `<root>/data/canvases/<dir>/`), and `before` must match exactly once.
  - `.media` needs the asset already in `<root>/data/media/` (publish it with `chat-tools` render_media, see `media.spec.ts`). `.delegate` works once per Chat and must finish within 120 s. `.stage` waits on a gate the spec opens after calling `chat-tools --chat-id … stage_command` (see `terminals.spec.ts`).
- `Codex(seed, prompt)` methods: `.say`, `.plan([[step, 'pending'|'inProgress'|'completed']])`, `.read`, `.edit`, `.approve(cmd, reason, out)`, `.sleep`, `.end`, `.write`.
  - Edits are display only.
- Fixture paths in a Rig's `STAMPEDE_FAKE_FIXTURE` must be absolute. The fakes match stdin by message kind only, never by text.

## Gotchas

- **Footage must be real.** Never pass one agent off as another by swapping logos or text; write a fixture for the real agent. Anything drawn in Remotion (lock screen, keycaps) must match what Stampede actually shows. The real push notification is `"<Rig> — <status>"` from the app named after the Host.
- **Keep the user's machine out of frame.**
  - The fakes keep agent state in the test root.
  - Drawer shells start with `exec env -i HOME=<root> PATH=/usr/bin:/bin GIT_PAGER=cat PS1='\W $ ' PROMPT_COMMAND='printf "\033]0;%s\007" "${PWD##*/}"' bash --norc --noprofile`. The title must come from `PROMPT_COMMAND`: a title set once is dropped when the shell retakes the foreground, and the tab falls back to `bash — <real path>`.
  - Project Terminals start the server's `$SHELL`; `terminals.spec.ts` points it at a sanitizing wrapper.
  - Keep the Project rail collapsed; the expanded rail shows temp paths.
  - The Host is "Studio" and commits are by "Ada Lin".
- Top-level Claude text only renders when streamed; `say()` handles that. Use `.ask()` (AskUserQuestion) rather than the MCP `ask_questions` card, which leaves `questions-result.json` in Changes.
- After a question card closes, the transcript can unpin; `mouse.wheel(0, 6000)` re-pins it. Use `rest()` after clicks so tooltips don't linger.
- Prefer role-based locators. `.diff` also matches inline edit cards, and `.terminal` also matches tabs.
- Draft uses `commit_rig = "Claude Haiku"`, whose fixture's `result` is the message.
- Language servers install from `<target>/tmp/lsp-cache` when it has them (see `editor.spec.ts`), so the switch reaches "on" in about a second.
- **Epics:**
  - Create them with `$STAMPEDE_E2E_BINARY epic add … --data-dir <root>/data` with cwd in the repo; no REST call creates one.
  - Set the Run's Rig explicitly.
  - Ticket Chats share one fixture and pick their files with `git rev-parse --abbrev-ref HEAD` (see `epic.spec.ts`). Put any delay before `epic complete`, because the Chat closes once its Ticket merges.
  - `file` entries don't create directories.
- Run every test through `./check`, per AGENTS.md. Remotion's renderer is its own headless Chrome and is separate from the app.
