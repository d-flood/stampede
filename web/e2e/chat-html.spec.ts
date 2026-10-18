import type { Item, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import { mkdirSync } from 'node:fs';
import { closeProject, openProject } from './shell.ts';

const snippet: Item = {
	kind: 'html',
	title: 'Research workflow',
	html: `<style>
		.flow { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }
		.flow div { padding: 16px; background: #e0e7ff; color: #172554; border-radius: 8px; }
		@media (max-width: 500px) { .flow { grid-template-columns: 1fr; } }
	</style>
	<div class="flow"><div>Collect</div><div>Collate</div><div>Publish</div></div>
	<button onclick="document.querySelector('#more').hidden = !document.querySelector('#more').hidden">Details</button>
	<div id="more" hidden style="height:300px">Additional context</div>
	<script>
		try { parent.document.body.dataset.snippetEscaped = 'yes'; } catch { document.body.dataset.isolated = 'yes'; }
		fetch('/api/rigs').then(() => document.body.dataset.network = 'allowed').catch(() => document.body.dataset.network = 'blocked');
	</script>`
};

test('chat HTML is responsive, isolated, interactive, and restored on reload', async ({ page }, { project, repeatEachIndex }) => {
	let chatId: string;
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') chatId = message.chat.id;
			if (message.type === 'chat.items' && message.chatId === chatId) {
				socket.send(JSON.stringify({ ...message, items: [snippet] }));
			} else socket.send(data);
		});
	});
	const name = `html-${project.name}-${repeatEachIndex}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const frame = page.locator('iframe[title="Research workflow"]');
	const content = frame.contentFrame();
	await expect(content.getByText('Collect', { exact: true })).toBeVisible();
	await expect(content.locator('body')).toHaveAttribute('data-isolated', 'yes');
	await expect(content.locator('body')).toHaveAttribute('data-network', 'blocked');
	await expect(page.locator('body')).not.toHaveAttribute('data-snippet-escaped');
	await expect(frame).toHaveAttribute('sandbox', 'allow-scripts');
	await expect.poll(() => content.locator('.flow').evaluate((node) => getComputedStyle(node).gridTemplateColumns.split(' ').length)).toBe(project.name === 'pixel' ? 1 : 3);
	await expect.poll(async () => {
		const body = await content.locator('body').evaluate((node) => Math.ceil(node.getBoundingClientRect().height));
		return frame.evaluate((node, body) => Math.abs(node.clientHeight - Math.max(80, body)) <= 1, body);
	}).toBe(true);
	const initial = await frame.evaluate((node) => node.clientHeight);
	await content.getByRole('button', { name: 'Details' }).click();
	await expect.poll(() => frame.evaluate((node) => node.clientHeight)).toBeGreaterThan(initial + 250);
	await content.getByRole('button', { name: 'Details' }).click();
	await expect.poll(() => frame.evaluate((node) => node.clientHeight)).toBe(initial);
	const panel = page.getByRole('tabpanel').filter({ has: frame });
	await expect.poll(() => panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
	await page.reload();
	await expect(content.getByText('Publish', { exact: true })).toBeVisible();
	await closeProject(page, name);
});
