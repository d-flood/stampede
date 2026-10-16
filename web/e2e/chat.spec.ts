import type { ServerMessage } from '#lib/device.svelte.ts';
import { expect, test, type Locator } from '@playwright/test';
import { copyFileSync, mkdirSync, utimesSync } from 'node:fs';
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
