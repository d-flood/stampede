import type { Item, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test, type WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { openProject } from './shell.ts';

test('agent messages stay visible with collapsed streaming tool counts between them', async ({ page }, { project }) => {
	let route: WebSocketRoute;
	let chatId: string;
	const items: Item[] = [
		{ kind: 'text', text: 'I’ll read the files.', phase: 'commentary', streaming: false },
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'notes.txt' }] }
	];
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') { route = socket; chatId = message.chat.id; }
			socket.send(message.type === 'chat.items' && message.chatId === chatId ? JSON.stringify({ ...message, items }) : data);
		});
	});
	const name = `calls-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const panel = page.locator('.panel[role="tabpanel"]:visible');
	const summaries = panel.locator('.calls > summary');
	await expect(panel.getByText('I’ll read the files.', { exact: true })).toBeVisible();
	await expect(summaries).toHaveText('1 tool callRead notes.txt');
	await expect(panel.getByText('Read 1 file', { exact: true })).toBeHidden();

	function update(index: number, item: Item) {
		items[index] = item;
		route.send(JSON.stringify({ type: 'chat.item', chatId, index, item }));
	}
	update(1, { kind: 'tools', calls: [
		{ kind: 'read', name: 'Read', detail: 'notes.txt' },
		{ kind: 'search', name: 'Grep', detail: 'needle' }
	] });
	await expect(summaries).toHaveText('2 tool callsGrep needle');
	await expect(panel.locator('.calls[open]')).toHaveCount(0);
	update(2, { kind: 'text', text: 'I found the match. I’ll count its lines.', phase: 'commentary', streaming: false });
	update(3, { kind: 'command', command: 'wc -l notes.txt', status: 'running', output: '' });
	await expect(summaries).toHaveText(['2 tool callsGrep needle', '1 tool call$ wc -l notes.txt']);
	await expect(panel.getByText('I found the match. I’ll count its lines.', { exact: true })).toBeVisible();
	await expect(panel.locator('.command')).toBeHidden();
	if (project.name === 'pixel') await summaries.last().tap();
	else await summaries.last().click();
	await expect(panel.locator('.command')).toBeVisible();
	update(3, { kind: 'command', command: 'wc -l notes.txt', status: 'ok', output: '3 notes.txt' });
	await expect(panel.locator('.calls[open]')).toHaveCount(1);
	await expect(summaries.last()).toContainText('1 tool call');
	await panel.locator('summary', { hasText: '$ wc -l notes.txt' }).last().click();
	await expect(panel.getByText('3 notes.txt', { exact: true })).toBeVisible();
	update(4, { kind: 'text', text: 'The file has three lines.', phase: 'final', streaming: false });
	await expect(panel.getByText('The file has three lines.', { exact: true })).toBeVisible();
	await summaries.last().click();
	await expect(panel.locator('.command')).toBeHidden();
	await expect.poll(() => summaries.first().evaluate((element) => getComputedStyle(element).display)).toBe('flex');
	await expect.poll(() => summaries.first().evaluate((element) => element.getBoundingClientRect().height)).toBeGreaterThanOrEqual(project.name === 'pixel' ? 48 : 26);
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await page.screenshot({ path: test.info().outputPath('collapsed-calls.png') });
});
