import type { Item, ServerMessage } from '#lib/device.svelte.ts';
import type { WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { openProject } from './shell.ts';

test('hidden Chats defer streamed text rendering and catch up when shown', async ({ page }, { project }) => {
	let route: WebSocketRoute;
	const ids: string[] = [];
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') { route = socket; ids.push(message.chat.id); }
			socket.send(data);
		});
	});
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/chat-rendering-${project.name}`);
	await openProject(page, `chat-rendering-${project.name}`);
	for (let i = 0; i < 2; i++) {
		await page.getByRole('button', { name: 'New Tab' }).click();
		await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
		await expect.poll(() => ids.length).toBe(i + 1);
		await expect(page.getByRole('textbox', { name: 'Composer' })).toBeVisible();
	}
	const tabs = page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab');
	await tabs.first().click();
	const item: Item = { kind: 'text', text: 'Initial reply.', streaming: true, phase: 'commentary' };
	route!.send(JSON.stringify({ type: 'chat.item', chatId: ids[0], index: 0, item }));
	const text = page.locator('.panel[role="tabpanel"]').first().locator('.text');
	await expect(text).toHaveText('Initial reply.');
	await tabs.last().click();
	await expect(text).toBeHidden();
	const mutations = await text.evaluateHandle((node) => {
		const result = { count: 0, observer: new MutationObserver((records) => { result.count += records.length; }) };
		result.observer.observe(node, { subtree: true, childList: true, characterData: true });
		return result;
	});
	for (let i = 0; i < 5; i++) {
		route!.send(JSON.stringify({ type: 'chat.append', chatId: ids[0], index: 0, text: ` Update ${i}.` }));
		await page.getByRole('textbox', { name: 'Composer' }).press('x');
	}
	const finished: Item = { ...item, text: 'Finished reply.', streaming: false };
	route!.send(JSON.stringify({ type: 'chat.item', chatId: ids[0], index: 0, item: finished }));
	await page.getByRole('textbox', { name: 'Composer' }).press('y');
	await expect(page.getByRole('textbox', { name: 'Composer' })).toHaveValue('xxxxxy');
	await text.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
	expect.soft(await mutations.evaluate((result) => { result.observer.disconnect(); return result.count; })).toBe(0);
	await tabs.first().click();
	await expect(page.locator('.panel:visible .text')).toHaveText(finished.text);
});
