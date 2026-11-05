import type { ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import type { Locator, WebSocketRoute } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { appendFileSync, copyFileSync, existsSync, mkdirSync, readFileSync, unlinkSync, utimesSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { closePinned, closeProject, expectCurrent, expectSelected, expectShown, newTab, openProject, pinned, openSwitcher, showScreen, showSidebar, showTab, tabCount } from './shell.ts';

test('a Claude Code Chat streams its reply, shows done, and asks before closing while working', async ({ page }, { project }) => {
	const name = `chat-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Fake Claude');
	const tab = await pinned(page.getByRole('tab', { name: /^Fake Claude/ }));
	await expectSelected(tab);
	await expect(tab.getByTitle('done')).toBeVisible();
	const square = page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name, exact: true });
	await expect(square.getByTitle('working')).toHaveCount(0);

	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const context = panel.getByRole('meter', { name: 'Context' });
	await expect(context).toHaveCount(0);
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('Greet the reader');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.getByRole('region', { name: 'User message 1' }).getByText('Greet the reader')).toBeVisible();
	await expect(panel.locator('strong', { hasText: 'welcome' })).toBeVisible();
	await expect(panel.getByText(/^Hello and welcome!/)).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();
	await expect(context).toHaveText('2%');
	await expect(context).toHaveAttribute('aria-valuetext', '22k / 1M (2%)');
	if (project.name === 'desktop') {
		await context.hover();
		await expect(page.getByRole('tooltip').filter({ hasText: 'Context 22k / 1M (2%)' })).toContainText('23k in · 32 out');
	}

	await composer.fill('Anything else?');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(tab.getByTitle('working')).toBeVisible();
	await expect(square.getByTitle('working')).toBeVisible();
	await closePinned(tab);
	const confirm = page.getByRole('dialog', { name: 'Close Fake Claude?' });
	await expect(confirm).toBeVisible();
	await confirm.getByRole('button', { name: 'Close' }).click();
	await expect(tab).toHaveCount(0);
	await expect(square.getByTitle('working')).toHaveCount(0);
});

test('a closed Chat stays disabled until its Agent exits', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `chat-closing-${project.name}`;
	const directory = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(directory, { recursive: true });
	writeFileSync(`${directory}/.fake-slow-exit`, '');
	await openProject(page, name);
	await newTab(page, 'Fake Claude');
	const tab = await pinned(page.getByRole('tab', { name: /^Fake Claude/ }));
	await expectSelected(tab);
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('textbox', { name: 'Composer' }).fill('Greet the reader');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.getByText(/^Hello and welcome!/)).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();

	const pressed = Date.now();
	await page.keyboard.press('Control+W');
	const closing = page.getByRole('tab', { name: 'Greet the reader' });
	await expect(closing).toHaveAttribute('aria-disabled', 'true', { timeout: 200 });
	await page.waitForTimeout(pressed + 500 - Date.now());
	await expect(closing).toHaveCount(1, { timeout: 0 });
	await expect(closing).toHaveCount(0, { timeout: pressed + 3000 - Date.now() });
});

test('a Codex Chat streams its reply, shows done, and closes', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `codex-chat-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Fake Codex');
	const tab = await pinned(page.getByRole('tab', { name: /^Fake Codex/ }));
	await expectSelected(tab);
	await expect(tab.getByTitle('done')).toBeVisible();

	const prompt = 'Greet the reader in one short sentence with one word in **bold**. Use no names and no tools.';
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('textbox', { name: 'Composer' }).fill(prompt);
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.locator('.user')).toHaveText(prompt.replaceAll('**', ''));
	await expect(panel.locator('strong', { hasText: 'glad' })).toBeVisible();
	await expect(panel.getByText('Welcome—glad you’re here.')).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();
	const context = panel.getByRole('meter', { name: 'Context' });
	await expect(context).toHaveText('5%');
	await expect(context).toHaveAttribute('aria-valuetext', '22k / 475k (5%)');
	await context.hover();
	await expect(page.getByRole('tooltip').filter({ hasText: 'Context 22k / 475k (5%)' })).toContainText('22k in · 14 out');

	await closePinned(tab);
	await expect(tab).toHaveCount(0);
});

test('a Chat folds tools, thinking, commands and a long first prompt into lines that open on tap', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `blocks-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Fake Claude Tools');
	const tab = await pinned(page.getByRole('tab', { name: /^Fake Claude Tools/ }));
	await expectSelected(tab);
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

	const search = panel.locator('.call').filter({ hasText: 'IANA example domain' });
	const calls = panel.locator('.calls > .summary');
	await expect(calls).toHaveText('Ran 2 commands, read 3 files, searched the web 1 time, used 1 tool, 1 failed');
	await expect(panel.locator('.calls .failure')).toHaveText(/^\s*wc -l missing\.txt\s*failed\s*Show output\s*$/);
	await expect(search).toBeHidden();
	await calls.click();
	await expect(search).toHaveText('Web search IANA example domain');
	await expect(panel.locator('.call').filter({ hasText: 'todo.txt' })).toHaveText('Read todo.txt');

	await expect(panel.getByText('Thought for 2s')).toHaveCount(2);

	const failed = panel.locator('summary', { hasText: '$ wc -l missing.txt' });
	const output = panel.getByText('wc: missing.txt: No such file or directory');
	await expect(failed).toContainText('✗ failed');
	await expect(panel.locator('summary', { hasText: '$ wc -l notes.txt' })).toContainText('✓');
	await expect(output).toBeHidden();
	await failed.click();
	await expect(output).toBeVisible();

	await closePinned(tab);
	await expect(tab).toHaveCount(0);
});

test('a working Chat steers, queues until its turn ends, and interrupts, with no KeyBar', async ({ page }, { project }) => {
	const name = `steer-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	const keys = page.getByRole('toolbar', { name: 'Keys' });
	if (project.name === 'pixel') {
		await newTab(page, 'Terminal');
		await page.locator('.terminal:visible canvas').click();
		await expect(keys).toBeVisible();
	}
	await newTab(page, 'Fake Claude (steer)');
	const tab = await pinned(page.getByRole('tab', { name: /^Fake Claude \(steer\)/ }));
	await expectSelected(tab);
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
	if (project.name === 'pixel') {
		await page.setViewportSize({ width: 375, height: 812 });
		await expect.poll(() => panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
		await expect(panel.getByRole('button', { name: 'Commands', exact: true })).toBeInViewport();
	}

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
	if (project.name === 'desktop') await composer.press('Escape');
	else await panel.getByRole('button', { name: 'Interrupt' }).click();
	await expect(panel.getByText('Interrupted', { exact: true })).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();
	await expect(queued).toHaveCount(0);
	await expect(composer).toHaveValue('Never sent');
	await expect(panel.getByRole('button', { name: 'Send' })).toBeVisible();
	await expect(keys).toHaveCount(0);

	await closePinned(tab);
	await expect(tab).toHaveCount(0);
});
test('long model and mode chips stay on one line', { tag: '@phone' }, async ({ page }, { project }) => {
	let route: WebSocketRoute;
	let chatId: string;
	const settings = { model: 'gpt-6.1-sol-extended-preview', effort: 'high', mode: 'approve for me every single time' };
	await page.routeWebSocket('**/ws', (socket) => {
		route = socket;
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data === 'string') {
				const message: ServerMessage = JSON.parse(data);
				if (message.type === 'chat.created') chatId = message.chat.id;
			}
			socket.send(data);
		});
	});
	const name = `chips-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Fake Codex');
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const before = await panel.getByRole('button', { name: /^Model: .* high$/ }).evaluate((element) => element.getBoundingClientRect().height);
	route!.send(JSON.stringify({ type: 'chat.settings', chatId: chatId!, settings }));
	const model = panel.getByRole('button', { name: `Model: ${settings.model} high`, exact: true });
	const mode = panel.getByRole('button', { name: `Mode: ${settings.mode}`, exact: true });
	await expect(model).toBeVisible();
	for (const chip of [model, mode]) await expect.poll(() => chip.evaluate((element) => element.getBoundingClientRect().height)).toBe(before);
	await expect.poll(() => panel.locator('.chips').evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
});

const slashed = [
	{ agent: 'claude', rig: 'Fake Claude', command: '/code-review', before: /^Model: \S+ xhigh$/, after: /^Model: \S+ low$/ },
	{ agent: 'codex', rig: 'Fake Codex', command: '/openai-docs', before: /^Model: \S+ high$/, after: /^Model: \S+ low, next turn$/ }
];

for (const { agent, rig, command, before, after } of slashed) {
	test(`/effort from the slash menu updates the effort chip of a ${rig} Chat`, { tag: agent === 'codex' ? '@desktop' : [] }, async ({ page }, { project }) => {
		const name = `slash-${agent}-${project.name}`;
		mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
		await openProject(page, name);
		await newTab(page, rig);
		const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
		await expect(panel.getByRole('button', { name: before })).toBeVisible();

		const composer = panel.getByRole('textbox', { name: 'Composer' });
		const menu = panel.getByRole('region', { name: 'Commands' });
		if (project.name === 'pixel') {
			const commands = panel.getByRole('button', { name: 'Commands', exact: true });
			await composer.fill('Keep this draft');
			await commands.click();
			await expect(menu).toBeVisible();
			await expect(commands).toHaveAttribute('aria-expanded', 'true');
			await expect(composer).toHaveValue('Keep this draft');
			await commands.click();
			await expect(menu).toHaveCount(0);
			await expect(commands).toHaveAttribute('aria-expanded', 'false');
			await expect(composer).toHaveValue('Keep this draft');
			await commands.click();
			await expect.poll(() => panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
		} else {
			await composer.fill('/');
		}
		await expect(menu.getByRole('button', { name: new RegExp(`^${command}`) })).toBeVisible();
		await menu.getByRole('button', { name: /^\/effort/ }).click();
		await expect(composer).toHaveValue('/effort ');
		await menu.getByRole('button', { name: 'low', exact: true }).click();
		await expect(panel.getByRole('button', { name: after })).toBeVisible();
		await expect(composer).toHaveValue('');
		await expect(menu).toHaveCount(0);

		await panel.getByRole('button', { name: after }).click();
		await expect(page.getByRole('menuitemcheckbox', { name: 'low' })).toHaveAttribute('aria-checked', 'true');
		await page.keyboard.press('Escape');
	});
}

test('/clear is not offered and is refused when typed', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `no-clear-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Fake Claude');
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	const menu = panel.getByRole('region', { name: 'Commands' });
	await expect(panel.getByRole('button', { name: 'Model: opus xhigh', exact: true })).toBeVisible();
	await composer.fill('/');
	await expect(menu.getByRole('button', { name: /^\/compact/ })).toBeVisible();
	await expect(menu.getByRole('button', { name: /^\/clear/ })).toHaveCount(0);
	for (const command of ['/clear', '/reset', '/new My next task']) {
		await composer.fill(command);
		await panel.getByRole('button', { name: 'Send', exact: true }).click();
		await expect(panel.getByRole('alert')).toContainText(`${command.split(' ')[0]} isn't available here`);
		await expect(composer).toHaveValue(command);
	}
	await expect(panel.locator('.transcript:visible').getByText('/clear', { exact: true })).toHaveCount(0);
});

test('speed and Ultracode change from the slash menu and show on the model control only once chosen', async ({ page }, { project }) => {
	const name = `speed-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Fake Claude');
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	const menu = panel.getByRole('region', { name: 'Commands' });
	await expect(panel.getByRole('button', { name: 'Model: opus xhigh', exact: true })).toBeVisible();
	await expect(panel.getByRole('button', { name: /^(Speed|Ultracode):/ })).toHaveCount(0);
	if (project.name !== 'pixel') {
		const size = page.viewportSize()!;
		await page.setViewportSize({ width: 900, height: size.height });
		await expect.poll(() => panel.getByRole('button', { name: 'Model: opus xhigh', exact: true }).evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
		await page.setViewportSize(size);
	}

	await composer.fill('/speed ');
	await menu.getByRole('button', { name: /^fast/ }).click();
	await expect(panel.getByRole('button', { name: 'Model: opus xhigh fast', exact: true })).toBeVisible();
	await composer.fill('/ultracode ');
	await menu.getByRole('button', { name: /^on/ }).click();
	const model = panel.getByRole('button', { name: 'Model: opus xhigh fast ultracode on', exact: true });
	await expect(model).toContainText('ultracode on');
	await expect.poll(() => panel.locator('.chips').first().evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);

	await model.click();
	await expect(page.getByRole('menuitemcheckbox', { name: 'standard' })).toHaveAttribute('aria-checked', 'false');
	await expect(page.getByRole('menuitemcheckbox', { name: 'fast' })).toContainText('Costs more; plans use usage credits');
	await page.getByRole('menuitemcheckbox', { name: 'standard' }).click();
	await expect(panel.getByRole('button', { name: 'Model: opus xhigh ultracode on', exact: true })).toBeVisible();
});

test('/speed takes the name Codex shows for a tier and applies on the next turn', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `speed-codex-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Fake Codex');
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await expect(panel.getByRole('button', { name: /^Model: \S+ high$/ })).toBeVisible();
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('/speed fast');
	await composer.press('Enter');
	await expect(panel.getByRole('button', { name: /^Model: \S+ high fast, next turn$/ })).toBeVisible();
	await panel.getByRole('button', { name: /^Model:/ }).click();
	await expect(page.getByRole('menu').getByText('Speed · next turn', { exact: true })).toBeVisible();
	await expect(page.getByRole('menu').getByText('Ultracode', { exact: true })).toHaveCount(0);
	await page.keyboard.press('Escape');
	await expect(composer).toHaveValue('');
});

test('an Ultracode Workflow asks with its script, runs as a subagent, and wakes Claude when it finishes', async ({ page }, { project }) => {
	const name = `workflow-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Fake Claude Workflow');
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('textbox', { name: 'Composer' }).fill('Run the workflow');
	await panel.getByRole('button', { name: 'Send', exact: true }).click();
	await expect(panel.getByText('Claude wants to run a workflow: Two agents')).toBeVisible();
	await expect(panel.getByText('await parallel([').first()).toBeVisible();
	await panel.getByRole('button', { name: 'Allow', exact: true }).click();
	await expect(panel.getByText('The two words are')).toBeVisible();
	await panel.getByRole('region', { name: 'Subagents' }).getByRole('button', { name: 'Subagents: 1 done' }).click();
	await expect(page.getByText('Two agents, no tools: one replies alpha, the other replies beta').first()).toBeVisible();
	await expect(page.getByText('alpha-beta-pair').first()).toBeVisible();
});

test('/create-epic sends two turns while showing one user command', async ({ page }, { project }) => {
	const root = process.env.STAMPEDE_E2E_ROOT;
	const name = `template-${project.name}`;
	mkdirSync(`${root}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Fake Claude');
	const tab = await pinned(page.getByRole('tab', { name: /^Fake Claude/ }));
	await expect(tab.getByTitle('done')).toBeVisible();

	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('/');
	const menu = panel.getByRole('region', { name: 'Commands' });
	await expect(menu.getByRole('button', { name: /^\/(spec|tickets)\b/ })).toHaveCount(0);
	await menu.getByRole('button', { name: /^\/create-epic create an Epic/ }).click();
	await expect(composer).toHaveValue('/create-epic ');
	await expect(menu).toHaveCount(0);
	await composer.press('End');
	await composer.pressSequentially('go');
	await panel.getByRole('button', { name: 'Send' }).click();

	await expect(panel.locator('.user')).toHaveText('/create-epic go');
	const data = `${root}/data`;
	const sent = () =>
		readFileSync(`${root}/claude/fake-received.jsonl`, 'utf8')
			.split('\n')
			.filter(Boolean)
			.map((line) => JSON.parse(line))
			.filter((message) => message.type === 'user')
			.map((message) => message.message.content);
	const note = `This Stampede server keeps its data in ${data}, so pass \`--data-dir ${data}\` to every \`stampede\` command.`;
	await expect.poll(() => sent().length).toBe(2);
	const prompts: string[] = sent();
	expect(prompts[0].endsWith(`${note}\n\ngo`)).toBe(true);
	expect(prompts[1].endsWith(note)).toBe(true);
	expect(prompts[0]).not.toBe(prompts[1]);
	await expect(panel.getByText('Building Tickets', { exact: true })).toBeVisible();
	await expect(panel.locator('.user')).toHaveText('/create-epic go');
	await expect(panel.getByRole('region', { name: 'User message 2' })).toHaveCount(0);

	await closePinned(tab);
	await page.getByRole('dialog', { name: /^Close / }).getByRole('button', { name: 'Close', exact: true }).click();
	await expect(tab).toHaveCount(0);
});

for (const agent of ['Claude', 'Codex']) {
	test(`a ${agent} edit is a card that opens its file in a Diff tab`, { tag: agent === 'Codex' ? '@desktop' : [] }, async ({ page }, { project }) => {
		const name = `${agent.toLowerCase()}-edits-${project.name}`;
		const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
		mkdirSync(root);
		const git = (...args: string[]) =>
			execFileSync('git', ['-C', root, '-c', 'user.name=stampede', '-c', 'user.email=e2e@stampede.invalid', '-c', 'commit.gpgsign=false', ...args]);
		git('init', '-q');
		writeFileSync(`${root}/notes.txt`, 'alpha\nbeta\ngamma\n');
		git('add', '.');
		git('commit', '-qm', 'Initial notes');
		// The fake Agent only replays its recording, so the edit it reports is made here.
		writeFileSync(`${root}/notes.txt`, 'alpha\nBETA\ngamma\n');
		await openProject(page, name);
		await newTab(page, `Fake ${agent} Edits`);
		const tab = await pinned(page.getByRole('tab', { name: new RegExp(`^Fake ${agent} Edits`) }));
		await expectSelected(tab);
		await expect(tab.getByTitle('done')).toBeVisible();

		const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
		await panel.getByRole('textbox', { name: 'Composer' }).fill('Make the edits');
		await panel.getByRole('button', { name: 'Send' }).click();
		await expect(tab.getByTitle('done')).toBeVisible();

		const card = panel.locator('.edit').filter({ hasText: 'Edited notes.txt' });
		await expect(card).toBeHidden();
		const chip = panel.locator('.calls .chip', { hasText: 'notes.txt' });
		await expect(chip).toHaveText('notes.txt+1−1');
		await panel.locator('.calls > .summary').click();
		await expect(card).toContainText('+1 −1');
		const added = card.getByText('+BETA');
		await expect(added).toBeHidden();
		await card.getByText('Edited notes.txt').click();
		await expect(added).toBeVisible();
		await expect(card.getByText('-beta')).toBeVisible();

		await chip.click();
		await expectCurrent(page, 'notes.txt ↔ HEAD');
		const diff = page.locator('.diff:not([hidden])');
		await expect(diff.locator('.path')).toHaveText('notes.txt');
		await expect(diff.locator('.cm-changedLine', { hasText: /^BETA$/ })).toBeVisible();
	});
}

test('a Chat that finishes unseen needs attention and opens from the Attention list or a notification', async ({ page }, { project }) => {
	const name = `attention-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, 'Terminal');
	const terminalTab = await pinned(page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab').first());
	const terminalId = (await terminalTab.getAttribute('data-tab')) ?? (await terminalTab.getAttribute('data-current-tab'))!;
	await expectShown(page, terminalId);

	const { projectId, chatId } = await page.evaluate(finishElsewhere, name);
	await expectShown(page, chatId, false);
	const mark = page.locator(`nav[aria-label="Projects"] button[aria-label="${name}"]`).getByText('1', { exact: true });
	await expect(mark).toBeAttached();
	const list = page.getByRole('button', { name: 'Attention list' });
	const entry =
		project.name === 'pixel'
			? page.getByRole('dialog', { name: 'Needs attention' }).getByRole('button', { name: `Fake Claude in ${name}` })
			: page.getByRole('menuitem', { name: `Fake Claude in ${name}` });
	await list.click();
	await entry.click();
	await expectShown(page, chatId);
	await list.click();
	await expect(entry).toHaveCount(0);
	await expect(mark).toHaveCount(0);
	await page.keyboard.press('Escape');

	// An unknown Tab falls back to the last one, so another Chat keeps this one from being chosen by default.
	await newTab(page, 'Fake Claude');
	if (project.name !== 'pixel') await expect(page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab').last()).toHaveAttribute('aria-selected', 'true');
	await expectShown(page, chatId, false);
	const notification = { projectId, chatId, label: 'Fake Claude', status: 'done' };
	await page.evaluate((data) => navigator.serviceWorker.dispatchEvent(new MessageEvent('message', { data })), notification);
	await expectShown(page, chatId);

	const saved = page.waitForResponse((response) => {
		const request = response.request();
		const layout = request.method() === 'PUT' && JSON.parse(request.postData() ?? '{}');
		return layout?.projects?.[projectId]?.active?.kind === 'terminal';
	});
	await showTab(page, terminalId);
	await saved;
	await page.goto(`/?${new URLSearchParams({ project: projectId, tab: chatId })}`);
	await expectShown(page, chatId);
});

test('four Usage window meters show their readings and warn from 80%', async ({ page }, { project }) => {
	await page.goto('/');
	let usage = page.getByRole('region', { name: 'Usage windows' });
	if (project.name === 'pixel') {
		await showScreen(page, 'projects');
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
	await expect(high).toHaveText('91 5h');
	await expect.poll(() => filledWith(high, '--status-waiting')).toBe(true);
	await expect.poll(() => filledWith(usage.getByRole('meter', { name: 'Claude · week' }), '--brand')).toBe(true);
	await usage.getByRole('button', { name: 'Usage details' }).click();
	const details = page.getByRole('dialog', { name: 'Usage details' });
	await expect(details.locator('.age')).toHaveCount(2);
	await expect(details.locator('.age').first()).toHaveText(/^(just now|\d+[mhd] ago)$/);
});

function filledWith(meter: Locator, token: string) {
	return meter.evaluate((element, token) => {
		const probe = element.appendChild(document.createElement('i'));
		probe.style.color = `var(${token})`;
		const color = getComputedStyle(probe).color;
		probe.remove();
		return getComputedStyle(element.querySelector('.arc')!).stroke === color;
	}, token);
}

test('Rigs launch Chats from New Tab, the Palette, and shortcuts on every device', async ({ page }, { project }) => {
	const created: string[] = [];
	page.on('websocket', (socket) => socket.on('framesent', ({ payload }) => {
		const type = typeof payload === 'string' && JSON.parse(payload).type;
		if (type === 'chat.create' || type === 'terminal.create') created.push(type);
	}));
	const name = `launch-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	const phone = project.name === 'pixel';
	const chats = (count: number) => phone ? expect.poll(() => tabCount(page)).toBe(count) : expect(page.getByRole('tab', { name: /^Fake Claude/ })).toHaveCount(count);
	const menu = phone ? (await openSwitcher(page)).locator('section') : page.getByRole('menu');
	if (!phone) await page.getByRole('button', { name: 'New Tab' }).click();
	const item = phone ? 'button' : 'menuitem';
	await expect(menu.getByRole(item, { name: 'Fake Claude', exact: true })).toHaveCount(1);
	await expect(menu.getByRole(item, { name: /as Chat|as TUI/ })).toHaveCount(0);
	await menu.getByRole(item, { name: 'Fake Claude', exact: true }).click();
	await chats(1);

	await page.keyboard.press('Control+;');
	await page.keyboard.press('p');
	await page.getByPlaceholder('Switch Project, launch a Rig, or open a file').fill('Fake Claude');
	await page.getByRole('option', { name: 'Fake Claude', exact: true }).click();
	await chats(2);

	await page.keyboard.press('Control+;');
	await page.keyboard.press('r');
	await expect(page.getByRole('status').filter({ hasText: 'Fake Claude' })).toBeVisible();
	await page.keyboard.press('c');
	await chats(3);
	if (phone) await expectCurrent(page, 'Fake Claude');
	else await expect(page.getByRole('tab', { name: /^Fake Claude/ }).last()).toHaveAttribute('aria-selected', 'true');
	expect(created).toEqual(['chat.create', 'chat.create', 'chat.create']);
	await expect(page.getByRole('button', { name: 'Open as TUI' })).toHaveCount(0);
	await closeProject(page, name);
});

test('past sessions of both Agents are listed newest first, searchable, and reopen as a Chat', async ({ page }, { project }) => {
	const name = `sessions-${project.name}`;
	const root = process.env.STAMPEDE_E2E_ROOT;
	const path = `${root}/projects/${name}`;
	mkdirSync(path, { recursive: true });
	const sessions = `${root}/claude/projects/${path.replace(/[^a-zA-Z0-9]/g, '-')}`;
	mkdirSync(sessions, { recursive: true });
	const session = '9afa077a-3173-4e49-9197-7ed54344b24e';
	const fixture = new URL(`../../crates/stampede/tests/fixtures/chats/claude-code/sessions/${session}.jsonl`, import.meta.url);
	copyFileSync(fileURLToPath(fixture), `${sessions}/${session}.jsonl`);
	appendFileSync(`${sessions}/${session}.jsonl`, `\n${JSON.stringify({ type: 'custom-title', customTitle: 'Palette study' })}\n`);
	utimesSync(`${sessions}/${session}.jsonl`, new Date(), new Date());
	await openProject(page, name);

	const phone = project.name === 'pixel';
	const sessionItems = async () => {
		if (phone) return (await openSwitcher(page)).getByRole('region', { name: 'Recent sessions' }).getByRole('button');
		await page.getByRole('button', { name: 'New Tab' }).click();
		return page.getByRole('group', { name: 'Sessions' }).getByRole('menuitem');
	};
	let listed = await sessionItems();
	await expect(listed).toHaveText([/Palette study/, /Reply with exactly: ok/, /Say hi in three words\./, /Search sessions…/]);
	if (!phone) {
		await expect
			.poll(() => page.getByRole('menu').evaluate((menu) => menu.getBoundingClientRect().bottom <= window.innerHeight))
			.toBe(true);
	}
	await listed.getByText('Search sessions…').click();
	const palette = page.getByRole('dialog', { name: 'Past Sessions' });
	await palette.getByPlaceholder('Find a session by title').fill('palette');
	await expect(palette.getByRole('option')).toHaveText([/Palette study/]);
	await palette.getByPlaceholder('Find a session by title').fill('three');
	await expect(palette.getByRole('option')).toHaveText([/Say hi in three words\./]);
	await page.keyboard.press('Escape');
	await expect(palette).toBeHidden();

	listed = await sessionItems();
	await listed.filter({ hasText: 'Palette study' }).click();
	const tab = await pinned(page.getByRole('tab', { name: 'Palette study' }));
	await expectSelected(tab);
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await expect(panel.getByRole('region', { name: 'User message 1' }).getByText('Name a color in one word.')).toBeVisible();
	await expect(panel.getByText('Crimson')).toBeVisible();

	listed = await sessionItems();
	await expect(listed).toHaveText([/Reply with exactly: ok/, /Say hi in three words\./, /Search sessions…/]);
	await page.keyboard.press('Escape');
	await closePinned(tab);
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


test('a Chat pins todos and counts its completed subagent in the Tray', async ({ page }, { project }) => {
	const name = `subagent-${project.name}`;
	const root = process.env.STAMPEDE_E2E_ROOT!;
	mkdirSync(`${root}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await newTab(page, `Fake Claude Subagent ${project.name}`);
	const tab = await pinned(page.getByRole('tab', { name: /^Fake Claude Subagent/ }));
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('textbox', { name: 'Composer' }).fill('Find needle using a subagent and track the steps.');
	await panel.getByRole('button', { name: 'Send' }).click();
	const todos = panel.getByRole('region', { name: 'Todos' });
	await expect(todos.locator('summary')).toHaveText('Todos 0 of 3 · Find which file mentions "needle"');
	await expect(tab.getByTitle('working')).toBeVisible();
	await expect.poll(() => existsSync(`${root}/subagent-${project.name}/todos`)).toBe(true);
	await expect(todos.locator('details')).toHaveJSProperty('open', false);
	await expect(todos.getByRole('listitem').first()).toBeHidden();
	await todos.locator('summary').click();
	await expect(todos.getByRole('listitem')).toHaveCount(3);
	await expect(todos.getByRole('listitem').first()).toBeInViewport();
	unlinkSync(`${root}/subagent-${project.name}/todos`);
	await expect(tab.getByTitle('done')).toBeVisible();
	await expect(todos.locator('summary')).toHaveText('Todos 3 of 3');
	await expect(panel.getByText('Explore · Find file mentioning needle')).toHaveCount(0);
	const picker = page.getByRole('combobox', { name: /^Session in/ });
	const subagents = panel.getByRole('region', { name: 'Subagents' });
	await expect(picker).toHaveCount(0);
	await expect(subagents.getByRole('button')).toHaveAccessibleName('Subagents: 1 done');
	const chatId = (await tab.getAttribute('data-tab')) ?? (await tab.getAttribute('data-current-tab'))!;
	await newTab(page, 'Terminal');
	await expect(picker).toHaveCount(0);
	await showTab(page, chatId);
	await expect(picker).toHaveCount(0);
	await expect(subagents.getByRole('button')).toHaveAccessibleName('Subagents: 1 done');
	await expect(panel.getByRole('textbox', { name: 'Composer' })).toBeVisible();
	await closePinned(tab);
	await expect(tab).toHaveCount(0);
});

for (const agent of ['claude', 'codex']) {
	test(`${agent} shows current activity before completion and clears it afterward`, { tag: '@desktop' }, async ({ page }, { project }) => {
		const name = `activity-${agent}-${project.name}`;
		const root = process.env.STAMPEDE_E2E_ROOT;
		const gates = `${root}/${name}`;
		const kind = agent === 'claude' ? 'claude-code' : 'codex';
		const recording = readFileSync(fileURLToPath(new URL(`../../crates/stampede/tests/fixtures/chats/${kind}/tools.recorded.jsonl`, import.meta.url)), 'utf8');
		const inserted = new Set<string>();
		let turns = 0;
		const entries = recording.trim().split('\n').flatMap((line) => {
			const entry = JSON.parse(line);
			if (entry.stdin?.method === 'turn/start') turns++;
			const message = entry.stdout;
			let gate: string | undefined;
			if (agent === 'codex') {
				if (message?.method === 'item/started') {
					gate = ({ reasoning: 'thinking', commandExecution: 'command', webSearch: 'tool' } as Record<string, string>)[message.params.item.type];
				}
				if (turns === 2 && message?.method === 'turn/started') gate = 'resume';
			} else {
				if (message?.type === 'stream_event' && message.event.type === 'content_block_start' && message.event.content_block.type === 'thinking') gate = 'thinking';
				if (message?.type === 'assistant') {
					if (message.message.content.some((block: { name?: string }) => block.name === 'Read')) gate = 'tool';
					if (message.message.content.some((block: { name?: string }) => block.name === 'Bash')) gate = 'command';
				}
			}
			if (!gate || inserted.has(gate)) return [entry];
			inserted.add(gate);
			return [entry, { gate }];
		});
		writeFileSync(`${gates}/tools.jsonl`, entries.map((entry) => JSON.stringify(entry)).join('\n'));
		mkdirSync(`${root}/projects/${name}`);
		await openProject(page, name);
		await newTab(page, `Activity ${agent} ${project.name}`);
		const tab = await pinned(page.getByRole('tab', { name: new RegExp(`^Activity ${agent}`) }));
		const panel = page.locator('.panel[role="tabpanel"]:visible');
		const activity = panel.getByRole('status', { name: 'Current activity' });
		await panel.getByRole('textbox', { name: 'Composer' }).fill('Run the tools');
		await panel.getByRole('button', { name: 'Send' }).click();
		const stages = agent === 'codex' ? ['thinking', 'command', 'tool'] : ['thinking', 'tool', 'command'];
		for (const stage of stages) {
			await expect.poll(() => existsSync(`${gates}/${stage}`)).toBe(true);
			await expect(activity).toContainText(stage === 'thinking' ? 'Thinking' : stage === 'command' ? 'Running wc -l notes.txt' : agent === 'codex' ? 'Searching the web' : 'Reading');
			await expect(tab.getByTitle('working')).toBeVisible();
			if (stage !== 'thinking') {
				const calls = panel.locator('.calls > .summary').last();
				await expect(calls).toContainText(/^[A-Z][a-z]+ \d/);
				await expect(calls.locator('.live')).toHaveText(`· ${await activity.locator('.label').innerText()}`);
				await expect(panel.locator('.calls > .summary[aria-expanded="true"]')).toHaveCount(0);
			}
			unlinkSync(`${gates}/${stage}`);
		}
		await expect(tab.getByTitle('done')).toBeVisible();
		await expect(activity).toHaveCount(0);
		if (agent === 'codex') {
			await newTab(page, 'Terminal');
			const chatId = await tab.getAttribute('data-tab');
			await page.evaluate((chatId) => new Promise<void>((resolve) => {
				const socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/ws`);
				socket.onopen = () => socket.send(JSON.stringify({ type: 'chat.send', chatId, text: 'Say done in one word.', queue: false }));
				socket.onmessage = ({ data }) => {
					if (typeof data === 'string' && JSON.parse(data).type === 'chat.status') { socket.close(); resolve(); }
				};
			}), chatId);
			await expect.poll(() => existsSync(`${gates}/resume`)).toBe(true);
			await expect(tab.getByTitle('working')).toBeVisible();
			await tab.click();
			await expect(activity).toBeVisible();
			unlinkSync(`${gates}/resume`);
			await expect(tab.getByTitle('done')).toBeVisible();
			await expect(activity).toHaveCount(0);
		}
	});
}

test('a completed Codex subagent counts as done in the Tray', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `codex-subagent-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	await newTab(page, `Fake Codex Subagent ${project.name}`);
	const tab = await pinned(page.getByRole('tab', { name: /^Fake Codex Subagent/ }));
	const panel = page.locator('.panel[role="tabpanel"]:visible');
	await panel.getByRole('textbox', { name: 'Composer' }).fill('Find needle using a subagent and track the steps.');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(tab.getByTitle('done')).toBeVisible();
	await expect(page.getByRole('combobox', { name: /^Session in/ })).toHaveCount(0);
	await expect(panel.getByRole('region', { name: 'Subagents' }).getByRole('button')).toHaveAccessibleName('Subagents: 1 done');
	await expect(panel.getByText('notes.txt mentions “needle” and has 3 lines.', { exact: true })).toBeVisible();
	await expect(panel.getByRole('textbox', { name: 'Composer' })).toBeVisible();
});

for (const agent of ['Claude', 'Codex']) {
	test(`${agent} rewinds from the slash menu in the same Chat tab`, { tag: '@desktop' }, async ({ page }, { project }) => {
		const name = `rewind-${agent}-${project.name}`;
		mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
		await openProject(page, name);
		await newTab(page, `${agent} Rewind`);
		const tab = await pinned(page.getByRole('tab', { name: new RegExp(`^${agent} Rewind`) }));
		const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
		const composer = panel.getByRole('textbox', { name: 'Composer' });
		await composer.fill('Change the file');
		await panel.getByRole('button', { name: 'Send', exact: true }).click();
		await expect(tab.getByTitle('done')).toBeVisible();
		await expect(panel.locator('.user')).toHaveCount(1);
		await composer.fill('/rewind');
		await panel.getByRole('region', { name: 'Commands' }).getByRole('button', { name: /^\/rewind/ }).click();
		const picker = page.getByRole('dialog', { name: 'Choose where to rewind' });
		await picker.getByRole('button', { name: agent === 'Claude' ? /Change the file/ : /Greet the reader/ }).click();
		const review = page.getByRole('dialog', { name: 'Rewind before message 1?' });
		await expect(review).toContainText(agent === 'Claude' ? 'Restore checkpointed file edits' : 'Files are left unchanged');
		await expect(panel.locator('.user')).toHaveCount(1);
		await review.getByRole('button', { name: 'Rewind before message 1', exact: true }).click();
		await expect(review).toHaveCount(0);
		await expect(panel.getByText(agent === 'Claude' ? /^Selected message and everything after it removed; tracked edits restored/ : 'Selected turn and every later turn removed. Files are left unchanged.')).toBeVisible();
		await expect(panel.locator('.user')).toHaveCount(0);
		await expect(tab).toHaveAttribute('aria-selected', 'true');
		await closeProject(page, name);
	});

}
