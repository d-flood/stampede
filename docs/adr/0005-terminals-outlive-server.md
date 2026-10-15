# Terminals run in holder processes that outlive the server

Each Terminal lives in its own small process that the server reattaches to, so Agents survive server restarts, upgrades, and crashes.

- Oikonomos will be developed with Agents running inside it, so server restarts will be constant.
- A Host reboot still ends Terminals; their history returns as Ended Terminals with Resume.
- Rejected: Terminals owned directly by the server, because every upgrade would kill every Agent.
