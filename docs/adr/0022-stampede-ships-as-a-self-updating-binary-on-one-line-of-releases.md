# Stampede ships as a self-updating binary on one line of Releases

A curl script installs Stampede as a user service on Linux, and one click in the web client takes an Update to the newest Release.

- Once running, the whole app is managed from the web client, so starting, stopping and Updating need only the CLI and Settings.
- Releases are cut by hand when a feature will stick, and often, because the Agent CLIs change underneath Stampede.
- Release artifacts are signed and Stampede verifies them, in place of operating-system code signing.
- macOS and native Windows are not planned yet; Windows is served through WSL2.
- Rejected: stable and nightly channels, or following main, in favour of a single semver line versioned by changesets.
- Rejected: npm, PyPI and other package routes, because each adds a second way to update.
- Rejected: a tray or desktop app to manage the service, for the reasons in ADR 0001.
