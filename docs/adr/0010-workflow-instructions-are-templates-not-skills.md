# Workflow instructions are Oikonomos Templates, not agent skills

Grilling, writing a spec, writing Tickets, working a Ticket, and resolving a merge are Templates built into Oikonomos and sent as part of a prompt, rather than skills installed for each Agent.

- The Epic and the loop are self-contained in Oikonomos, and David no longer wants to maintain skills outside it.
- The actions already live in the `oikonomos` CLI, so all a skill would add is instructions, which Oikonomos can send itself.
- Templates name CLI commands and Ticket statuses, so they change in the same commit as those.
- Accepted cost: a Template reaches an Agent only through Oikonomos, from a Chat's slash menu or the Composer.
- Rejected: Template files in the config directory, editable without a rebuild, because Oikonomos restarts constantly anyway and keeping them beside the code is the point.
- Rejected: keeping `grill-with-docs`, `to-spec`, `to-tickets`, and `ticket-tracker` as skills.
