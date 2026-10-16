# Oikonomos owns Epics, Tickets, and the loop; Botley is retired

Ticket state and the loop that works through it move into Oikonomos, which runs each Ticket as a Chat; Botley is no longer developed.

- Chats report exactly when a turn ends and let David answer approvals and questions from any Device, so most of Botley's machinery for watching opaque TUIs is no longer needed.
- Botley Runs rarely finished an Epic without being rescued after they died; steering stuck Chats directly is meant to replace those rescues.
- Accepted cost: loops run only Claude Code and Codex, so Botley's other harnesses drop out.
- Accepted cost: Finalization is dropped, to be rethought later.
- Ticket state lives in Oikonomos's data directory, outside project repos, and open Botley Epics are finished in Botley rather than imported.
- Rejected: keeping Botley and making Oikonomos its only Host.
