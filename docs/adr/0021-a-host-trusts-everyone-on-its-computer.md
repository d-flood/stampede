# A Host trusts everyone on its computer

A Host answers only requests sent to an address it knows as its own, asks for no sign-in, and over the tailnet answers only its owner.

- Stampede is for one person on their own Host and Devices.
- Any web page open in a browser on the Host could otherwise drive Terminals; refusing unknown addresses stops that with no sign-in step on any Device.
- Other OS users on the same computer can still reach the Host, so a Host must be a single-user computer.
- Beyond the Host itself, Devices come through Tailscale, or through a reverse proxy whose address the user lists.
- A tailnet may be shared with other people, so requests through it are served only for the login that owns the Host and logins the user lists; Tailscale supplies the login.
- Rejected: a Host secret or password every Device presents, because it adds a sign-in step on each browser and phone.
- Rejected: listening on the local network, because without HTTPS a Device loses install, push and clipboard.
