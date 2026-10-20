# Stampede owns Epics, Tickets, and the loop; Botley is retired

Ticket state and the loop that works through it move into Stampede, which runs each Ticket as a Chat; Botley is no longer developed.

- Chats report exactly when a turn ends and let David answer approvals and questions from any Device, so most of Botley's machinery for watching opaque TUIs is no longer needed.
- Botley Runs rarely finished an Epic without being rescued after they died; steering stuck Chats directly is meant to replace those rescues.
- Accepted cost: loops run only Claude Code and Codex, so Botley's other harnesses drop out.
- Accepted cost: Finalization is dropped; a Run's Review later took its place.
- Ticket state lives in Stampede's data directory, outside project repos, and open Botley Epics are finished in Botley rather than imported.
- Rejected: keeping Botley and making Stampede its only Host.
