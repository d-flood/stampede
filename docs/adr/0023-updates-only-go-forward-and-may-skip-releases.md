# Updates only go forward and may skip Releases

Every Release reads what any earlier Release wrote, and none is expected to read what a later one wrote.

- A Host that was off for months jumps straight to the newest Release.
- An older Stampede misreads a newer config and cannot reattach Terminals a newer one started (config.rs, terminals.rs).
- The one step backward is restoring the previous Release when an Update fails to start.
- Rejected: moving a Host to an older Release on request, because every stored format would have to be safe to downgrade.
- Rejected: stepping through each Release in order.
