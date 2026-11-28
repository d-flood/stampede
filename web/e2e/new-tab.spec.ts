import { expect, test } from './fixtures.ts';
import { mkdirSync, writeFileSync } from 'node:fs';
import { openProject } from './shell.ts';

test.use({ starterRigs: true });

test('New Tab shows session loading and fits the loaded sessions', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const item = phone ? 'button' : 'menuitem';
	let release: (() => void) | undefined;
	let requests = 0;
	page.on('websocket', (socket) => socket.on('framesent', ({ payload }) => {
		if (typeof payload === 'string' && JSON.parse(payload).type === 'sessions.list') requests++;
	}));
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((message) => {
			if (typeof message === 'string') {
				const data = JSON.parse(message);
				if (data.type === 'sessions.listed') {
					release = () => socket.send(JSON.stringify({ ...data, sessions: [{ key: { agent: 'codex', store: '/codex', sessionId: 'past-session' }, project: { id: data.projectId, path: '/project' }, title: 'Continue investigating the manuscript transcription workflow', updatedAt: Date.now() }] }));
					return;
				}
			}
			socket.send(message);
		});
	});
	await openProject(page, 'alpha');
	if (!phone) await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	const menu = phone ? page.getByRole('region', { name: 'alpha', exact: true }) : page.getByRole('menu');
	await expect(menu.getByRole('status')).toHaveText('Reading sessions…');
	await expect(menu.getByRole(item, { name: 'Terminal', exact: true })).toBeEnabled();
	await expect.poll(() => release !== undefined).toBe(true);
	expect(requests).toBe(1);
	release!();
	await expect(menu.getByRole('status')).toHaveCount(0);
	await expect(menu.getByRole(phone ? 'option' : 'menuitem', { name: /Continue investigating/ })).toBeVisible();
	await expect.poll(() => menu.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		return bounds.left >= 0 && bounds.right <= innerWidth;
	})).toBe(true);
	await page.keyboard.press('Escape');
	release = undefined;
	await page.keyboard.press('Control+T');
	const picker = page.getByRole('dialog', { name: 'New Tab', exact: true });
	await expect(picker.getByRole('status')).toHaveText('Reading sessions…');
	await expect.poll(() => release !== undefined).toBe(true);
	release!();
	await expect(picker.getByRole('status')).toHaveCount(0);
	await expect(picker.getByRole('combobox')).toBeFocused();
});

test('the Launchpad offers the New Tab items and comes back focused when the last Tab closes', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const created: Record<string, unknown>[] = [];
	page.on('websocket', (socket) => socket.on('framesent', ({ payload }) => {
		const message = typeof payload === 'string' && JSON.parse(payload);
		if (message?.type === 'chat.create') created.push(message);
	}));
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((message) => {
			if (typeof message === 'string' && JSON.parse(message).type === 'sessions.listed') {
				const data = JSON.parse(message);
				socket.send(JSON.stringify({ ...data, sessions: [{ key: { agent: 'codex', store: '/codex', sessionId: 'past-session' }, project: { id: data.projectId, path: '/project' }, title: 'Continue the collation', updatedAt: Date.now() }] }));
				return;
			}
			socket.send(message);
		});
	});
	await openProject(page, 'alpha');
	const launchpad = page.getByRole('region', { name: 'alpha', exact: true });
	await expect(launchpad.getByText(/^main · (clean|\d+ changed files?)$/)).toBeVisible();
	const rigs = ['Claude Code', 'Codex'];
	const tools = ['Terminal', 'Browser', 'Open file', 'Continue the collation', 'Search sessions…'];
	for (const label of tools.slice(0, 3)) await expect(launchpad.getByRole('button', { name: new RegExp(`^${label}`) })).toBeVisible();
	const recent = launchpad.getByRole('region', { name: 'Recent sessions' });
	await expect(recent.getByRole('combobox', { name: 'Search sessions' })).toBeVisible();
	await expect(recent.getByRole('option', { name: /^Continue the collation/ })).toBeVisible();
	const chip = launchpad.getByRole('button', { name: 'Rig: Claude Code', exact: true });
	await chip.click();
	const choices = page.getByRole('menu').getByRole('menuitemcheckbox');
	await expect(choices).toHaveCount(rigs.length);
	for (const [index, label] of rigs.entries()) await expect(choices.nth(index)).toHaveAccessibleName(label);
	await page.keyboard.press('Escape');

	if (!phone) {
		await page.getByRole('button', { name: 'New Tab', exact: true }).click();
		const items = page.getByRole('menu').getByRole('menuitem');
		await expect(items).toHaveCount(rigs.length + tools.length);
		for (const [index, label] of [...rigs, ...tools].entries()) await expect(items.nth(index)).toHaveAccessibleName(label);
		await page.keyboard.press('Escape');
	}

	const mode = launchpad.getByRole('button', { name: /^Mode: / });
	await mode.click();
	const other = page.getByRole('menu').getByRole('menuitemcheckbox', { checked: false }).last();
	const label = (await other.textContent())!.trim();
	await other.click();
	await expect(mode).toHaveAccessibleName(`Mode: ${label}`);
	const value = await page.evaluate(() => fetch('/api/modes').then((response) => response.json()));

	const composer = launchpad.getByRole('textbox', { name: 'Composer' });
	await composer.fill('Start from the Launchpad');
	await launchpad.getByRole('button', { name: 'Send', exact: true }).click();
	await expect(launchpad).toHaveCount(0);
	await expect(page.getByRole('region', { name: 'User message 1', exact: true })).toContainText('Start from the Launchpad');
	expect(created.at(-1)?.settings).toEqual({ mode: value['claude-code'].find((mode: { label: string }) => mode.label === label).value });
	await page.getByRole('tablist', { name: 'Tabs' }).getByRole('button', { name: /^Close / }).click();
	await expect(launchpad).toBeVisible();
	if (!phone) await expect(composer).toBeFocused();
});

test('a Launchpad without Rigs offers Create a Rig, which opens Rigs settings', async ({ page }) => {
	await page.route('**/api/rigs', (route) => route.request().method() === 'GET' ? route.fulfill({ json: [] }) : route.fallback());
	await openProject(page, 'alpha');
	const launchpad = page.getByRole('region', { name: 'alpha', exact: true });
	await expect(launchpad.getByRole('button', { name: /^Terminal/ })).toBeVisible();
	await expect(launchpad.getByRole('button', { name: /^Browser/ })).toBeVisible();
	await expect(launchpad.getByRole('button', { name: /^Open file/ })).toBeVisible();
	await launchpad.getByRole('button', { name: /^Create a Rig/ }).click();
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	await expect(settings.getByRole('heading', { name: 'Rigs', exact: true })).toBeVisible();
});

test.describe('with the fake Agents', () => {
	test.use({ starterRigs: false });

	test('a found Session previews its matching turn and opens as a Dormant Chat at the match', { tag: '@desktop' }, async ({ page }, { project }) => {
		const name = `preview-${project.name}`;
		const path = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
		const sessions = `${process.env.STAMPEDE_E2E_ROOT}/claude/projects/${path.replace(/[^a-zA-Z0-9]/g, '-')}`;
		mkdirSync(path, { recursive: true });
		mkdirSync(sessions, { recursive: true });
		const sessionId = '3b0c5f7e-2d4a-4c6b-9e1f-8a7d6c5b4a39';
		const filler = 'Each witness carries its own readings into the apparatus, and the stemma has to account for every one of them. '.repeat(8);
		const entry = (type: string, uuid: string, parentUuid: string | null, message: object) => JSON.stringify({ type, uuid, parentUuid, isSidechain: false, cwd: path, sessionId, timestamp: new Date().toISOString(), message });
		const lines = Array.from({ length: 30 }, (_, turn) => [
			entry('user', `u${turn}`, turn ? `a${turn - 1}` : null, { role: 'user', content: `Question ${turn + 1}` }),
			entry('assistant', `a${turn}`, `u${turn}`, { id: `m${turn}`, role: 'assistant', content: [{ type: 'text', text: turn === 3 ? `The colophon names a quokka as scribe. ${filler}` : filler }], stop_reason: 'end_turn' })
		]).flat();
		writeFileSync(`${sessions}/${sessionId}.jsonl`, [...lines, JSON.stringify({ type: 'custom-title', customTitle: 'Witness survey', sessionId })].join('\n') + '\n');
		await openProject(page, name);

		const recent = page.getByRole('region', { name, exact: true }).getByRole('region', { name: 'Recent sessions' });
		await recent.getByRole('combobox', { name: 'Search sessions' }).fill('quokka');
		await expect(recent.getByRole('option')).toHaveText([/Witness survey/]);
		const preview = recent.getByRole('region', { name: 'Session preview' });
		await expect(preview.getByRole('region', { name: /^User message/ })).toHaveText([/Question 3/, /Question 4/, /Question 5/]);
		await expect(preview.locator('.outlined')).toHaveText(/^The colophon names a quokka/);
		const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
		await expect(panel).toHaveCount(0);

		await page.keyboard.press('Enter');
		await expect(panel.getByText(/^The colophon names a quokka/)).toBeInViewport();
		await expect(panel.getByText('Question 30')).not.toBeInViewport();
		const marked = () => page.evaluate(() => [...(CSS.highlights.get('landing') ?? [])].map((range) => (range as Range).toString()));
		await expect.poll(marked).toEqual(['quokka']);

		const composer = panel.getByRole('textbox', { name: 'Composer' });
		await expect(composer).toHaveAttribute('placeholder', 'Message to resume this Session');
		await composer.fill('Greet the reader');
		await panel.getByRole('button', { name: 'Send' }).click();
		await expect(panel.getByText(/^Hello and welcome!/)).toBeVisible();
		await expect.poll(marked).toEqual([]);
	});
});
