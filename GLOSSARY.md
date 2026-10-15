# Oikonomos

A self-hosted IDE for directing terminal-based AI coding agents, served from the development machine and used from a browser on any computer or phone.

## Language

### Structure

**Host**:
The development machine that runs Oikonomos, its Projects, and their Terminals.
_Avoid_: server, box, machine

**Device**:
A single browser window connected to Oikonomos, with its own layout and Focused Project; two windows on one computer are two Devices.
_Avoid_: client, session, browser

**Project**:
A directory on the Host, usually a git repository root, that owns its Terminals, open files, git view, and language servers. A git worktree is opened as its own Project.
_Avoid_: workspace, repo, folder

**Focused Project**:
The one open Project a Device is currently showing; every other open Project keeps running in the background.
_Avoid_: active project, current project

**Terminal**:
A persistent shell belonging to a Project or to the Drawer that keeps running regardless of which Devices are connected or whether Oikonomos restarts.
_Avoid_: session, pty, pane

**Ended Terminal**:
A Terminal lost to a Host reboot, kept as a read-only record of its last output until dismissed.
_Avoid_: dead terminal, zombie

**Drawer**:
A slide-in panel of Terminals that belong to no Project, for quick commands.
_Avoid_: global terminal, quake terminal, scratch terminal

### Agents

**Agent**:
A terminal-based AI coding CLI, such as Claude Code or Codex, running inside a Terminal.
_Avoid_: assistant, bot, AI

**Agent status**:
One of _working_, _waiting_ (blocked mid-task on the user, e.g. a permission prompt), or _done_ (finished its turn and sitting at its prompt).
_Avoid_: state, activity, idle

**Rig**:
A named, configured way of launching an Agent — its command, flags, and settings — started into a Project with one click or shortcut.
_Avoid_: preset, profile, launcher

### Changes

**Uncommitted changes**:
Every difference between the files on disk and the Project's most recent commit; what the Changes section lists.
_Avoid_: unstaged changes, working changes

**Change marker**:
An indication beside a line in the editor that it differs from the most recent commit.
_Avoid_: git decoration, dirty marker

### Layout

**Project rail**:
The column of open Projects used to hop between them, each marked when something inside needs attention.
_Avoid_: project list, project sidebar

**Tab**:
One thing open in a Project's main window: an Agent, a Terminal, a file, or a diff.
_Avoid_: pane, panel, view

**Agent tab**:
A Tab showing a Terminal running an Agent; it looks distinct from Terminal and Editor tabs, changes appearance with Agent status, and carries the Agent's logo.
_Avoid_: agent pane

**Editor tab**:
A Tab showing one file in the editor.
_Avoid_: file tab, buffer

**Diff tab**:
A Tab comparing a file against a commit; editable when showing Uncommitted changes, read-only for history.
_Avoid_: diff view, diff tile

**Split**:
Two Tabs shown side by side in the main window.
_Avoid_: tile, pane

**Sidebar**:
The collapsible panel beside the main window, showing one section at a time: Files, Changes, History, or Search.
_Avoid_: explorer, wing, panel; never call its sections "tabs"

**Needs attention**:
True of a Tab whose Agent is waiting, or is done and not yet viewed on any Device.
_Avoid_: unread, alert

**Attention list**:
The dropdown of every Tab, across all Projects, that needs attention.
_Avoid_: notification center, inbox

**Composer**:
A native text box for writing input that is sent into a Terminal all at once.
_Avoid_: input box, prompt box
