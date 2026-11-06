import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { Chat } from './chat.ts';
import { Director } from './record.ts';
import { configure, project, seed } from './stage.ts';

test('Rigs are agents ready to launch', async ({ browser, baseURL }) => {
	test.setTimeout(120_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	project(root, 'tidepool');
	const fixture = join(root, 'idle.jsonl');
	new Chat(seed).write(fixture);
	const opus = new Chat(seed);
	(opus.entries[1].stdout as { response: { response: { current_permission_mode: string } } }).response.response.current_permission_mode = 'plan';
	opus.entries.push({ stdin: { type: 'control_request', request_id: 'settings', request: { subtype: 'get_settings' } } }, { stdout: { type: 'control_response', response: { subtype: 'success', request_id: 'settings', response: { applied: { model: 'claude-opus-5-5', effort: 'high' } } } } });
	opus.write(join(root, 'opus.jsonl'));

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, [
		{ name: 'Claude Opus', fixture: join(root, 'opus.jsonl'), model: 'opus', effort: 'high', mode: 'plan', shortcut: 'o' },
		{ name: 'Claude', fixture, shortcut: 'c' },
		{ name: 'Codex Review', agent: 'codex', fixture: 'simple-turn', model: 'gpt-6-sol', effort: 'xhigh', mode: 'read-only', shortcut: 'x' }
	]);
	await desktop.goto('/');
	await desktop.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await desktop.getByRole('option', { name: /\/tidepool$/ }).click();
	await expect(desktop.getByRole('dialog')).toBeHidden();
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) await desktop.getByRole('button', { name: 'Sidebar' }).click();
	await sidebar.getByRole('button', { name: 'README.md', exact: true }).click();
	await desktop.getByRole('radio', { name: 'Preview', exact: true }).click();
	await desktop.getByRole('button', { name: /^Settings/ }).click();
	const settings = desktop.getByRole('dialog', { name: 'Settings', exact: true });
	await settings.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', { name: /^Rigs/ }).click();
	const list = settings.getByRole('list', { name: 'Rigs', exact: true });
	const rig = (name: string) => list.getByRole('button', { name, exact: true });
	const model = settings.getByRole('combobox', { name: 'Model', exact: true });
	await expect(rig('Claude Opus')).toBeVisible();
	await desktop.mouse.move(1150, 300);

	const director = new Director('rigs');
	await director.record('desktop', desktop);
	await director.hold(0.8);
	director.mark('desktop', 'start');
	await director.click('desktop', rig('Claude Opus'), 0.6);
	await expect(model).toHaveValue('opus');
	await expect(settings.getByRole('combobox', { name: 'Permission mode', exact: true })).toHaveValue('plan');
	await director.focus('desktop', 'editor', list);
	await director.rest('desktop', desktop, 1150, 260);
	await director.hold(1.6);
	await director.click('desktop', rig('Codex Review'), 0.5);
	await expect(model).toHaveValue('gpt-6-sol');
	await expect(settings.getByRole('combobox', { name: 'Effort level', exact: true })).toHaveValue('xhigh');
	director.mark('desktop', 'codex');
	await director.rest('desktop', desktop, 1150, 260);
	await director.hold(1.6);
	await director.click('desktop', settings.getByRole('button', { name: 'Done', exact: true }), 0.5);
	await expect(settings).toBeHidden();
	director.mark('desktop', 'closed');

	await director.click('desktop', desktop.getByRole('button', { name: 'New Tab', exact: true }), 0.6);
	const menu = desktop.getByRole('menu');
	await expect(menu.getByRole('menuitem', { name: 'Codex Review' })).toBeVisible();
	await director.focus('desktop', 'menu', menu);
	await director.hold(1.6);
	await desktop.keyboard.press('Escape');
	await expect(menu).toBeHidden();
	await director.rest('desktop', desktop, 1150, 260);
	director.mark('desktop', 'leader');

	const hint = desktop.getByRole('status').filter({ hasText: 'Cancel' });
	director.events.push({ t: Date.now() / 1000, device: 'desktop', type: 'press', name: 'Ctrl + ;' });
	await desktop.keyboard.press('Control+Semicolon');
	await expect(hint).toBeVisible();
	await director.hold(0.6);
	director.events.push({ t: Date.now() / 1000, device: 'desktop', type: 'press', name: 'R' });
	await desktop.keyboard.press('r');
	await expect(hint).toContainText('Claude Opus');
	await director.focus('desktop', 'hint', hint);
	await director.hold(1);
	director.events.push({ t: Date.now() / 1000, device: 'desktop', type: 'press', name: 'O' });
	await desktop.keyboard.press('o');
	const tab = desktop.getByRole('tab', { name: /^Claude Opus/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	director.mark('desktop', 'launched');
	const chips = desktop.getByRole('tabpanel').filter({ has: desktop.getByRole('textbox', { name: 'Composer' }) }).locator('.chips');
	await expect(chips.getByRole('button', { name: /^Mode: plan/i })).toBeVisible();
	await director.focus('desktop', 'chips', chips);
	await director.hold(1.6);
	director.mark('desktop', 'end');
	await director.finish();
});
