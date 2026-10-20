import type { ClientMessage, Item, ServerMessage, Subagent } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import type { WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { openProject } from './shell.ts';

test('a finished subagent and a long command output arrive in brief and load when opened', async ({ page }, { project }) => {
	let route: WebSocketRoute;
	let chatId: string;
	const expanded: number[] = [];
	const output = 'a line of build output\n'.repeat(400);
	const subagent = { kind: 'subagent', id: 'finished', name: 'Explore', task: 'Old task', status: 'done', outcome: 'completed', report: 'Done.', startedAt: Date.now() - 60_000, totals: { durationMs: 1_000, tools: 1 } } as const;
	const items: Item[] = [
		{ kind: 'command', command: 'cat build.log', status: 'ok', output: '', elided: true },
		{ ...subagent, items: [], elided: true }
	];
	const full: Item[] = [
		{ kind: 'command', command: 'cat build.log', status: 'ok', output },
		{ ...subagent, items: [{ kind: 'text', text: 'Inner work.', phase: 'commentary', streaming: false }] } satisfies Subagent
	];
	await page.routeWebSocket('**/ws', (socket) => {
		route = socket;
		const server = socket.connectToServer();
		socket.onMessage((data) => {
			const message: ClientMessage | undefined = typeof data === 'string' ? JSON.parse(data) : undefined;
			if (message?.type !== 'chat.expand' || message.chatId !== chatId) return server.send(data);
			expanded.push(message.index);
			socket.send(JSON.stringify({ type: 'chat.item', chatId, index: message.index, item: full[message.index] }));
		});
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') chatId = message.chat.id;
			socket.send(message.type === 'chat.items' && message.chatId === chatId ? JSON.stringify({ ...message, items }) : data);
		});
	});
	const name = `chat-elided-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const panel = page.locator('.panel[role="tabpanel"]:visible');
	await expect(panel.getByRole('textbox', { name: 'Composer' })).toBeVisible();
	route!.send(JSON.stringify({ type: 'chat.items', chatId: chatId!, items } satisfies ServerMessage));

	await panel.locator('details.calls > summary').click();
	const command = panel.locator('details.ok');
	await expect(command.locator('pre')).toHaveCount(0);
	await command.locator('summary').click();
	await expect(command.locator('pre')).toContainText('a line of build output');
	expect(expanded).toEqual([0]);

	await panel.getByRole('button', { name: /^1 Subagent · done/ }).click();
	await expect(panel.getByText('Inner work.', { exact: true })).toBeVisible();
	expect(expanded).toEqual([0, 1]);
});
