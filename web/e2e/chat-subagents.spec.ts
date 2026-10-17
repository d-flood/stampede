import type { Item, ServerMessage, Subagent } from '#lib/device.svelte.ts';
import { expect, test, type WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { openProject } from './shell.ts';

test('the dock shows running subagents as pills and keeps finished ones reachable', async ({ page }, { project }) => {
	let route: WebSocketRoute;
	let chatId: string;
	const now = Date.now();
	const text = (text: string): Item => ({ kind: 'text', text, phase: 'commentary', streaming: false });
	const command = (command: string): Item => ({ kind: 'command', command, status: 'ok', output: '' });
	const old: Subagent = {
		kind: 'subagent',
		id: 'old',
		name: 'Explore',
		model: 'claude-haiku-4-5',
		task: 'Old task',
		status: 'done',
		outcome: 'completed',
		prompt: 'Find the old file.\nSearch every directory.\nReport its path and line count.',
		report: 'Found **notes.txt** with 3 lines.',
		startedAt: now - 600_000,
		totals: { durationMs: 72_000, tools: 14, tokens: 38_000 },
		items: [text('Old work.'), { kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'notes.txt' }] }]
	};
	const failed: Subagent = { kind: 'subagent', id: 'failed', name: 'general-purpose', task: 'Broken task', status: 'done', outcome: 'failed', startedAt: now - 300_000, totals: { durationMs: 5_000, tools: 1 }, items: [] };
	const nested: Subagent = { kind: 'subagent', id: 'nested', name: 'Explore', task: 'Search deeper', status: 'working', startedAt: now, items: [text('Deep work.')] };
	const active: Subagent = { kind: 'subagent', id: 'active', name: 'Explore', task: 'Find the file', status: 'working', startedAt: now - 65_000, activity: 'Reading codex.rs', items: [text('Reading the files.'), nested] };
	const waiting: Subagent = { kind: 'subagent', id: 'waiting', name: 'Review', task: 'Review the file', status: 'waiting', startedAt: now, items: [] };
	let items: Item[] = [];
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
	const send = (message: ServerMessage) => route.send(JSON.stringify(message));
	const name = `subagent-dock-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude/ });
	const panel = page.locator('.panel[role="tabpanel"]:visible');
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	const dock = panel.getByRole('region', { name: 'Subagents' });
	const header = panel.locator('header');
	const breadcrumb = header.getByRole('navigation', { name: 'Breadcrumb' });
	const prompt = panel.getByRole('region', { name: 'Prompt' });
	const report = panel.getByRole('region', { name: 'Report' });
	await expect(composer).toBeVisible();
	await expect(dock).toHaveCount(0);

	items = [command('ls'), old, active, waiting, command('pwd'), failed];
	send({ type: 'chat.items', chatId: chatId!, items });
	const pill = (task: string) => dock.getByRole('button', { name: new RegExp(`^${task}`) });
	await expect(pill('Find the file')).toBeVisible();
	await expect(pill('Search deeper')).toBeVisible();
	await expect(pill('Review the file')).toBeVisible();
	await expect(dock.getByRole('button')).toHaveCount(4);
	await expect(page.getByRole('combobox', { name: /^Session in/ })).toHaveCount(0);
	await expect(tab.getByTitle('working')).toBeVisible();
	await expect.poll(() => pill('Review the file').evaluate((element) => {
		const probe = element.appendChild(document.createElement('span'));
		probe.style.color = 'var(--status-waiting)';
		const yellow = getComputedStyle(probe).color;
		probe.remove();
		return getComputedStyle(element).borderTopColor === yellow && getComputedStyle(element.querySelector('.mark')!).color === yellow;
	})).toBe(true);
	await expect(pill('Find the file')).toHaveText(/1:\d\d$/);
	const clock = await pill('Find the file').textContent();
	await expect.poll(() => pill('Find the file').textContent()).not.toBe(clock);

	const marker = (text: string) => panel.getByRole('button', { name: new RegExp(`^${text}`) });
	const spawn = marker('3 Subagents');
	await expect(panel.locator('details.calls, button.spawn')).toHaveText([/^1 tool call/, '3 Subagents · 2 running', /^1 tool call/, '1 Subagent · done']);
	const group = panel.locator('details.calls').first();
	await group.locator('> summary').click();
	await expect(group).toHaveJSProperty('open', true);
	await group.locator('> summary').click();
	await expect(group).toHaveJSProperty('open', false);
	await expect(spawn).toBeVisible();
	await spawn.click();
	const rows = page.getByRole('menuitem');
	await expect(rows).toHaveCount(3);
	await expect(rows.nth(0)).toContainText('Old task');
	await expect(rows.nth(1)).toContainText('Find the file');
	await expect(rows.nth(2)).toContainText('Review the file');
	await rows.nth(1).click();
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(composer).toHaveCount(0);
	await marker('1 Subagent · 1 running').click();
	await expect(rows).toHaveCount(0);
	await expect(panel.getByText('Deep work.', { exact: true })).toBeVisible();
	await panel.getByRole('button', { name: 'Back to Main' }).click();
	await marker('1 Subagent · done').click();
	await expect(rows).toHaveCount(0);
	await expect(breadcrumb).toHaveText(/^Main\s*\/\s*Broken task$/);
	await expect(composer).toHaveCount(0);
	await panel.getByRole('button', { name: 'Back to Main' }).click();

	await dock.getByRole('button', { name: '2 finished', exact: true }).click();
	await expect(rows).toHaveCount(2);
	await expect(rows.nth(0)).toHaveText(/^\s*Broken task\s*general-purpose\s*failed\s*·\s*5s · 1 tool$/);
	await expect(rows.nth(1)).toHaveText(/^\s*Old task\s*Explore\s*1m 12s · 14 tools · 38k tokens$/);
	await rows.nth(1).click();
	await expect(panel.getByText('Old work.', { exact: true })).toBeVisible();
	await expect(composer).toHaveCount(0);
	await expect(dock).toBeVisible();
	await expect(panel.getByText('Subagent', { exact: true })).toBeVisible();
	await expect(breadcrumb).toHaveText(/^Main\s*\/\s*Old task$/);
	await expect(header.getByTitle('Type')).toHaveText('Explore');
	await expect(header.getByTitle('Model')).toHaveText('claude-haiku-4-5');
	await expect(header).toContainText('1m 12s · 14 tools · 38k tokens');
	const lines = () => prompt.getByText(/^Find the old file/).evaluate((element) => Math.round(element.clientHeight / parseFloat(getComputedStyle(element).lineHeight)));
	await expect.poll(lines).toBe(2);
	await prompt.getByRole('button', { name: 'Show more' }).click();
	await expect.poll(lines).toBe(3);
	await expect(panel.getByText('1 tool call', { exact: true }).filter({ visible: true })).toBeVisible();
	await expect(report).toHaveText(/^\s*Report\s*Found notes.txt with 3 lines.\s*$/);
	await expect(report.locator('strong')).toHaveText('notes.txt');
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);

	send({ type: 'chat.request', chatId: chatId!, request: { kind: 'command', id: 'review', words: [['touch', 'reviewed.txt']], suggestion: [], sessionPrefixes: true, subagent: 'waiting' } });
	const approval = panel.getByRole('region', { name: 'Approval' });
	await expect(approval).toContainText('Asked by Review the file');
	await pill('Review the file').click();
	await expect(composer).toBeVisible();
	await expect(breadcrumb).toHaveCount(0);
	await expect(approval).toBeFocused();
	send({ type: 'chat.resolved', chatId: chatId!, requestId: 'review' });
	await expect(approval).toHaveCount(0);

	await pill('Search deeper').click();
	await expect(panel.getByText('Deep work.', { exact: true })).toBeVisible();
	await expect(breadcrumb).toHaveText(/^Main\s*\/\s*Find the file\s*\/\s*Search deeper$/);
	items[2] = { ...active, items: [text('Reading the files.'), { ...nested, status: 'done', outcome: 'completed' }] };
	send({ type: 'chat.item', chatId: chatId!, index: 2, item: items[2] });
	await expect(dock.getByRole('button', { name: '3 finished', exact: true })).toBeVisible();
	await expect(pill('Search deeper')).toHaveCount(0);
	await expect(panel.getByText('Deep work.', { exact: true })).toBeVisible();
	await expect(composer).toHaveCount(0);

	await breadcrumb.getByRole('button', { name: 'Find the file', exact: true }).click();
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(breadcrumb).toHaveText(/^Main\s*\/\s*Find the file$/);
	await expect(header).toContainText(/1:\d\d/);
	await expect(report).toHaveCount(0);
	items[2] = { ...items[2], status: 'done', outcome: 'completed', activity: null, report: 'The file is `codex.rs`.', totals: { durationMs: 80_000, tools: 3 } } as Subagent;
	send({ type: 'chat.item', chatId: chatId!, index: 2, item: items[2] });
	await expect(dock.getByRole('button', { name: '4 finished', exact: true })).toBeVisible();
	await expect(pill('Find the file')).toHaveCount(0);
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(report.locator('code')).toHaveText('codex.rs');
	await expect(header).toContainText('1m 20s · 3 tools');
	await expect(composer).toHaveCount(0);
	await panel.getByRole('button', { name: 'Back to Main' }).click();
	await expect(composer).toBeVisible();
	await expect(spawn).toHaveText('3 Subagents · 1 running');

	await page.reload();
	await expect(pill('Review the file')).toBeVisible();
	await dock.getByRole('button', { name: '4 finished', exact: true }).click();
	await page.getByRole('menuitem', { name: /^Find the file/ }).click();
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(composer).toHaveCount(0);
	await breadcrumb.getByRole('button', { name: 'Main', exact: true }).click();
	await expect(composer).toBeVisible();
	items[3] = { ...waiting, status: 'done', outcome: 'stopped' };
	send({ type: 'chat.item', chatId: chatId!, index: 3, item: items[3] });
	await expect(dock.getByRole('button')).toHaveCount(1);
	await expect(dock.getByRole('button', { name: '5 finished', exact: true })).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();

	const many = Array.from({ length: 12 }, (_, index): Subagent => ({ kind: 'subagent', id: `many-${index}`, name: 'Explore', task: `Task ${index}`, status: 'working', startedAt: now, activity: `Reading file-${index}.rs`, items: [] }));
	items = [...items, ...many];
	send({ type: 'chat.items', chatId: chatId!, items });
	const more = dock.getByRole('button', { name: /^\+\d+$/ });
	await expect(more).toBeVisible();
	const hidden = Number((await more.textContent())!.slice(1));
	await expect(dock.getByRole('button', { name: /^Task \d+/ })).toHaveCount(12 - hidden);
	await expect.poll(() => dock.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		return [...element.querySelectorAll('button')].every((button) => {
			const box = button.getBoundingClientRect();
			return box.right <= bounds.right && box.bottom <= bounds.bottom;
		});
	})).toBe(true);
	await more.click();
	await expect(rows).toHaveCount(12);
	await expect(rows.nth(11)).toContainText('Reading file-11.rs');
	await rows.nth(11).click();
	await expect(breadcrumb).toContainText('Task 11');
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
});
