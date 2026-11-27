# A Cloud Host runs its own Stampede, reached through its Home Host

Work moves off a Host onto Cloud Hosts, each a full Stampede created for one worktree and destroyed with it, which Devices reach only through its Home Host.

- Everything works on the Cloud Host unmodified; driving a bare machine through remote commands would need a second version of files, git, search, language servers and Chat tools.
- The cloud service's own CLI, installed and signed in by David, is the only integration, as with gh and the Agent CLIs.
- One Cloud Host per worktree keeps machines clean: no build output piles up and no storage is paid for unused work.
- Going through the Home Host keeps one origin, so PWA install and push stay as they are, and the cloud service's token never reaches the browser.
- Accepted cost: a Cloud Host can't be seen while its Home Host is off, though its Chats keep working.
- History travels between the two by git, with no origin and no file syncing. The one exception: when a Cloud Host is destroyed, its Sessions' snapshots move to the Home Host as Ended Sessions (ADR 0020).
- Rejected: Linked Hosts on the tailnet relayed through one Host, because Cloud Hosts can't join the tailnet and a client that connects to several Hosts looks simpler when that is wanted.
- Rejected: driving the machine through the cloud CLI alone, because every later feature would be built twice.
