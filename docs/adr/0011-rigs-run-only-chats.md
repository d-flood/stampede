# Rigs run only Chats

Every Rig launches a Chat on every Device. Runs, Claims, planning Templates, and other managed Agent workflows use Chats.

- A Rig configures the Agent, environment, model, effort, permission mode and shortcut. Legacy `command`, `args` and `resume` config keys are ignored.
- Users can start any TUI in an ordinary Terminal. Stampede manages that Terminal as a shell, with no TUI hooks, Agent status, attention, resume, handoff or Claim tracking.
- Terminal output and shell processes still survive a Stampede restart; Host reboots leave Ended Terminals. Chats keep their own session recovery.
- Supersedes ADR 0006 and the TUI handoff portions of ADR 0008. The loop uses only Chat Claims; TUI Claim transfers and Terminal planning Templates are dropped.
- Accepted cost: Agent management features absent from Chats must be used separately through the Agent's CLI.
