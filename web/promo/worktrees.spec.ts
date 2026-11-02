import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { Chat } from './chat.ts';
import { Director } from './record.ts';
import { configure, project, seed } from './stage.ts';

const branch = 'try/dark-mode';

test('a worktree is a Project of its own', async ({ browser, baseURL }) => {
	test.setTimeout(180_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	project(root, 'tidepool');
	const fixture = join(root, 'worktree.jsonl');
	new Chat(seed)
		.prompt()
		.say("I'll try a darker palette on this branch, so `main` stays untouched.")
		.read('src/app.css')
		.edit('src/app.css', '\t\t--surface: #111315;\n\t\t--surface-raised: #1b1e21;', '\t\t--surface: #0b0d0f;\n\t\t--surface-raised: #15181b;')
		.say('Darkened the surfaces. Compare it with `main` whenever you like.')
		.end('Darkened.')
		.write(fixture);

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, [{ name: 'Claude', fixture, shortcut: 'c' }]);
	await desktop.goto('/');
	const rail = desktop.getByRole('navigation', { name: 'Projects' });
	await rail.getByRole('button', { name: 'Open Project' }).click();
	await desktop.getByRole('option', { name: /\/tidepool$/ }).click();
	await expect(desktop.getByRole('dialog')).toBeHidden();
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) await desktop.getByRole('button', { name: 'Sidebar' }).click();
	await sidebar.getByRole('button', { name: 'README.md', exact: true }).click();
	await desktop.getByRole('button', { name: 'Preview', exact: true }).click();
	await desktop.mouse.move(700, 420);

	const director = new Director('worktrees');
	await director.record('desktop', desktop);
	await director.hold(0.8);
	director.mark('desktop', 'start');
	const parent = rail.getByRole('button', { name: 'tidepool', exact: true });
	await director.point('desktop', parent, 0.6);
	director.events.push({ t: Date.now() / 1000, device: 'desktop', type: 'click' });
	await parent.click({ button: 'right' });
	const item = desktop.getByRole('menuitem', { name: 'New worktree…' });
	await expect(item).toBeVisible();
	await director.focus('desktop', 'menu', desktop.getByRole('menu'));
	await director.hold(0.3);
	await director.click('desktop', item, 0.5);
	const dialog = desktop.getByRole('dialog', { name: 'New worktree' });
	await expect(dialog.getByRole('combobox', { name: 'From' })).toHaveValue('main');
	await director.focus('desktop', 'dialog', dialog);
	await director.type('desktop', dialog.getByRole('textbox', { name: 'New branch' }), branch);
	await director.click('desktop', dialog.getByRole('button', { name: 'Create' }), 0.5);
	await expect(dialog).toBeHidden();
	director.mark('desktop', 'created');
	const row = rail.getByRole('listitem').filter({ has: desktop.getByRole('button', { name: 'tidepool', exact: true }) });
	const tile = row.getByRole('group', { name: 'Worktrees of tidepool' }).getByRole('button', { name: branch, exact: true });
	await expect(tile).toHaveAttribute('aria-current', 'true');
	await director.focus('desktop', 'rail', row);
	await director.hold(0.8);

	director.keys('desktop', 'Ctrl + Shift + 1');
	await desktop.keyboard.press('Control+Shift+1');
	await director.hold(0.5);
	const panel = desktop.getByRole('tabpanel').filter({ has: desktop.getByRole('textbox', { name: 'Composer' }) });
	await director.type('desktop', panel.getByRole('textbox', { name: 'Composer' }), 'Try a darker palette');
	await director.click('desktop', panel.getByRole('button', { name: 'Send' }), 0.4);
	director.mark('desktop', 'sent');
	await director.rest('desktop', desktop, 900, 330);
	const tab = desktop.getByRole('tab', { name: /^Claude/ });
	await expect(tab.getByTitle('done')).toBeVisible({ timeout: 30_000 });
	await director.focus('desktop', 'chat', panel);
	await director.hold(1.2);
	director.mark('desktop', 'end');
	await director.finish();
});
