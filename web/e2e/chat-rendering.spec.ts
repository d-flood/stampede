import type { Item, ServerMessage } from '#lib/device.svelte.ts';
import type { Locator, Page, WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { openProject } from './shell.ts';

test('hidden Chats defer streamed text rendering and catch up when shown', { tag: '@desktop' }, async ({ page }, { project }) => {
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

async function chatShowing(page: Page, name: string, text: string) {
	let chatId: string;
	const items: Item[] = [{ kind: 'text', text, streaming: false, phase: 'final' }];
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'chat.created') chatId = message.chat.id;
			socket.send(message.type === 'chat.items' && message.chatId === chatId ? JSON.stringify({ ...message, items }) : data);
		});
	});
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	return page.locator('.panel[role="tabpanel"]:visible .text');
}

test('Chat tables keep short words whole and headings step down in size', async ({ page }, { project, repeatEachIndex }) => {
	const prose = 'A long explanation that wraps across several lines in a narrow column. '.repeat(3);
	const text = await chatShowing(page, `chat-markdown-${project.name}-${repeatEachIndex}`, `# One\n\n## Two\n\n### Three\n\nBody\n\n| Agent | Notes |\n| --- | --- |\n| Copilot | ${prose} |`);
	const lines = (locator: Locator) => locator.evaluate((element) => {
		const range = document.createRange();
		range.selectNodeContents(element);
		return range.getClientRects().length;
	});
	await expect(text.getByRole('cell', { name: 'Copilot' })).toBeVisible();
	expect(await lines(text.getByRole('columnheader', { name: 'Agent' }))).toBe(1);
	expect(await lines(text.getByRole('cell', { name: 'Copilot' }))).toBe(1);
	const sizes = await text.locator('h1, h2, h3, p').evaluateAll((elements) => elements.map((element) => parseFloat(getComputedStyle(element).fontSize)));
	expect(sizes).toEqual([...sizes].sort((a, b) => b - a));
	expect(new Set(sizes).size).toBe(4);
});

test('Chat table columns shrink for a few words and stay readable for long text', async ({ page }, { project, repeatEachIndex }) => {
	const row = (cells: string[]) => `| ${cells.join(' | ')} |`;
	const prose = 'A long explanation that wraps across several lines in a narrow column. '.repeat(3);
	const few = Array(5).fill('three short words');
	const text = await chatShowing(page, `chat-table-widths-${project.name}-${repeatEachIndex}`, [
		row(['A', 'B', 'C', 'D', 'E']), row(Array(5).fill('---')), row(few),
		'',
		row(['Agent', 'First', 'Second', 'Third']), row(['---', '---:', '---', '---']), row(['Copilot', prose, prose, prose])
	].join('\n'));
	const [short, long] = [text.locator('table').first(), text.locator('table').last()];
	const overflows = (table: Locator) => table.evaluate((element) => element.scrollWidth > element.clientWidth);
	const width = (cell: Locator) => cell.evaluate((element) => element.getBoundingClientRect().width);
	await expect(long.getByRole('cell', { name: 'Copilot' })).toBeVisible();
	expect(await overflows(short)).toBe(false);
	expect(await overflows(long)).toBe(true);
	const [agent, notes] = [await width(long.getByRole('cell', { name: 'Copilot' })), await width(long.getByRole('cell').nth(1))];
	expect(notes).toBeGreaterThan(200);
	expect(agent).toBeLessThan(notes / 2);
	await expect(long.getByRole('columnheader', { name: 'First' })).toHaveCSS('text-align', 'right');
});
