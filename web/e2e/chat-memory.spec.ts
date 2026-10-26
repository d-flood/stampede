import type { ServerMessage } from '#lib/device.svelte.ts';
import type { WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { openProject } from './shell.ts';

for (const streaming of [true, false]) {
	test(`closing a Chat releases cached ${streaming ? 'streaming' : 'completed'} reply versions`, { tag: '@desktop' }, async ({ page }, { project }) => {
		let route: WebSocketRoute;
		let chatId: string;
		await page.routeWebSocket('**/ws', (socket) => {
			const server = socket.connectToServer();
			server.onMessage((data) => {
				if (typeof data !== 'string') return socket.send(data);
				const message: ServerMessage = JSON.parse(data);
				if (message.type === 'chat.created') { route = socket; chatId = message.chat.id; }
				socket.send(data);
			});
		});
		const name = `chat-memory-${project.name}`;
		mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
		await openProject(page, name);
		await page.getByRole('button', { name: 'New Tab' }).click();
		await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
		await expect(page.getByRole('textbox', { name: 'Composer' })).toBeVisible();
		const cdp = await page.context().newCDPSession(page);
		const heap = async () => {
			await cdp.send('HeapProfiler.collectGarbage');
			return (await cdp.send('Runtime.getHeapUsage')).usedSize;
		};
		const before = await heap();
		const text = 'A paragraph with **formatting** and `code`.\n\n'.repeat(1000);
		route!.send(JSON.stringify({ type: 'chat.item', chatId: chatId!, index: 0, item: { kind: 'text', text, streaming, phase: 'commentary' } }));
		const reply = page.locator('.panel:visible .text');
		await expect(reply.locator('p')).toHaveCount(1000);
		for (let index = 0; index < 100; index++) {
			const delta = ` Snapshot ${index}.`;
			route!.send(JSON.stringify({ type: 'chat.append', chatId: chatId!, index: 0, text: delta }));
			await expect.poll(() => reply.evaluate((node, suffix) => node.textContent!.trimEnd().endsWith(suffix), delta.trimStart()), { intervals: [10] }).toBe(true);
		}
		await page.getByRole('button', { name: 'Close Fake Claude', exact: true }).click();
		await expect(page.getByRole('tab', { name: 'Fake Claude', exact: true })).toHaveCount(0);
		await expect(page.locator('.panel .text')).toHaveCount(0);
		const after = await heap();
		console.log(`${project.name} ${streaming ? 'streaming' : 'completed'} Chat retained ${(after - before) / 2**20} MiB after closing`);
		expect(after - before).toBeLessThan(8 << 20);
	});
}
