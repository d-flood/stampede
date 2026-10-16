import type { ServerMessage } from '#lib/device.svelte.ts';
import { expect, test, type Locator } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, unlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { openProject, showSidebar } from './shell.ts';

test('a Claude Code Chat streams its reply, shows done, and asks before closing while working', async ({ page }, { project }) => {
	const name = `chat-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude as Chat' }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	await expect(tab.getByTitle('done')).toBeVisible();

	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const context = panel.getByRole('meter', { name: 'Context' });
	await expect(context).toHaveCount(0);
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('Greet the reader');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.getByText('Greet the reader')).toBeVisible();
	await expect(panel.locator('strong', { hasText: 'welcome' })).toBeVisible();
	await expect(panel.getByText(/^Hello and welcome!/)).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();
	if (project.name === 'pixel') {
		await expect(context).toHaveText('2%');
	} else {
		await expect(context).toContainText('22k / 1M');
		await expect(panel.getByText('23k in · 32 out')).toBeVisible();
	}

	await composer.fill('Anything else?');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(tab.getByTitle('working')).toBeVisible();
	await tab.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
	const confirm = page.getByRole('dialog', { name: 'Close Fake Claude?' });
	await expect(confirm).toBeVisible();
	await confirm.getByRole('button', { name: 'Close' }).click();
	await expect(tab).toHaveCount(0);
});

test('a Codex Chat streams its reply, shows done, and closes', async ({ page }, { project }) => {
	const name = `codex-chat-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Codex as Chat' }).click();
	const tab = page.getByRole('tab', { name: /^Fake Codex/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	await expect(tab.getByTitle('done')).toBeVisible();

	const prompt = 'Greet the reader in one short sentence with one word in **bold**. Use no names and no tools.';
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('textbox', { name: 'Composer' }).fill(prompt);
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.getByText(prompt)).toBeVisible();
	await expect(panel.locator('strong', { hasText: 'glad' })).toBeVisible();
	await expect(panel.getByText('Welcome—glad you’re here.')).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();
	const context = panel.getByRole('meter', { name: 'Context' });
	if (project.name === 'pixel') {
		await expect(context).toHaveText('5%');
	} else {
		await expect(context).toContainText('22k / 475k');
		await expect(panel.getByText('22k in · 14 out')).toBeVisible();
	}

	await tab.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
	await expect(tab).toHaveCount(0);
});

test('a Chat folds tools, thinking, commands and a long first prompt into lines that open on tap', async ({ page }, { project }) => {
	const name = `blocks-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude Tools as Chat' }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude Tools/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	await expect(tab.getByTitle('done')).toBeVisible();

	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const brief = Array.from({ length: 14 }, (_, line) => `Brief line ${line + 1}`).join('\n');
	await panel.getByRole('textbox', { name: 'Composer' }).fill(brief);
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.getByText(/^All steps are done/)).toBeVisible();

	const prompt = panel.locator('summary', { hasText: 'First prompt' });
	await expect(prompt).toContainText('Brief line 1');
	await expect(prompt).toContainText('14 lines');
	const rest = panel.getByText('Brief line 14');
	await expect(rest).toBeHidden();
	await prompt.click();
	await expect(rest).toBeVisible();

	const tools = panel.locator('summary', { hasText: 'Read 2 files · searched the web · used ToolSearch' });
	const search = panel.getByRole('listitem').filter({ hasText: 'IANA example domain' });
	await expect(tools).toBeVisible();
	await expect(search).toBeHidden();
	await tools.click();
	await expect(search).toHaveText('Web search IANA example domain');
	await expect(panel.getByRole('listitem').filter({ hasText: 'todo.txt' })).toHaveText('Read todo.txt');

	await expect(panel.getByText('Thought for 2s')).toHaveCount(2);

	const failed = panel.locator('summary', { hasText: '$ wc -l missing.txt' });
	const output = panel.getByText('wc: missing.txt: No such file or directory');
	await expect(failed).toContainText('✗ failed');
	await expect(panel.locator('summary', { hasText: '$ wc -l notes.txt' })).toContainText('✓');
	await expect(output).toBeHidden();
	await failed.click();
	await expect(output).toBeVisible();

	await tab.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
	await expect(tab).toHaveCount(0);
});

test('a working Chat steers, queues until its turn ends, and interrupts, with no KeyBar', async ({ page }, { project }) => {
	const name = `steer-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	const keys = page.getByRole('toolbar', { name: 'Keys' });
	if (project.name === 'pixel') {
		await page.getByRole('button', { name: 'New Tab' }).click();
		await page.getByRole('menuitem', { name: 'Terminal' }).click();
		await page.locator('.terminal:visible canvas').click();
		await expect(keys).toBeVisible();
	}
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude (steer) as Chat' }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude \(steer\)/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	await expect(tab.getByTitle('done')).toBeVisible();

	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	const queued = panel.getByRole('list', { name: 'Queued' }).getByRole('listitem');
	const sent = (text: string) => panel.locator('.user', { hasText: text });
	const later = 'Count from 1 to 300, one number per line. Use no tools.';
	await composer.click();
	await expect(keys).toHaveCount(0);
	await composer.fill('Count from 1 to 30, one number per line. Use no tools.');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(tab.getByTitle('working')).toBeVisible();
	await expect(panel.getByRole('button', { name: 'Send' })).toHaveCount(0);

	await composer.fill(later);
	await panel.getByRole('button', { name: 'Queue' }).click();
	await expect(queued).toHaveText([later]);
	await expect(composer).toHaveValue('');
	await expect(sent(later)).toHaveCount(0);
	await composer.fill('Then also say banana.');
	await panel.getByRole('button', { name: 'Steer' }).click();
	await expect(sent('Then also say banana.')).toBeVisible();
	await expect(panel.getByText('banana 🍌')).toBeVisible();
	await expect(sent(later)).toBeVisible();
	await expect(queued).toHaveCount(0);
	await expect(tab.getByTitle('working')).toBeVisible();

	await composer.fill('Never sent');
	await panel.getByRole('button', { name: 'Queue' }).click();
	await expect(queued).toHaveText(['Never sent']);
	await panel.getByRole('button', { name: 'Interrupt' }).click();
	await expect(panel.getByText('Interrupted', { exact: true })).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();
	await expect(queued).toHaveCount(0);
	await expect(composer).toHaveValue('Never sent');
	await expect(panel.getByRole('button', { name: 'Send' })).toBeVisible();
	await expect(keys).toHaveCount(0);

	await tab.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
	await expect(tab).toHaveCount(0);
});
const slashed = [
	{ agent: 'claude', rig: 'Fake Claude', command: '/code-review', before: 'Effort: xhigh', after: 'Effort: low' },
	{ agent: 'codex', rig: 'Fake Codex', command: '/openai-docs', before: 'Effort: high', after: 'Effort: low, next turn' }
];

for (const { agent, rig, command, before, after } of slashed) {
	test(`/effort from the slash menu updates the effort chip of a ${rig} Chat`, async ({ page }, { project }) => {
		const name = `slash-${agent}-${project.name}`;
		mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
		await openProject(page, name);
		await page.getByRole('button', { name: 'New Tab' }).click();
		await page.getByRole('menuitem', { name: `${rig} as Chat` }).click();
		const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
		await expect(panel.getByRole('button', { name: before, exact: true })).toBeVisible();

		const composer = panel.getByRole('textbox', { name: 'Composer' });
		await composer.fill('/');
		const menu = panel.getByRole('region', { name: 'Commands' });
		await expect(menu.getByRole('button', { name: new RegExp(`^${command}`) })).toBeVisible();
		await menu.getByRole('button', { name: /^\/effort/ }).click();
		await expect(composer).toHaveValue('/effort ');
		await menu.getByRole('button', { name: 'low', exact: true }).click();
		await expect(panel.getByRole('button', { name: after, exact: true })).toBeVisible();
		await expect(composer).toHaveValue('');
		await expect(menu).toHaveCount(0);

		await panel.getByRole('button', { name: after, exact: true }).click();
		await expect(page.getByRole('menuitemcheckbox', { name: 'low' })).toHaveAttribute('aria-checked', 'true');
		await page.keyboard.press('Escape');
	});
}

for (const agent of ['Claude', 'Codex']) {
	test(`a ${agent} edit is a card that opens its file in a Diff tab`, async ({ page }, { project }) => {
		const name = `${agent.toLowerCase()}-edits-${project.name}`;
		const root = `${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`;
		mkdirSync(root);
		const git = (...args: string[]) =>
			execFileSync('git', ['-C', root, '-c', 'user.name=oikonomos', '-c', 'user.email=e2e@oikonomos.invalid', '-c', 'commit.gpgsign=false', ...args]);
		git('init', '-q');
		writeFileSync(`${root}/notes.txt`, 'alpha\nbeta\ngamma\n');
		git('add', '.');
		git('commit', '-qm', 'Initial notes');
		// The fake Agent only replays its recording, so the edit it reports is made here.
		writeFileSync(`${root}/notes.txt`, 'alpha\nBETA\ngamma\n');
		await openProject(page, name);
		await page.getByRole('button', { name: 'New Tab' }).click();
		await page.getByRole('menuitem', { name: `Fake ${agent} Edits as Chat` }).click();
		const tab = page.getByRole('tab', { name: new RegExp(`^Fake ${agent} Edits`) });
		await expect(tab).toHaveAttribute('aria-selected', 'true');
		await expect(tab.getByTitle('done')).toBeVisible();

		const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
		await panel.getByRole('textbox', { name: 'Composer' }).fill('Make the edits');
		await panel.getByRole('button', { name: 'Send' }).click();
		await expect(tab.getByTitle('done')).toBeVisible();

		const card = panel.locator('.edit').filter({ hasText: 'Edited notes.txt' });
		await expect(card).toContainText('+1 −1');
		const added = card.getByText('+BETA');
		if (project.name === 'pixel') {
			await expect(added).toBeHidden();
			await card.getByText('Edited notes.txt').click();
		}
		await expect(added).toBeVisible();
		await expect(card.getByText('-beta')).toBeVisible();

		await card.getByRole('button', { name: 'Open diff' }).click();
		await expect(page.getByRole('tab', { name: /^notes\.txt ↔ HEAD/ })).toHaveAttribute('aria-selected', 'true');
		const diff = page.locator('.diff:not([hidden])');
		await expect(diff.locator('.path')).toHaveText('notes.txt');
		await expect(diff.locator('.cm-changedLine', { hasText: /^BETA$/ })).toBeVisible();
	});
}

test('a Chat that finishes unseen needs attention and opens from the Attention list or a notification', async ({ page }, { project }) => {
	const name = `attention-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	const terminalTab = page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab').first();
	await expect(terminalTab).toHaveAttribute('aria-selected', 'true');

	const { projectId, chatId } = await page.evaluate(finishElsewhere, name);
	const chatTab = page.locator(`[data-tab="${chatId}"]`);
	await expect(chatTab).toHaveAttribute('aria-selected', 'false');
	const mark = page.locator(`nav[aria-label="Projects"] button[aria-label="${name}"]`).getByText('1', { exact: true });
	await expect(mark).toBeAttached();
	const list = page.getByRole('button', { name: 'Attention list' });
	const entry =
		project.name === 'pixel'
			? page.getByRole('dialog', { name: 'Needs attention' }).getByRole('button', { name: `Fake Claude in ${name}` })
			: page.getByRole('menuitem', { name: `Fake Claude in ${name}` });
	await list.click();
	await entry.click();
	await expect(chatTab).toHaveAttribute('aria-selected', 'true');
	await list.click();
	await expect(entry).toHaveCount(0);
	await expect(mark).toHaveCount(0);
	await page.keyboard.press('Escape');

	// An unknown Tab falls back to the last one, so another Chat keeps this one from being chosen by default.
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude as Chat' }).click();
	await expect(page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab').last()).toHaveAttribute('aria-selected', 'true');
	await expect(chatTab).toHaveAttribute('aria-selected', 'false');
	const notification = { projectId, chatId, label: 'Fake Claude', status: 'done' };
	await page.evaluate((data) => navigator.serviceWorker.dispatchEvent(new MessageEvent('message', { data })), notification);
	await expect(chatTab).toHaveAttribute('aria-selected', 'true');

	const saved = page.waitForResponse((response) => {
		const request = response.request();
		const layout = request.method() === 'PUT' && JSON.parse(request.postData() ?? '{}');
		return layout?.projects?.[projectId]?.active?.kind === 'terminal';
	});
	await terminalTab.click();
	await saved;
	await page.goto(`/?${new URLSearchParams({ project: projectId, tab: chatId })}`);
	await expect(chatTab).toHaveAttribute('aria-selected', 'true');
});

test('four Usage window meters show their readings and turn amber from 90%', async ({ page }, { project }) => {
	await page.goto('/');
	let usage = page.getByRole('region', { name: 'Usage windows' });
	if (project.name === 'pixel') {
		await page.getByRole('button', { name: 'Previous screen' }).click();
		await expect(page.getByRole('navigation', { name: 'Projects' })).toBeInViewport();
	} else {
		await expect(page.getByRole('button', { name: 'Sidebar' })).toBeVisible();
		usage = (await showSidebar(page)).getByRole('region', { name: 'Usage windows' });
	}
	await expect(usage).toBeInViewport();

	await expect(usage.getByRole('meter')).toHaveCount(4);
	for (const name of ['Claude · 5h', 'Claude · week', 'Codex · 5h', 'Codex · week']) {
		await expect(usage.getByRole('meter', { name })).toBeVisible();
	}
	const high = usage.getByRole('meter', { name: 'Codex · 5h' });
	await expect(high).toHaveAttribute('aria-valuenow', '91');
	await expect(usage.getByText('91%')).toBeVisible();
	await expect.poll(() => filledWith(high, '--ansi-3')).toBe(true);
	await expect.poll(() => filledWith(usage.getByRole('meter', { name: 'Claude · week' }), '--ansi-4')).toBe(true);
	await expect(usage.getByText(/^(now|\d+[mhd])$/)).toHaveCount(4);
});

function filledWith(meter: Locator, token: string) {
	return meter.evaluate((element, token) => {
		const probe = document.body.appendChild(document.createElement('i'));
		probe.style.color = `var(${token})`;
		const color = getComputedStyle(probe).color;
		probe.remove();
		return getComputedStyle(element.firstElementChild!).backgroundColor === color;
	}, token);
}

test('each Rig launches as a TUI or a Chat, the Chat first on the phone and the TUI on desktop', async ({ page }, { project }) => {
	const created: string[] = [];
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			const type = typeof payload === 'string' && JSON.parse(payload).type;
			if (type === 'chat.create' || type === 'terminal.create') created.push(type);
		})
	);
	const name = `launch-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	const phone = project.name === 'pixel';
	const order = phone ? ['Fake Claude as Chat', 'Fake Claude'] : ['Fake Claude', 'Fake Claude as Chat'];

	await page.getByRole('button', { name: 'New Tab' }).click();
	const launchers = /^Fake Claude( as Chat)?$/;
	await expect(page.getByRole('menuitem', { name: launchers })).toHaveText(order);
	await page.keyboard.press('Escape');

	await page.keyboard.press('Control+Space');
	await page.keyboard.press('p');
	await page.getByPlaceholder('Switch Project, launch a Rig, or open a file').fill('Fake Claude');
	await expect(page.getByRole('option', { name: launchers })).toHaveText(order);
	await page.keyboard.press('Escape');

	await page.keyboard.press('Control+Space');
	await page.keyboard.press('r');
	await expect(page.getByRole('status').filter({ hasText: order[0] })).toBeVisible();
	await page.keyboard.press('c');
	await expect(page.getByRole('tab', { name: /^Fake Claude/ })).toHaveAttribute('aria-selected', 'true');
	expect(created).toEqual([phone ? 'chat.create' : 'terminal.create']);
});

test('past sessions of both Agents are listed newest first, searchable, and reopen as a Chat', async ({ page }, { project }) => {
	const name = `sessions-${project.name}`;
	const root = process.env.OIKONOMOS_E2E_ROOT;
	const path = `${root}/projects/${name}`;
	mkdirSync(path, { recursive: true });
	const sessions = `${root}/claude/projects/${path.replace(/[^a-zA-Z0-9]/g, '-')}`;
	mkdirSync(sessions, { recursive: true });
	const session = '9afa077a-3173-4e49-9197-7ed54344b24e';
	const fixture = new URL(`../../crates/oikonomos/tests/fixtures/chats/claude-code/sessions/${session}.jsonl`, import.meta.url);
	copyFileSync(fileURLToPath(fixture), `${sessions}/${session}.jsonl`);
	utimesSync(`${sessions}/${session}.jsonl`, new Date(), new Date());
	await openProject(page, name);

	const newTab = page.getByRole('button', { name: 'New Tab' });
	const listed = page.getByRole('group', { name: 'Sessions' }).getByRole('menuitem');
	await newTab.click();
	await expect(listed).toHaveText([/Name a color in one word\./, /Reply with exactly: ok/, /Say hi in three words\./, /Search sessions…/]);
	await expect
		.poll(() => page.getByRole('menu').evaluate((menu) => menu.getBoundingClientRect().bottom <= window.innerHeight))
		.toBe(true);
	await listed.getByText('Search sessions…').click();
	const palette = page.getByRole('dialog', { name: 'Past Sessions' });
	await palette.getByPlaceholder('Find a session by its first prompt').fill('three');
	await expect(palette.getByRole('option')).toHaveText([/Say hi in three words\./]);
	await page.keyboard.press('Escape');
	await expect(palette).toBeHidden();

	await newTab.click();
	await listed.filter({ hasText: 'Name a color in one word.' }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await expect(panel.getByText('Name a color in one word.')).toBeVisible();
	await expect(panel.getByText('Crimson')).toBeVisible();

	await newTab.click();
	await expect(listed).toHaveText([/Reply with exactly: ok/, /Say hi in three words\./, /Search sessions…/]);
	await page.keyboard.press('Escape');
	await tab.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
	await expect(tab).toHaveCount(0);
});

/** Runs a Chat's turn to done from a second Device that shows nothing. */
async function finishElsewhere(name: string) {
	type Of<T> = Extract<ServerMessage, { type: T }>;
	const socket = new WebSocket(new URL('/ws', location.href.replace(/^http/, 'ws')));
	const next = <T extends ServerMessage['type']>(type: T, matches: (message: Of<T>) => boolean = () => true) =>
		new Promise<Of<T>>((resolve) => {
			socket.addEventListener('message', function listener({ data }) {
				const message = JSON.parse(data);
				if (message.type !== type || !matches(message)) return;
				socket.removeEventListener('message', listener);
				resolve(message);
			});
		});
	await new Promise((resolve) => (socket.onopen = resolve));
	socket.send(JSON.stringify({ type: 'hello', deviceId: crypto.randomUUID() }));
	socket.send(JSON.stringify({ type: 'projects.list' }));
	const { projects } = await next('projects.changed');
	const projectId = projects.find((project) => project.path.endsWith(`/${name}`))!.id;
	socket.send(JSON.stringify({ type: 'chat.create', projectId, rig: 'Fake Claude' }));
	const { chat } = await next('chat.created');
	socket.send(JSON.stringify({ type: 'chat.send', chatId: chat.id, text: 'Greet the reader' }));
	await next('chat.status', (message) => message.chatId === chat.id && message.status === 'done');
	socket.close();
	return { projectId, chatId: chat.id };
}


test('a Chat pins todos above its transcript and expands subagent activity', async ({ page }, { project }) => {
	const name = `subagent-${project.name}`;
	const root = process.env.OIKONOMOS_E2E_ROOT!;
	mkdirSync(`${root}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: `Fake Claude Subagent ${project.name} as Chat` }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude Subagent/ });
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('textbox', { name: 'Composer' }).fill('Find needle using a subagent and track the steps.');
	await panel.getByRole('button', { name: 'Send' }).click();
	const todos = panel.getByRole('region', { name: 'Todos' });
	await expect(todos.locator('summary')).toHaveText('Todos 0 of 3 · Find which file mentions "needle"');
	await expect(tab.getByTitle('working')).toBeVisible();
	await expect.poll(() => existsSync(`${root}/subagent-${project.name}/todos`)).toBe(true);
	const expanded = project.name === 'pixel';
	await expect(todos.locator('details')).toHaveJSProperty('open', expanded);
	if (expanded) {
		await expect(todos.getByRole('listitem')).toHaveCount(3);
		await expect(todos.getByRole('listitem').first()).toBeInViewport();
	} else {
		await expect(todos.getByRole('listitem').first()).toBeHidden();
		await todos.locator('summary').click();
		await expect(todos.getByRole('listitem').first()).toBeVisible();
	}
	unlinkSync(`${root}/subagent-${project.name}/todos`);
	await expect(tab.getByTitle('done')).toBeVisible();
	await expect(todos.locator('summary')).toHaveText('Todos 3 of 3');
	const child = panel.locator('details').filter({ has: page.locator('summary', { hasText: 'Explore · Find file mentioning needle' }) });
	await expect(child.locator(':scope > summary')).toContainText('↳ Explore · Find file mentioning needle');
	await expect(child.locator('summary .status')).toHaveText('done');
	await expect(child.getByText('Read 2 files')).toBeHidden();
	await child.locator(':scope > summary').click();
	await expect(child.getByText('Read 2 files')).toBeVisible();
	await child.getByText('Read 2 files').click();
	await expect(child.getByRole('listitem')).toHaveText(['Read notes.txt', 'Read todo.txt']);
	await tab.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
	await expect(tab).toHaveCount(0);
});
