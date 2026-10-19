import { expect, test } from './fixtures.ts';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer } from 'vite';
import { openProject } from './shell.ts';

test('Browser Tabs proxy a Vite app with HMR and survive reopening', async ({ page }, { project }) => {
	const sockets: string[] = [];
	const nativeLogs: string[] = [];
	page.on('console', (message) => nativeLogs.push(message.text()));
	page.on('websocket', (socket) => sockets.push(socket.url()));
	const root = await mkdtemp(join(tmpdir(), 'oikonomos-preview-'));
	await writeFile(join(root, 'index.html'), `<!doctype html><html><head><title>Preview fixture</title><link rel="stylesheet" href="/style.css"><script>console.log('Preview startup', { phase: 'head' });</script></head>
		<body><h1>Host development preview</h1><input aria-label="Preview input"><button id="save">Save</button><p id="reply"></p><a href="/nested/path?q=1">Nested page</a>
		<script type="module">
			import { message } from '/message.ts';
			document.querySelector('h1').textContent = message;
			if (import.meta.hot) import.meta.hot.accept('/message.ts', (module) => { document.querySelector('h1').textContent = module.message; });
			document.querySelector('#save').onclick = async () => {
				const response = await fetch('/api/save', { method: 'POST', body: document.querySelector('input').value });
				document.querySelector('#reply').textContent = await response.text();
			};
		</script></body></html>`);
	await writeFile(join(root, 'style.css'), 'body { color: rgb(12, 34, 56); font-family: sans-serif; } input { max-width: 60%; }');
	await writeFile(join(root, 'message.ts'), 'export const message = "Host development preview";');
	const vite = await createServer({
		configFile: false,
		root,
		server: { host: '127.0.0.1', port: 0 },
		plugins: [{
			name: 'preview-fixture',
			configureServer(server) {
				server.middlewares.use('/blocked', (_request, response) => {
					response.setHeader('Content-Type', 'text/html');
					response.end('<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="script-src &apos;none&apos;"></head><body><h1>Blocked capture</h1><script>console.log("Blocked script");</script></body></html>');
				});
				server.middlewares.use('/api/save', (request, response) => {
					let body = '';
					request.on('data', (chunk) => body += chunk);
					request.on('end', () => response.end(`Saved on Host: ${body}`));
				});
			}
		}]
	});
	try {
		await vite.listen();
		const listening = vite.httpServer!.address();
		if (!listening || typeof listening === 'string') throw new Error('Preview fixture did not listen');
		const url = `http://localhost:${listening.port}/`;
		await openProject(page, 'alpha');
		if (project.name === 'desktop') {
			await page.getByRole('button', { name: 'New Tab', exact: true }).click();
			await page.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
			await expect(page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab')).not.toHaveCount(0);
		}
		await page.getByRole('button', { name: 'New Tab', exact: true }).click();
		await page.getByRole('menuitem', { name: 'Browser', exact: true }).click();
		const browser = page.getByRole('tabpanel', { name: 'Browser', exact: true });
		await browser.getByRole('textbox', { name: 'Host address' }).fill('http://example.com');
		await browser.getByRole('button', { name: 'Go', exact: true }).click();
		await expect(browser.getByRole('alert')).toContainText('Enter an HTTP address on the Host');
		await browser.getByRole('textbox', { name: 'Host address' }).fill(url);
		await browser.getByRole('button', { name: 'Go', exact: true }).click();
		const frame = browser.frameLocator('iframe');
		await expect(frame.getByRole('heading')).toHaveText('Host development preview');
		const src = await browser.locator('iframe').getAttribute('src');
		expect(new URL(src!).origin).not.toBe(new URL(page.url()).origin);
		expect(new URL(src!).port).not.toBe(String(listening.port));
		await expect.poll(() => sockets.some((socket) => socket.startsWith(new URL(src!).origin.replace('http:', 'ws:')))).toBe(true);
		expect(sockets.every((socket) => new URL(socket).port !== String(listening.port))).toBe(true);
		await expect.poll(() => frame.locator('body').evaluate((body) => getComputedStyle(body).color)).toBe('rgb(12, 34, 56)');
		await expect.poll(() => browser.evaluate((element) => {
			const input = element.querySelector('input')!.getBoundingClientRect();
			const frame = element.querySelector('iframe')!.getBoundingClientRect();
			return input.width > 80 && frame.height > 200 && element.scrollWidth <= element.clientWidth;
		})).toBe(true);
		await browser.getByRole('button', { name: /^Console / }).click();
		const output = browser.getByRole('log', { name: 'Console output' });
		await expect(output).toContainText('Preview startup { phase: head }');
		expect(nativeLogs.some((message) => message.startsWith('Preview startup'))).toBe(true);
		await frame.locator('body').evaluate(() => {
			const value: { self?: unknown; big: bigint; getter?: string } = { big: 42n };
			value.self = value;
			Object.defineProperty(value, 'getter', { enumerable: true, get() { throw new Error('Getter must not run'); } });
			console.warn('Circular value', value);
			console.error(new Error('Logged preview failure'));
			console.assert(false, 'Preview assertion');
			setTimeout(() => { throw new Error('Uncaught preview failure'); }, 0);
			Promise.reject(new Error('Rejected preview promise'));
		});
		for (const text of ['Circular value', '42n', '[Circular]', '[Getter]', 'Logged preview failure', 'Preview assertion', 'Uncaught preview failure', 'Rejected preview promise']) {
			await expect(output).toContainText(text);
		}
		await page.evaluate(() => window.postMessage({ type: 'oikonomos.console', kind: 'entries', entries: [{ level: 'error', text: 'Forged console message', time: Date.now() }] }, location.origin));
		await frame.locator('body').evaluate(() => window.parent.postMessage({ type: 'oikonomos.console', kind: 'entries', entries: [{ level: 'error', text: { invalid: true }, time: Date.now() }] }, '*'));
		await expect(output).not.toContainText('Forged console message');
		await expect.poll(() => browser.evaluate((element) => {
			const output = element.querySelector('.entries')!.getBoundingClientRect();
			const frame = element.querySelector('iframe')!.getBoundingClientRect();
			return output.height > 80 && frame.height > 200 && element.scrollWidth <= element.clientWidth;
		})).toBe(true);
		await page.screenshot({ path: test.info().outputPath(`browser-console-${project.name}.png`) });
		await browser.getByRole('button', { name: 'Clear console', exact: true }).click();
		await expect(output).toHaveText('No console output.');
		await frame.locator('body').evaluate(() => {
			for (let index = 0; index < 300; index++) console.log('Flood', index);
		});
		await expect(output).toContainText('200 console messages omitted');
		await frame.locator('body').evaluate(() => console.clear());
		await expect(output).toHaveText('No console output.');
		await browser.getByRole('button', { name: /^Console / }).click();
		await frame.getByRole('textbox', { name: 'Preview input' }).fill('kept through HMR');
		await frame.getByRole('button', { name: 'Save', exact: true }).click();
		await expect(frame.locator('#reply')).toHaveText('Saved on Host: kept through HMR');
		await writeFile(join(root, 'message.ts'), 'export const message = "Hot update from Host";');
		await expect(frame.getByRole('heading')).toHaveText('Hot update from Host');
		await expect(frame.getByRole('textbox', { name: 'Preview input' })).toHaveValue('kept through HMR');
		await page.screenshot({ path: test.info().outputPath(`browser-${project.name}.png`) });
		await frame.getByRole('link', { name: 'Nested page' }).click();
		await expect.poll(() => page.frames().some((frame) => frame.url() === `${src}nested/path?q=1`)).toBe(true);
		await browser.getByRole('button', { name: 'Reload preview', exact: true }).click();
		await expect.poll(() => page.frames().some((frame) => frame.url() === src)).toBe(true);
		if (project.name === 'desktop') {
			await browser.getByRole('textbox', { name: 'Host address' }).click();
			await page.keyboard.press('Control+;');
			await page.keyboard.press('s');
			await expect(page.getByRole('tablist', { name: 'Right Tabs' }).getByRole('tab', { name: `localhost:${listening.port}` })).toHaveAttribute('aria-selected', 'true');
		}
		await expect.poll(() => page.evaluate(async (url) => {
			const id = sessionStorage.getItem('deviceId');
			const layout = await (await fetch(`/api/devices/${id}/layout`)).json();
			return Object.values(layout?.projects ?? {}).some((project) =>
				(project as { tabs: { kind: string; url?: string }[] }).tabs.some((tab) => tab.kind === 'browser' && tab.url === url));
		}, url)).toBe(true);
		await page.reload();
		await expect(browser.getByRole('textbox', { name: 'Host address' })).toHaveValue(url);
		await expect(frame.getByRole('heading')).toHaveText('Hot update from Host');
		if (project.name === 'desktop') await expect(page.getByRole('separator')).toBeVisible();
		await browser.getByRole('button', { name: /^Console / }).click();
		await expect(output).toContainText('Preview startup');
		await expect.poll(() => browser.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
		await browser.getByRole('textbox', { name: 'Host address' }).fill(`${url}blocked`);
		await browser.getByRole('button', { name: 'Go', exact: true }).click();
		await expect(frame.getByRole('heading')).toHaveText('Blocked capture');
		await expect(output).toContainText('Waiting for console capture');
		await expect(output).not.toContainText('Blocked script');
		const close = page.getByRole('button', { name: `Close localhost:${listening.port}`, exact: true });
		await close.click();
		await expect(page.getByRole('tab', { name: `localhost:${listening.port}`, exact: true })).toHaveCount(0);
	} finally {
		await vite.close();
		await rm(root, { recursive: true, force: true });
	}
});
