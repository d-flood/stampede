import { execFileSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { Chat } from './chat.ts';
import { Director } from './record.ts';
import { configure, project } from './stage.ts';

// Project Terminals start $SHELL from the server's environment, which is set when the fixture spawns the server.
const shell = join(process.env.STAMPEDE_E2E_ROOT!, 'promo-shell');
mkdirSync(shell, { recursive: true });
writeFileSync(join(shell, 'bash'), `#!/bin/sh
printf '\\033]0;%s\\007' "\${PWD##*/}"
exec env -i HOME="\${PWD%/projects/*}" PATH=/usr/bin:/bin TERM=xterm-256color GIT_PAGER=cat PS1='\\W $ ' bash --norc --noprofile
`);
chmodSync(join(shell, 'bash'), 0o755);
const previous = process.env.SHELL;
process.env.SHELL = join(shell, 'bash');
test.afterAll(() => {
	process.env.SHELL = previous;
});

const prompt = 'Push the tags commit.';
const command = 'git push origin main';

test('an agent stages a command for review', async ({ browser, baseURL }) => {
	test.setTimeout(180_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	const path = project(root, 'tidepool');
	const git = (...args: string[]) => execFileSync('git', ['-C', path, ...args]);
	execFileSync('git', ['clone', '-q', '--bare', path, join(root, 'projects', 'tidepool.git')]);
	git('remote', 'add', 'origin', '../tidepool.git');
	git('fetch', '-q', 'origin');
	git('branch', '-q', '--set-upstream-to', 'origin/main');
	const notes = join(path, 'src/lib/notes.ts');
	writeFileSync(notes, readFileSync(notes, 'utf8').replace('\tpinned: boolean;\n\tupdated: number;', '\tpinned: boolean;\n\ttags: string[];\n\tupdated: number;').replace('pinned: false, updated: Date.now() };', 'pinned: false, tags: [], updated: Date.now() };'));
	git('commit', '-qam', 'Add tags to notes');
	git('tag', 'v0.5.0');

	const gates = join(root, 'gates');
	mkdirSync(gates);
	const fixture = join(root, 'terminals.jsonl');
	new Chat(path)
		.prompt()
		.say("Pushing needs your credentials, so I've staged it for you to review and run.")
		.stage(command, 'stage')
		.sleep(0.4)
		.end("It's waiting in a terminal.")
		.write(fixture);

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, [{ name: 'Claude', fixture, shortcut: 'c', gates }]);
	await desktop.goto('/');
	await desktop.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await desktop.getByRole('option', { name: /\/tidepool$/ }).click();
	await expect(desktop.getByRole('dialog')).toBeHidden();
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (await sidebar.isVisible()) await sidebar.getByRole('button', { name: 'Sidebar' }).click();
	await expect(sidebar).toBeHidden();
	await desktop.getByRole('button', { name: 'New Tab' }).click();
	await desktop.getByRole('menuitem', { name: 'Claude', exact: true }).click();
	const tab = desktop.getByRole('tab', { name: /^Claude/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const chatId = (await tab.getAttribute('data-tab'))!;
	const chat = desktop.getByRole('tabpanel').filter({ has: desktop.getByRole('textbox', { name: 'Composer' }) }).filter({ has: desktop.getByRole('button', { name: 'Send' }) });
	await desktop.mouse.move(900, 300);

	const director = new Director('terminals');
	await director.record('desktop', desktop);
	await director.hold(0.8);
	director.mark('desktop', 'start');
	await director.type('desktop', chat.getByRole('textbox', { name: 'Composer' }), prompt);
	await director.click('desktop', chat.getByRole('button', { name: 'Send' }), 0.5);
	director.mark('desktop', 'sent');
	await director.rest('desktop', desktop, 900, 300);
	await expect.poll(() => existsSync(join(gates, 'stage')), { timeout: 30_000 }).toBe(true);
	const output = execFileSync(process.env.STAMPEDE_E2E_BINARY!, ['chat-tools', '--data-dir', join(root, 'data'), '--chat-id', chatId], {
		input: `${JSON.stringify({ id: 1, method: 'tools/call', params: { name: 'stage_command', arguments: { command } } })}\n`,
		encoding: 'utf8'
	});
	const { terminal_id: id } = JSON.parse(output).result.structuredContent;
	unlinkSync(join(gates, 'stage'));
	await expect(desktop.locator(`[data-tab="${id}"]`)).toHaveAttribute('aria-selected', 'true');
	const terminal = desktop.getByRole('tabpanel').filter({ has: desktop.getByRole('button', { name: 'Run', exact: true }) });
	const composer = terminal.getByRole('textbox', { name: 'Composer' });
	await expect(composer).toHaveValue(command);
	director.mark('desktop', 'staged');
	await director.focus('desktop', 'staged', terminal.locator('.composer'));
	await director.hold(1.4);
	await director.click('desktop', composer, 0.5);
	await composer.press('End');
	await director.type('desktop', composer, ' v0.5.0');
	await director.hold(0.4);
	await director.click('desktop', terminal.getByRole('button', { name: 'Run', exact: true }), 0.5);
	director.mark('desktop', 'ran');
	await director.rest('desktop', desktop, 1180, 120);
	await expect(desktop.getByText('Review command before running')).toBeHidden();
	await expect.poll(() => execFileSync('git', ['-C', path, 'status', '-sb'], { encoding: 'utf8' })).not.toContain('ahead');
	const screen = (await desktop.locator('.panel[role="tabpanel"]:visible div.terminal').boundingBox())!;
	director.events.push({ t: Date.now() / 1000, device: 'desktop', type: 'focus', name: 'output', box: { x: screen.x, y: screen.y, width: 640, height: 200 } });
	await director.hold(2.4);
	director.mark('desktop', 'end');
	await director.finish();
});
