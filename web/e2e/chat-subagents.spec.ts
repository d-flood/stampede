import type { Item, ServerMessage, Subagent } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import type { WebSocketRoute } from '@playwright/test';
import { existsSync, mkdirSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { openProject } from './shell.ts';

for (const agent of ['claude', 'codex']) {
	test(`${agent} subagents expose live tools, thinking and output through the server`, async ({ page }, { project }) => {
		const root = process.env.STAMPEDE_E2E_ROOT!;
		const gates = `${root}/activity-${agent}-${project.name}`;
		const kind = agent === 'claude' ? 'claude-code' : 'codex';
		const recording = readFileSync(fileURLToPath(new URL(`../../crates/stampede/tests/fixtures/chats/${kind}/subagents.recorded.jsonl`, import.meta.url)), 'utf8');
		const rows = recording.trim().split('\n').map((line) => JSON.parse(line));
		const main = rows.find((row) => row.stdin?.method === 'turn/start')?.stdin.params.threadId;
		let inserted = false;
		const entries = rows.flatMap((row) => {
			if (row.gate) return [];
			const message = row.stdout;
			const nested = agent === 'claude'
				? message?.type === 'assistant' && message.parent_tool_use_id
				: message?.method === 'item/started' && message.params.threadId !== main && message.params.item.type === 'commandExecution';
			if (inserted || !nested) return [row];
			inserted = true;
			const threadId = message.params?.threadId;
			const parent = message.parent_tool_use_id;
			const stream = (event: object) => ({ stdout: { type: 'stream_event', parent_tool_use_id: parent, event } });
			const event = (method: string, params: object) => ({ stdout: { method, params: { threadId, ...params } } });
			const initial = agent === 'claude' ? [
				stream({ type: 'message_start', message: { id: 'live-reply' } }),
				stream({ type: 'content_block_start', index: 0, content_block: { type: 'thinking', thinking: '' } }),
				stream({ type: 'content_block_delta', index: 0, delta: { type: 'thinking_delta', thinking: 'Checking the relevant code.' } }),
				stream({ type: 'content_block_stop', index: 0 }),
				stream({ type: 'content_block_start', index: 1, content_block: { type: 'text', text: '' } }),
				stream({ type: 'content_block_delta', index: 1, delta: { type: 'text_delta', text: 'First finding' } })
			] : [
				event('item/reasoning/summaryTextDelta', { itemId: 'live-thought', summaryIndex: 0, delta: 'Checking the relevant code.' }),
				event('item/started', { item: { type: 'agentMessage', id: 'live-output', text: '', phase: 'final_answer' } }),
				event('item/agentMessage/delta', { itemId: 'live-output', delta: 'First finding' })
			];
			const continued = agent === 'claude'
				? stream({ type: 'content_block_delta', index: 1, delta: { type: 'text_delta', text: ': missing validation.' } })
				: event('item/agentMessage/delta', { itemId: 'live-output', delta: ': missing validation.' });
			const finished = agent === 'claude' ? [
				stream({ type: 'content_block_stop', index: 1 }),
				{ stdout: { type: 'assistant', parent_tool_use_id: parent, message: { id: 'live-reply', content: [
					{ type: 'thinking', thinking: 'Checking the relevant code.' },
					{ type: 'text', text: 'First finding: missing validation.' }
				] } } }
			] : [
				event('item/completed', { item: { type: 'reasoning', id: 'live-thought', summary: ['Checking the relevant code.'], content: [] } }),
				event('item/completed', { item: { type: 'agentMessage', id: 'live-output', text: 'First finding: missing validation.', phase: 'final_answer' } })
			];
			return [row, ...initial, { gate: 'stream' }, continued, { gate: 'continued' }, ...finished];
		});
		expect(inserted).toBe(true);
		writeFileSync(`${gates}/tools.jsonl`, entries.map((entry) => JSON.stringify(entry)).join('\n') + '\n');
		const name = `live-subagent-${agent}-${project.name}`;
		mkdirSync(`${root}/projects/${name}`);
		await openProject(page, name);
		await page.getByRole('button', { name: 'New Tab' }).click();
		await page.getByRole('menuitem', { name: `Activity ${agent} ${project.name}`, exact: true }).click();
		const panel = page.locator('.panel[role="tabpanel"]:visible');
		await panel.getByRole('textbox', { name: 'Composer' }).fill('Find needle using a subagent and track the steps.');
		await panel.getByRole('button', { name: 'Send', exact: true }).click();
		await expect.poll(() => existsSync(`${gates}/stream`)).toBe(true);
		await panel.getByRole('button', { name: /^1 Subagent · 1 running/ }).click();
		await expect(panel.locator('header').getByTitle('Spawned by')).toHaveText('Harness');
		await expect(panel.getByText('First finding', { exact: true })).toBeVisible();
		const calls = panel.locator('.transcript:visible details.calls');
		await expect(calls).toHaveJSProperty('open', true);
		await expect(calls).toContainText(agent === 'claude' ? 'Read notes.txt' : 'cat notes.txt todo.txt');
		await expect(panel.getByText('Checking the relevant code.', { exact: true })).toBeVisible();
		unlinkSync(`${gates}/stream`);
		await expect.poll(() => existsSync(`${gates}/continued`)).toBe(true);
		await expect(panel.getByText('First finding: missing validation.', { exact: true })).toBeVisible();
		await expect(panel.getByRole('region', { name: 'Report' })).toHaveCount(0);
		await expect(panel.getByRole('status', { name: 'Current activity' })).toBeVisible();
		await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
		unlinkSync(`${gates}/continued`);
	});
}

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
	const active: Subagent = { kind: 'subagent', id: 'active', origin: 'stampede', name: 'Reviewer', task: 'Find the file', status: 'working', startedAt: now - 65_000, activity: 'Reading codex.rs', items: [text('Reading the files.'), nested] };
	const waiting: Subagent = { kind: 'subagent', id: 'waiting', name: 'Review', task: 'Review the file', status: 'waiting', startedAt: now, items: [] };
	let items: Item[] = [];
	const phone = project.name === 'pixel';
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
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
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
	const chip = (finished: number, summary: string) => dock.getByRole('button', { name: phone ? summary : `${finished} finished`, exact: true });
	const sheet = page.getByRole('dialog', { name: 'Subagents' });
	const section = (name: string) => sheet.getByRole('region', { name }).getByRole('listitem');
	const rows = phone ? page.getByRole('dialog').getByRole('listitem') : page.getByRole('menuitem');
	const choose = async (task: string) => {
		if (!phone) return pill(task).click();
		await dock.getByRole('button').click();
		await sheet.getByRole('button', { name: new RegExp(`^${task}`) }).click();
		await expect(sheet).toHaveCount(0);
	};
	if (phone) {
		await expect(dock.getByRole('button')).toHaveCount(1);
		await expect(chip(2, '2 running · 1 waiting · 2 finished')).toBeVisible();
	} else {
		await expect(pill('Find the file')).toBeVisible();
		await expect(pill('Search deeper')).toBeVisible();
		await expect(pill('Review the file')).toBeVisible();
		await expect(dock.getByRole('button')).toHaveCount(4);
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
	}
	await expect(page.getByRole('combobox', { name: /^Session in/ })).toHaveCount(0);
	await expect(tab.getByTitle('working')).toBeVisible();

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
	const spawned = phone ? page.getByRole('dialog', { name: '3 Subagents · 2 running' }).getByRole('listitem') : rows;
	await expect(spawned).toHaveCount(3);
	await expect(spawned.nth(0)).toContainText('Old task');
	await expect(spawned.nth(1)).toContainText('Find the file');
	await expect(spawned.nth(2)).toContainText('Review the file');
	await spawned.nth(1).click();
	await expect(rows).toHaveCount(0);
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(header.getByTitle('Spawned by')).toHaveText('Stampede delegate');
	await expect(composer).toHaveCount(0);
	items[2] = { ...active, activity: 'Running cargo test', items: [
		text('Reading the files.'),
		{ kind: 'thinking', seconds: 0, text: 'Checking the relevant code.' },
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'codex.rs' }] },
		{ kind: 'command', command: 'cargo test', status: 'running', output: 'running 3 tests' },
		{ kind: 'text', text: 'First finding', phase: 'final', streaming: true },
		nested
	] };
	send({ type: 'chat.item', chatId: chatId!, index: 2, item: items[2] });
	await expect(panel.getByText('First finding', { exact: true })).toBeVisible();
	const calls = panel.locator('.transcript:visible details.calls');
	await expect(calls).toHaveJSProperty('open', true);
	await expect(calls).toContainText('Read codex.rs');
	await expect(panel.getByText('Checking the relevant code.', { exact: true })).toBeVisible();
	await calls.locator('details.running > summary').click();
	await expect(calls.locator('pre')).toHaveText('running 3 tests');
	(items[2] as Subagent).items[4] = { kind: 'text', text: 'First finding: missing validation.', phase: 'final', streaming: true };
	send({ type: 'chat.item', chatId: chatId!, index: 2, item: items[2] });
	await expect(panel.getByText('First finding: missing validation.', { exact: true })).toBeVisible();
	await expect(report).toHaveCount(0);
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await marker('1 Subagent · 1 running').click();
	await expect(rows).toHaveCount(0);
	await expect(panel.getByText('Deep work.', { exact: true })).toBeVisible();
	await panel.getByRole('button', { name: 'Back to Main' }).click();
	await marker('1 Subagent · done').click();
	await expect(rows).toHaveCount(0);
	await expect(breadcrumb).toHaveText(/^Main\s*\/\s*Broken task$/);
	await expect(composer).toHaveCount(0);
	await panel.getByRole('button', { name: 'Back to Main' }).click();

	await chip(2, '2 running · 1 waiting · 2 finished').click();
	const done = phone ? section('Finished') : rows;
	if (phone) {
		await expect(sheet.getByRole('heading', { level: 3 })).toHaveText(['Running', 'Finished']);
		await expect(section('Running')).toHaveText([/^\s*Find the file/, /^\s*Search deeper/, /^\s*Review the file/]);
	}
	await expect(done).toHaveCount(2);
	await expect(done.nth(0)).toHaveText(/^\s*Broken task\s*Harness\s*general-purpose\s*failed\s*·\s*5s · 1 tool$/);
	await expect(done.nth(1)).toHaveText(/^\s*Old task\s*Harness\s*Explore\s*1m 12s · 14 tools · 38k tokens$/);
	await done.nth(1).click();
	await expect(rows).toHaveCount(0);
	await expect(panel.getByText('Old work.', { exact: true })).toBeVisible();
	await expect(composer).toHaveCount(0);
	await expect(dock).toBeVisible();
	await expect(panel.getByText('Subagent', { exact: true })).toBeVisible();
	await expect(breadcrumb).toHaveText(/^Main\s*\/\s*Old task$/);
	await expect(header.getByTitle('Type')).toHaveText('Explore');
	await expect(header.getByTitle('Spawned by')).toHaveText('Harness');
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
	await choose('Review the file');
	await expect(composer).toBeVisible();
	await expect(breadcrumb).toHaveCount(0);
	await expect(approval).toBeFocused();
	send({ type: 'chat.resolved', chatId: chatId!, requestId: 'review' });
	await expect(approval).toHaveCount(0);

	await choose('Search deeper');
	await expect(panel.getByText('Deep work.', { exact: true })).toBeVisible();
	await expect(breadcrumb).toHaveText(/^Main\s*\/\s*Find the file\s*\/\s*Search deeper$/);
	items[2] = { ...active, items: [text('Reading the files.'), { ...nested, status: 'done', outcome: 'completed' }] };
	send({ type: 'chat.item', chatId: chatId!, index: 2, item: items[2] });
	await expect(chip(3, '1 running · 1 waiting · 3 finished')).toBeVisible();
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
	await expect(chip(4, '1 waiting · 4 finished')).toBeVisible();
	await expect(pill('Find the file')).toHaveCount(0);
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(report.locator('code')).toHaveText('codex.rs');
	await expect(header).toContainText('1m 20s · 3 tools');
	await expect(composer).toHaveCount(0);
	await panel.getByRole('button', { name: 'Back to Main' }).click();
	await expect(composer).toBeVisible();
	await expect(spawn).toHaveText('3 Subagents · 1 running');

	await page.reload();
	await expect(phone ? chip(4, '1 waiting · 4 finished') : pill('Review the file')).toBeVisible();
	await chip(4, '1 waiting · 4 finished').click();
	await done.filter({ hasText: 'Find the file' }).click();
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(composer).toHaveCount(0);
	await breadcrumb.getByRole('button', { name: 'Main', exact: true }).click();
	await expect(composer).toBeVisible();
	items[3] = { ...waiting, status: 'done', outcome: 'stopped' };
	send({ type: 'chat.item', chatId: chatId!, index: 3, item: items[3] });
	await expect(dock.getByRole('button')).toHaveCount(1);
	await expect(chip(5, '5 finished')).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();

	const many = Array.from({ length: 12 }, (_, index): Subagent => ({ kind: 'subagent', id: `many-${index}`, name: 'Explore', task: `Task ${index}`, status: 'working', startedAt: now, activity: `Reading file-${index}.rs`, items: [] }));
	items = [...items, ...many];
	send({ type: 'chat.items', chatId: chatId!, items });
	if (phone) await chip(5, '12 running · 5 finished').click();
	else {
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
	}
	const running = phone ? section('Running') : rows;
	await expect(running).toHaveCount(12);
	await expect(running.nth(11)).toContainText('Reading file-11.rs');
	await running.nth(11).click();
	await expect(breadcrumb).toContainText('Task 11');
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
});
