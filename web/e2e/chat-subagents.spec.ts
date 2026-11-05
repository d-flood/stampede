import type { Item, ServerMessage, Subagent } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import type { WebSocketRoute } from '@playwright/test';
import { existsSync, mkdirSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { openProject } from './shell.ts';

for (const agent of ['claude', 'codex']) {
	test(`${agent} subagents expose live tools, thinking and output through the server`, { tag: '@desktop' }, async ({ page }, { project }) => {
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
		await panel.locator('button.spawn').click();
		await expect(panel.locator('header').getByTitle('Name')).toHaveText(agent === 'claude' ? 'Explore' : 'Heisenberg');
		await expect(panel.getByText('First finding', { exact: true })).toBeVisible();
		const calls = panel.locator('.transcript:visible .calls');
		await expect(calls.locator('> .summary')).toHaveAttribute('aria-expanded', 'true');
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

test('the Tray counts Subagents with colored dots, surfaces waiting ones and keeps every one reachable', async ({ page }, { project }) => {
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
	const failed: Subagent = { kind: 'subagent', id: 'failed', name: 'Plan', task: 'Broken task', status: 'done', outcome: 'failed', startedAt: now - 300_000, totals: { durationMs: 5_000, tools: 1 }, items: [] };
	const nested: Subagent = { kind: 'subagent', id: 'nested', name: 'Explore', task: 'Search deeper', status: 'working', startedAt: now, items: [text('Deep work.')] };
	const active: Subagent = { kind: 'subagent', id: 'active', origin: 'stampede', name: 'Reviewer', agent: 'codex', task: 'Find the file', status: 'working', startedAt: now - 65_000, activity: 'Reading codex.rs', items: [text('Reading the files.'), nested] };
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
	const name = `subagent-tray-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude/ });
	const panel = page.locator('.panel[role="tabpanel"]:visible');
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	const subagents = panel.getByRole('region', { name: 'Subagents' });
	const tray = panel.getByRole('region', { name: 'Tray' });
	const header = panel.locator('header');
	const breadcrumb = header.getByRole('navigation', { name: 'Breadcrumb' });
	const prompt = panel.getByRole('region', { name: 'Prompt' });
	const report = panel.getByRole('region', { name: 'Report' });
	await expect(composer).toBeVisible();
	await expect(subagents).toHaveCount(0);

	items = [command('ls'), old, active, waiting, command('pwd'), failed];
	send({ type: 'chat.items', chatId: chatId!, items });
	const chip = (summary: string) => subagents.getByRole('button', { name: `Subagents: ${summary}`, exact: true });
	const attention = subagents.getByRole('button', { name: /^\d+ waiting$/ });
	const sheet = page.getByRole('dialog', { name: 'Subagents' });
	const section = (name: string) => sheet.getByRole('region', { name }).getByRole('listitem');
	const menuGroup = (name: string) => page.getByRole('menu').getByRole('group').filter({ has: page.getByText(name, { exact: true }) }).getByRole('menuitem');
	const rows = phone ? page.getByRole('dialog').getByRole('listitem') : page.getByRole('menuitem');
	const choose = async (task: string) => {
		await subagents.getByRole('button', { name: /^Subagents/ }).click();
		await (phone ? sheet.getByRole('button', { name: new RegExp(`^${task}`) }) : page.getByRole('menuitem', { name: new RegExp(`^${task}`) })).click();
		await expect(rows).toHaveCount(0);
	};
	const oneRow = () => tray.evaluate((element) => {
		const boxes = [...element.children].map((child) => child.getBoundingClientRect());
		return Math.max(...boxes.map(({ top }) => top)) < Math.min(...boxes.map(({ bottom }) => bottom));
	});
	await expect(chip('2 running, 2 done')).toBeVisible();
	await expect(subagents.locator('.dot')).toHaveCount(3);
	await expect(chip('2 running, 2 done')).toContainText('+2');
	await expect(attention).toHaveText('1 waiting');
	await expect.poll(() => attention.evaluate((element) => {
		const probe = element.appendChild(document.createElement('span'));
		probe.style.color = 'var(--status-waiting)';
		const yellow = getComputedStyle(probe).color;
		probe.remove();
		return getComputedStyle(element).color === yellow;
	})).toBe(true);
	await expect(subagents.getByRole('button')).toHaveCount(2);
	await expect.poll(oneRow).toBe(true);
	await expect(page.getByRole('combobox', { name: /^Session in/ })).toHaveCount(0);
	await expect(tab.getByTitle('working')).toBeVisible();

	const line = (task: string) => panel.locator('button.spawn:visible', { hasText: task });
	await expect(panel.locator('.calls > .summary, button.spawn')).toHaveText(['Ran 1 command', /^\s*Explore\s*Old task\s*finished in 1m 12s, 14 tools$/, /^\s*Reviewer\s*Find the file\s*1:\d\d$/, /^\s*Review\s*Review the file\s*waiting for you$/, 'Ran 1 command', /^\s*Plan\s*Broken task\s*failed$/]);
	await expect(line('Find the file').getByRole('img', { name: 'Codex' })).toBeVisible();
	const group = panel.locator('.calls > .summary').first();
	await group.click();
	await expect(group).toHaveAttribute('aria-expanded', 'true');
	await group.click();
	await expect(group).toHaveAttribute('aria-expanded', 'false');
	await line('Find the file').click();
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(header.getByRole('img', { name: 'Codex' })).toBeVisible();
	await expect(header.getByTitle('Name')).toHaveText('Reviewer');
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
	const calls = panel.locator('.transcript:visible .calls');
	await expect(calls.locator('> .summary')).toHaveAttribute('aria-expanded', 'true');
	await expect(calls).toContainText('Read codex.rs');
	await expect(panel.getByText('Checking the relevant code.', { exact: true })).toBeVisible();
	await calls.locator('details.running > summary').click();
	await expect(calls.locator('pre')).toHaveText('running 3 tests');
	(items[2] as Subagent).items[4] = { kind: 'text', text: 'First finding: missing validation.', phase: 'final', streaming: true };
	send({ type: 'chat.item', chatId: chatId!, index: 2, item: items[2] });
	await expect(panel.getByText('First finding: missing validation.', { exact: true })).toBeVisible();
	await expect(report).toHaveCount(0);
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await line('Search deeper').click();
	await expect(rows).toHaveCount(0);
	await expect(panel.getByText('Deep work.', { exact: true })).toBeVisible();
	await panel.getByRole('button', { name: 'Back to Main' }).click();
	await line('Broken task').click();
	await expect(rows).toHaveCount(0);
	await expect(breadcrumb).toHaveText(/^Main\s*\/\s*Broken task$/);
	await expect(composer).toHaveCount(0);
	await panel.getByRole('button', { name: 'Back to Main' }).click();

	await chip('2 running, 2 done').click();
	const done = phone ? section('Finished') : menuGroup('Finished');
	if (phone) await expect(sheet.getByRole('heading', { level: 3 })).toHaveText(['Running', 'Finished']);
	else await expect(page.getByRole('menu').getByRole('group')).toHaveText([/^Running/, /^Finished/]);
	await expect(phone ? section('Running') : menuGroup('Running')).toHaveText([/^\s*Find the file/, /^\s*Search deeper/, /^\s*Review the file/]);
	await expect(done).toHaveCount(2);
	await expect(done.nth(0)).toHaveText(/^\s*Broken task\s*Plan\s*failed\s*·\s*5s · 1 tool$/);
	await expect(done.nth(1)).toHaveText(/^\s*Old task\s*Explore\s*1m 12s · 14 tools · 38k tokens$/);
	await done.nth(1).click();
	await expect(rows).toHaveCount(0);
	await expect(panel.getByText('Old work.', { exact: true })).toBeVisible();
	await expect(composer).toHaveCount(0);
	await expect(subagents).toBeVisible();
	await expect(panel.getByText('Subagent', { exact: true })).toBeVisible();
	await expect(breadcrumb).toHaveText(/^Main\s*\/\s*Old task$/);
	await expect(header.getByTitle('Name')).toHaveText('Explore');
	await expect(header.getByTitle('Model')).toHaveText('claude-haiku-4-5');
	await expect(header).toContainText('1m 12s · 14 tools · 38k tokens');
	const lines = () => prompt.getByText(/^Find the old file/).evaluate((element) => Math.round(element.clientHeight / parseFloat(getComputedStyle(element).lineHeight)));
	await expect.poll(lines).toBe(2);
	await prompt.getByRole('button', { name: 'Show more' }).click();
	await expect.poll(lines).toBe(3);
	await expect(panel.getByText('Read 1 file', { exact: true }).filter({ visible: true })).toBeVisible();
	await expect(report).toHaveText(/^\s*Report\s*Found notes.txt with 3 lines.\s*$/);
	await expect(report.locator('strong')).toHaveText('notes.txt');
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);

	send({ type: 'chat.request', chatId: chatId!, request: { kind: 'command', id: 'review', words: [['touch', 'reviewed.txt']], suggestion: [], sessionPrefixes: true, subagent: 'waiting' } });
	const approval = panel.getByRole('region', { name: 'Approval' });
	await expect(approval).toContainText('Asked by Review the file');
	await attention.click();
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
	await expect(chip('1 running, 3 done')).toBeVisible();
	await expect(attention).toHaveText('1 waiting');
	await expect(panel.getByText('Deep work.', { exact: true })).toBeVisible();
	await expect(composer).toHaveCount(0);

	await breadcrumb.getByRole('button', { name: 'Find the file', exact: true }).click();
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(breadcrumb).toHaveText(/^Main\s*\/\s*Find the file$/);
	await expect(header).toContainText(/1:\d\d/);
	await expect(report).toHaveCount(0);
	items[2] = { ...items[2], status: 'done', outcome: 'completed', activity: null, report: 'The file is `codex.rs`.', totals: { durationMs: 80_000, tools: 3 } } as Subagent;
	send({ type: 'chat.item', chatId: chatId!, index: 2, item: items[2] });
	await expect(chip('4 done')).toBeVisible();
	await expect(attention).toHaveText('1 waiting');
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(report.locator('code')).toHaveText('codex.rs');
	await expect(header).toContainText('1m 20s · 3 tools');
	await expect(composer).toHaveCount(0);
	await panel.getByRole('button', { name: 'Back to Main' }).click();
	await expect(composer).toBeVisible();
	await expect(line('Find the file')).toHaveText(/finished in 1m 20s, 3 tools$/);

	await page.reload();
	await expect(attention).toHaveText('1 waiting');
	await chip('4 done').click();
	await done.filter({ hasText: 'Find the file' }).click();
	await expect(panel.getByText('Reading the files.', { exact: true })).toBeVisible();
	await expect(composer).toHaveCount(0);
	await breadcrumb.getByRole('button', { name: 'Main', exact: true }).click();
	await expect(composer).toBeVisible();
	items[3] = { ...waiting, status: 'done', outcome: 'stopped' };
	send({ type: 'chat.item', chatId: chatId!, index: 3, item: items[3] });
	await expect(subagents.getByRole('button')).toHaveCount(1);
	await expect(chip('5 done')).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();

	const many = Array.from({ length: 12 }, (_, index): Subagent => ({ kind: 'subagent', id: `many-${index}`, name: 'Explore', task: `Task ${index}`, status: 'working', startedAt: now, activity: `Reading file-${index}.rs`, items: [] }));
	items = [...items, ...many];
	send({ type: 'chat.items', chatId: chatId!, items });
	await expect(chip('12 running, 5 done')).toContainText('+14');
	await expect(subagents.locator('.dot')).toHaveCount(3);
	await expect.poll(oneRow).toBe(true);
	await chip('12 running, 5 done').click();
	const running = phone ? section('Running') : menuGroup('Running');
	await expect(running).toHaveCount(12);
	await expect(running.nth(11)).toContainText('Reading file-11.rs');
	await running.nth(11).click();
	await expect(breadcrumb).toContainText('Task 11');
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
});
