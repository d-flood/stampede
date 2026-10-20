# Stampede

A self-hosted IDE for directing AI coding agents, served from the development machine and used from a browser on any computer or phone.

## Language

### Structure

**Host**:
The development machine that runs Stampede, its Projects, and their Terminals.
_Avoid_: server, box, machine

**Host name**:
The name a Host goes by wherever it identifies itself, such as its installed app, so that several Hosts can be told apart; "Stampede" unless set.
_Avoid_: hostname, server name, alias, label

**Device**:
A single browser window connected to Stampede, with its own layout and Focused Project; two windows on one computer are two Devices.
_Avoid_: client, session, browser

**Project**:
A directory on the Host, usually a git repository root, that owns its Terminals, open files, git view, and language servers. A git worktree is opened as its own Project.
_Avoid_: workspace, repo, folder

**Focused Project**:
The one open Project a Device is currently showing; every other open Project keeps running in the background.
_Avoid_: active project, current project

**Terminal**:
A persistent shell belonging to a Project or to the Drawer that keeps running regardless of which Devices are connected or whether Stampede restarts.
_Avoid_: session, pty, pane

**Ended Terminal**:
A Terminal lost to a Host reboot, kept as a read-only record of its last output until dismissed.
_Avoid_: dead terminal, zombie

**Drawer**:
A slide-in panel of Terminals that belong to no Project, for quick commands.
_Avoid_: global terminal, quake terminal, scratch terminal

### Agents

**Agent**:
The underlying AI coding harness, such as Claude Code or Codex, driven by a Chat.
_Avoid_: assistant, bot, AI, harness

**Subagent**:
A child conversation a Chat's Agent spawns to carry out a delegated piece of work and report back; it is not itself an Agent or a Chat.
_Avoid_: task, sidechain, helper, delegate

**Main**:
A Chat's own conversation, as distinct from those of its Subagents.
_Avoid_: parent, root, main session

**TUI**:
An Agent running its own terminal interface, started by the user in an ordinary Terminal. Stampede does not manage its lifecycle or status.
_Avoid_: agent, agent terminal

**Chat**:
An Agent that Stampede drives directly and shows in its own interface; it keeps running when Devices disconnect or Stampede restarts, stops when its Tab is closed.
_Avoid_: conversation, thread, session, native agent

**Agent status**:
One of _working_, _waiting_ (blocked mid-task on the user, e.g. a permission prompt), or _done_ (finished its turn, with no Subagent still running, and sitting at its prompt).
_Avoid_: state, activity, idle

**Template**:
Instructions Stampede owns and sends as part of a prompt, such as a Ticket's first prompt, the Resolver's, or _/grill_, _/spec_, and _/tickets_.
_Avoid_: skill, prompt file, command

**Clip**:
Text David saves and reuses, inserted at the caret in any Composer by typing `;` and its name, and editable there before sending; unlike a Template, Stampede never expands or recognizes it.
_Avoid_: snippet, macro, canned prompt, saved prompt, skill

**Rig**:
A named configuration for a Chat — its Agent, model, effort, permission mode and environment — started in a Project with one click or shortcut. Rigs also configure the Chats used by Runs.
_Avoid_: preset, profile, launcher

### Tickets

**Epic**:
A bounded body of work in one Project, made of a Brief and the Tickets that carry it out.
_Avoid_: milestone, feature, project

**Archived**:
True of an Epic hidden from every list without being destroyed.
_Avoid_: deleted, closed

**Brief**:
An Epic's governing document: the spec or map every Ticket's work answers to.
_Avoid_: spec, plan, description

**Ticket**:
One slice of an Epic, sized for a single Chat, naming the Tickets that block it.
_Avoid_: task, issue, story, card

**Ticket status**:
One of _not started_, _in progress_ (held by a Chat), _merging_ (complete and waiting for or undergoing its merge), _merged_, or _skipped_.
_Avoid_: state, stage, column

**Ticket tag**:
One of _Routine_ (predictable implementation), _Judgment_ (autonomous decisions), or _Frontend design_ (visual or interaction design). A Ticket can have none or several; tags describe its work without selecting a Rig.

**Ticket Rig**:
An optional saved Rig choice for a Ticket, overriding the Run's default when its Chat starts. Clearing it restores the default; it can be changed while the Ticket is not started.

**Blocker**:
A Ticket that must be merged into its Epic's branch, or skipped, before another Ticket may be taken.
_Avoid_: dependency, prerequisite, parent

**Frontier**:
Every Ticket that can be taken right now: not started, not held by a Chat, and with every Blocker resolved.
_Avoid_: ready queue, backlog, next up

**Run**:
One pass of the loop over an Epic, starting a Chat for each Frontier Ticket up to its parallel limit, then its Review when it names Reviewers, until nothing is left to start or it is stopped.
_Avoid_: job, batch, execution, cycle

**Paused**:
True of a Run that starts no new Chats while those already working carry on, either by hand, because its Agent's usage ran out, or because storage is low. Automatic pauses resume once usage resets or enough space is available.
_Avoid_: stopped, suspended, halted

**Claim**:
The record that a Chat holds a Ticket.
_Avoid_: lock, lease, assignment

**Verdict**:
What a Chat records about the Ticket it holds, either _complete_ or _skip_ (the codebase has made the Ticket moot); the only account of outcome the loop reads.
_Avoid_: result, status, exit code

**Usage window**:
One of an Agent's rolling usage allowances, such as its five-hour or weekly window.
_Avoid_: quota, rate limit, limit

**Note**:
A durable message an Agent or David attaches to a Ticket: a decision, an answer the Ticket asked for, or a hand-off. Stampede attaches its own, and a refused Landing becomes a Note on the Epic.
_Avoid_: comment, log, message

**Setup Command**:
What makes a fresh Ticket Worktree or Integration Worktree runnable, decided for each Epic when its Tickets are written.
_Avoid_: bootstrap, provision, install

**Ticket Worktree**:
The git worktree one Ticket is worked in, cut from its Epic's branch.
_Avoid_: workspace, sandbox, checkout

**Integration Worktree**:
The one worktree on an Epic's branch, into which finished Tickets merge one at a time and which David inspects before Landing.
_Avoid_: main worktree, staging

**Resolver**:
A Chat started in the Integration Worktree to resolve a finished Ticket's conflicted merge.
_Avoid_: merge agent, merger

**Review**:
The last stage of a Run that names Reviewers, once every Ticket is merged or skipped: its Reviewers examine the Epic branch, then its Remediator deals with what they found. A Run has at most one.
_Avoid_: finalization, audit, QA

**Reviewer**:
A Chat a Review starts with one of its Run's chosen Rigs, in a throwaway worktree of the Epic branch, to record Findings and then report.
_Avoid_: critic, auditor, review agent

**Finding**:
A defect a Reviewer records against an Epic, which the Remediator accepts or rejects with a reason.
_Avoid_: issue, comment, bug report

**Remediator**:
The one Chat that, once every Reviewer has reported, judges every Finding and fixes the accepted ones in the Integration Worktree, after which its Run finishes.
_Avoid_: fixer, judge

**Landing**:
Merging an Epic's branch into the branch it was cut from, in the Project's own checkout, done by hand and refused rather than forced.
_Avoid_: merge, ship, release

### Changes

**Uncommitted changes**:
Every difference between the files on disk and the Project's most recent commit; what the Changes section lists.
_Avoid_: unstaged changes, working changes

**Change marker**:
An indication beside a line in the editor that it differs from the most recent commit.
_Avoid_: git decoration, dirty marker

### Layout

**Project rail**:
The column of open Projects used to hop between them, collapsed to icons or expanded to show each one's path, each marked when something inside needs attention or a Run is live; its worktree Projects, an Epic's or made by hand, are counted on the Project they belong to and listed under it until deleted.
_Avoid_: project list, project sidebar

**Tab**:
One thing open in a Project's main window: a Chat, a Terminal, a file, a diff, or an Epic.
_Avoid_: pane, panel, view

**Chat tab**:
A Tab showing a Chat; it carries the Agent's logo and changes appearance with Agent status.
_Avoid_: agent tab, chat view, conversation tab

**Epic tab**:
A Tab showing one Epic: its Brief, its Tickets and their Notes, its Run, and Landing.
_Avoid_: board, epic view

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
The collapsible panel beside the main window, showing one section at a time: Files, Changes, History, Search, or Epics.
_Avoid_: explorer, wing, panel; never call its sections "tabs"

**Needs attention**:
True of a Tab whose Agent is waiting, or is done and not yet viewed on any Device.
_Avoid_: unread, alert

**Attention list**:
The dropdown of every Tab, across all Projects, that needs attention.
_Avoid_: notification center, inbox

**Composer**:
A native text box for writing input that is sent into a Terminal or a Chat all at once.
_Avoid_: input box, prompt box
