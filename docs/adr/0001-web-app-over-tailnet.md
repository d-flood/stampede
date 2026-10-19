# Stampede is a web app served from the Host over the tailnet

The UI is a web app served by a local server on the Host and opened from any browser on the tailnet.

- Phone and other computers get full access by opening a URL, with no mobile-specific setup.
- Web technology is expected to be fast enough when the app keeps its state warm.
- Rejected: Electron, because David doesn't want it; Orca, the closest existing tool, is Electron-based and feels sluggish.
- Rejected: a native UI (Rust, Quickshell), because a browser covers desktop and phone with one client.
