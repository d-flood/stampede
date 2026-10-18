import type { Item, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test, type WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { openProject } from './shell.ts';

test('running shells stay visible across sessions and disappear when finished', async ({ page }, { project }) => {
	let route: WebSocketRoute;
	let chatId: string;
	const command = `pnpm dev --host 0.0.0.0 --port 5173 ${'long-argument-'.repeat(20)}`;
	const shell: Item = { kind: 'command', command, status: 'running', output: 'Server ready\n' };
	const watcher: Item = { kind: 'command', command: 'pnpm test --watch', status: 'running', output: 'Watching files' };
	const child: Item = { kind: 'subagent', id: 'builder', name: 'Builder', task: 'Build the app', status: 'working', items: [watcher] };
	const items: Item[] = [shell, child, { kind: 'text', text: 'The server is running.', phase: 'final', streaming: false }];
	await page.routeWebSocket('**/ws', (socket) => {
		route = socket;
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') chatId = message.chat.id;
			socket.send(message.type === 'chat.items' && message.chatId === chatId ? JSON.stringify({ ...message, items }) : data);
		});
	});
	const name = `shells-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const panel = page.locator('.panel[role="tabpanel"]:visible');
	const shells = panel.getByRole('region', { name: 'Running shells' });
	await expect(shells).toContainText('Running shells · 2');
	await expect(shells.getByText('Builder', { exact: true })).toBeVisible();
	await expect(panel.locator('.calls[open]')).toHaveCount(0);
	await shells.locator('summary').first().click();
	await expect(shells.getByText('Server ready', { exact: true })).toBeVisible();
	await expect.poll(() => shells.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await expect.poll(() => shells.locator('summary').first().evaluate((element) => element.getBoundingClientRect().height)).toBeGreaterThanOrEqual(project.name === 'pixel' ? 48 : 26);
	const dock = panel.getByRole('region', { name: 'Subagents' });
	if (project.name === 'pixel') {
		await dock.getByRole('button').click();
		await page.getByRole('dialog', { name: 'Subagents' }).getByRole('button', { name: /^Build the app/ }).click();
	} else await dock.getByRole('button', { name: /^Build the app/ }).click();
	await expect(shells.getByText('Server ready', { exact: true })).toBeVisible();
	await page.screenshot({ path: test.info().outputPath('running-shells.png') });
	await page.reload();
	await expect(shells).toContainText('Running shells · 2');
	items[0] = { ...shell, status: 'ok' };
	route!.send(JSON.stringify({ type: 'chat.item', chatId: chatId!, index: 0, item: items[0] }));
	await expect(shells).toContainText('Running shells · 1');
	items[1] = { ...child, status: 'done' };
	route!.send(JSON.stringify({ type: 'chat.item', chatId: chatId!, index: 1, item: items[1] }));
	await expect(shells).toContainText('pnpm test --watch');
	items[1] = { ...child, status: 'done', items: [{ ...watcher, status: 'failed' }] };
	route!.send(JSON.stringify({ type: 'chat.item', chatId: chatId!, index: 1, item: items[1] }));
	await expect(shells).toHaveCount(0);
	await panel.locator('.calls > summary').click();
	await expect(panel.locator('.transcript .command')).toContainText(command);
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
});
