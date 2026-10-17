# Browser previews use a separate origin for each Host endpoint

A Browser Tab embeds a development server through a loopback reverse proxy. The proxy preserves paths, streams HTTP bodies, forwards WebSockets, and uses the development server's Host and Origin headers. Root-relative assets and Vite HMR work through the preview origin.

- Local HTTP Devices use a separate loopback port.
- Devices opened through Tailscale HTTPS use a temporary foreground `tailscale serve` endpoint on that proxy's port. It exits with Oikonomos; existing background Serve endpoints remain in place. The Host user needs permission to run Serve, and tailnet policy must permit the preview port.
- Only HTTP loopback targets are accepted: `localhost`, `127.0.0.1`, and `[::1]`. Oikonomos's own ports are excluded.
- Proxies are reused while Oikonomos runs. Browser Tabs and entered addresses persist in the Device layout, including splits.
- Framing restrictions are removed from preview responses. Other content security policy directives remain intact.
- Hardcoded localhost URLs in application scripts still refer to the Device. Configure those applications to use relative requests or their own development proxy.
- The address bar tracks entered addresses. Links navigate inside the iframe; reload returns to the entered address.
- HTML responses receive an external console bridge before application scripts. It forwards console output, uncaught errors and unhandled promise rejections to a collapsible Console panel using messages checked against the iframe window and origin. Normal browser console output remains available. Entries are bounded to the latest 500 and reset on page navigation or reload.
- The bridge obeys script policies, including metadata preceding application scripts. Policies excluding the bridge can prevent capture. Worker output and browser-generated diagnostics require DevTools. The proxy requests uncompressed responses; responses still sent compressed pass through without capture. Logs remain in the Device's Browser Tab memory.

Open one with **New Tab → Browser**, or follow an HTTP loopback link in a Project Terminal. Enter an address such as `localhost:5173`, then select **Go**.
