# Computer-use design interview

Design interview record, not a specification. Product decisions are settled; final confirmation of shared understanding is pending. No implementation, specification, or tickets are authorized by this interview.

## Settled

- Exactly two phases; phase two is the complete intended product. Exclusions are explicit, not deferred to another phase.
- Computer use supports general user-directed desktop tasks, including development.
- Phase one delivers usable control on the Host, including setup and stop controls, rather than only a technical prototype.
- This two-phase plan completes with Linux local and remote computer use. macOS and Windows integration belongs to their separate main-app platform rollout, gated on GitHub Actions building main-app binaries; it is not a third phase of this plan. This replaces the earlier unconditional three-platform requirement.
- X11, GNOME Wayland, and Sway must pass release tests in both phases. Other Linux compositors are outside the support guarantee. The phase split is local versus remote control, not incremental compositor support.
- Stampede's PWA remains the full interface. A small native helper provides desktop access, pairing, permissions, and stop controls; a full desktop Stampede application is not required.
- A Desktop is a remembered graphical environment for one computer and OS-user login, independent of a Device. Its identity survives logout and reboot; separate OS-user logins have separate Desktops. Only one active graphical session per login is supported; concurrent sessions for the same login are outside scope.
- Both phases require an existing logged-in graphical session. Provisioning virtual desktops is outside the two-phase plan.
- Helpers require existing reachability to the Host through localhost, the same network, or the tailnet. A public relay service is outside the plan.
- Paired computers provide GUI control only. Shell commands and Project files remain on the Host; remote shell and file transfer are outside the plan. GUI actions may manipulate files through applications.
- The product serves one trusted person using their computers. A separate multi-user account and access-rights system is outside the plan.
- Phase-one acceptance requires editing, saving, and reopening a scratch document in GNOME Text Editor; completing a Chrome form using an existing authenticated test profile; and changing and restoring an exposed native-app preference. These run on X11, GNOME Wayland, and Sway with a compatible vision-capable model for each of Claude Code, Codex, and Copilot.
- The user selects one Desktop per Chat. Opening that Chat from another Device never changes its selected Desktop.
- A Desktop permits only one controlling Chat at a time, including across multiple paired Hosts. The helper enforces exclusivity.
- Phase one must pass real compatibility tests with Claude Code, Codex, and Copilot.
- Authorized desktop work continues when all Devices close, provided the Host and helper remain connected.
- A Desktop may be paired with multiple Hosts.
- Both phases provide the current action, on-demand screenshots, and stop controls. Continuous video and manual remote mouse/keyboard control are outside the plan.
- Selecting a Desktop and authorizing its use are distinct. The user explicitly grants one Chat access to its selected Desktop, covering all applications until revoked or the Chat closes.
- A permissive Rig does not automatically grant Desktop access. Existing Agent approval rules still apply to consequential actions.
- Stop computer use revokes the Chat's Desktop grant and stops further helper actions.
- GUI use of a terminal is allowed within the user's task. The remote-shell exclusion concerns a separate remote-shell capability, not commands entered through a GUI.
- Whole-Desktop authorization was chosen over per-application grants: cross-application tasks would become more cumbersome, and granting a browser still exposes a broad environment.
- Desktop access is authorization; Desktop control is exclusive use for the current Agent turn, including mid-turn waits. Completion or interruption releases control without revoking access.
- A busy Desktop reports the controlling Chat. Takeover requires explicit user action; there is no automatic queue.
- Subagents within a Chat share its Desktop access and serialized actions. Delegated Chats and Run Chats require their own explicit grants.
- Helper disconnection, Desktop lock or sleep, and Host or helper restart stop input and release control. Access is preserved, but work requires explicit user resume and a fresh observation. Uncertain actions are never replayed.
- Ordinary Chat interruption stops desktop actions and releases control while retaining access for a later turn. Stop computer use additionally revokes access.
- Cua Driver supplies accessibility inspection and native actions through MCP to existing Agents. Screenshots provide verification and fallback; no separate vision-model service is added. Runtime compatibility must be verified against release requirements.
- Phase-two pairing starts with PWA download and user installation. A short-lived pairing link or code opens the helper; the user confirms the named Host locally, then the helper connects outward to that Host.
- Either the helper or the Host's PWA can revoke their pairing, ending that Host's Desktop access without removing other Host pairings.
- Explicit takeover stops the old Chat's desktop actions and revokes its Desktop access before another Chat takes control. The old Chat cannot automatically reclaim control on a later turn.
- Human intervention uses explicit stop controls. The Agent may focus applications when needed; ordinary human keyboard or mouse activity does not automatically pause automation.
- Enhanced browser-debugging/profile attachment is an explicit opt-in in both phases wherever supported; native accessibility and visual control remain the required baseline. This replaces the earlier exclusion.
- The helper owns the local browser-debugging connection; Stampede does not expose its debugging port remotely. Enabling attachment explains persistent browser settings and how to disable them.
- Normal Chat history is retained. Stampede adds no separate desktop recording or screenshot archive; saving artifacts requires explicit intent. This does not promise that Agent transcripts omit tool arguments, results, or screenshots.
- The helper supports Linux x86_64 in both phases. Linux ARM64 is outside this plan.
- A small Tauri helper provides a normal local control window for pairing, status, and Stop. A tray menu is supplementary where available; a universal global emergency hotkey is not required.
- Phase-one Stampede setup offers optional helper installation, followed by explicit PWA activation. The same helper is reused for phase two; users do not have to install and configure Cua separately.
- Linux-only GitHub Actions builds produce versioned helper packages linked from the PWA, independently of future main-app platform builds.
- The helper bundles a tested Cua version. Downloaded updates are verified, installed only at user request while idle, and incompatible Host/helper combinations are refused with a clear message.
- Both phases support windows across multiple monitors, including mixed display scaling, with corresponding acceptance tests.
- Settings → Desktops provides installation, pairing, naming, and removal. A strip above the Chat Composer provides Desktop selection, access, status, preview, and Stop; mobile has equivalent capabilities through a compact strip and sheet.
- Delivery starts with an AppImage and per-user desktop integration, documenting system dependencies and offering optional start-at-login. Starting the helper does not authorize a Chat.
- Changing a Chat's selected Desktop revokes the old grant and requires a new one. Fresh Desktop observations require access. Browser attachment has its own opt-in and respects backend authorization expiry. Losing a pairing requires pairing again.
- Phase-one release gates include semantic actions, visual fallback, mixed-scale monitors, busy-Desktop refusal, explicit takeover, local and PWA Stop, and recovery after lock, sleep, and restart without repeating uncertain actions.
- Phase two repeats the same workflows on a separate paired Linux computer and adds two-Host contention, pairing revocation, failed connections, and compatible/incompatible helper update tests. These remote capabilities complete the second and final phase.
- Support is demonstrated for named application/version combinations, not every application. Stop cannot undo completed actions. Desktop grants govern Stampede's helper integration, not an OS sandbox against unrestricted same-user shell commands.

The acceptance gates above are agreed requirements, not claims of completed runtime validation.

## Design tree

- [x] General desktop work
  - [x] Native accessibility and actions, visual fallback, optional browser attachment
  - [x] Named application workflows and all three Agents
- [x] Two-phase scope
  - [x] Phase one: complete Host-local desktop control
  - [x] Phase two: paired remote desktop control and multi-Host coordination
  - [x] Linux x86_64, X11/GNOME Wayland/Sway, multiple monitors
  - [x] Other platform rollouts and explicit exclusions
- [x] Domain relationships
  - [x] Desktop identity independent of Device, stable across login/reboot
  - [x] Desktop pairing distinct from Chat access and exclusive control
  - [x] Target selection, Subagents, delegated Chats, Runs, and contention
- [x] Helper delivery
  - [x] Tauri, AppImage, optional setup installation and login startup
  - [x] Linux CI downloads, tested Cua runtime, verified user-requested updates
  - [x] Pairing, revocation, compatibility, and local control window
- [x] Control lifecycle
  - [x] Grant scope, expiry, target changes, interruption, Stop, and takeover
  - [x] Disconnect, lock, sleep, restart, fresh observation, and uncertain outcomes
- [x] PWA integration
  - [x] Desktop settings and per-Chat controls, including mobile
  - [x] Action status, screenshots, normal Chat history, explicit artifact saving
- [x] Phase-specific release gates and limits
- [ ] Final user confirmation of shared understanding
