# Stampede spans Hosts through one Home Host, and Projects stay per Host

Devices open one Home Host, which shows and relays the Projects of its Linked Hosts; a Project remains a directory on one Host.

- Runs and Chats keep working with no Device connected, so Hosts, not the browser, coordinate with each other.
- One origin keeps PWA install, push notifications and Browser previews (ADR 0012) as they are.
- One Attention list covers every Host.
- Linked Hosts reach each other only through Home. Accepted cost: Runs that span Hosts wait while Home is down.
- Rejected: the browser connecting to every Host, because Runs spanning Hosts would need an open Device and each Host would be its own origin.
- Rejected: a Project as one repository with a checkout per Host, because Terminals, files, Uncommitted changes and language servers belong to a checkout anyway, and matching clones to a repository is guesswork.
