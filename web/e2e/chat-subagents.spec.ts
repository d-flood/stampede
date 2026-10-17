import type { Item, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test, type WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { openProject } from './shell.ts';

test('completed subagents leave the picker and return their selected session to Main', async ({ page }, { project }) => {
	let route: WebSocketRoute;
	let chatId: string;
	const finished: Item = { kind: 'subagent', name: 'Finished', task: 'Old task', status: 'done', items: [] };
	const active: Item = { kind: 'subagent', name: 'Explore', task: 'Find the file', status: 'working', items: [
		{ kind: 'text', text: 'Reading the files.', phase: 'commentary', streaming: false }
	] };
	const waiting: Item = { kind: 'subagent', name: 'Review', task: 'Review the file', status: 'waiting', items: [] };
	const items: Item[] = [finished, active, waiting];
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') { route = socket; chatId = message.chat.id; }
			socket.send(message.type === 'chat.items' && message.chatId === chatId ? JSON.stringify({ ...message, items }) : data);
		});
	});
	const name = `subagent-archive-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const picker = page.getByRole('combobox', { name: 'Session in Fake Claude' });
	const panel = page.locator('.panel[role="tabpanel"]:visible');
	await expect(picker.getByRole('option')).toHaveText(['Main · done', '↳ Explore · working', '↳ Review · waiting']);
	await picker.selectOption({ label: '↳ Explore · working' });
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(panel.getByRole('textbox', { name: 'Composer' })).toHaveCount(0);
	await expect.poll(() => picker.evaluate((element) => {
		const parent = element.parentElement!.getBoundingClientRect();
		const bounds = element.getBoundingClientRect();
		return bounds.left >= parent.left && bounds.right <= parent.right && bounds.bottom <= parent.bottom;
	})).toBe(true);
	items[1] = { ...active, status: 'done' };
	route!.send(JSON.stringify({ type: 'chat.item', chatId: chatId!, index: 1, item: items[1] }));
	await expect(picker.getByRole('option')).toHaveText(['Main · done', '↳ Review · waiting']);
	await expect(picker).toHaveValue('');
	await expect(panel.getByRole('textbox', { name: 'Composer' })).toBeVisible();
	await picker.selectOption({ label: '↳ Review · waiting' });
	items[2] = { ...waiting, status: 'done' };
	route!.send(JSON.stringify({ type: 'chat.item', chatId: chatId!, index: 2, item: items[2] }));
	await expect(picker).toHaveCount(0);
	await expect(panel.getByRole('textbox', { name: 'Composer' })).toBeVisible();
	await page.reload();
	await expect(picker).toHaveCount(0);
	await expect(panel.getByRole('textbox', { name: 'Composer' })).toBeVisible();
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
});
