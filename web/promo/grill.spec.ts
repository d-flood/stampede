import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { pinned } from '../e2e/shell.ts';
import { Chat, type Question } from './chat.ts';
import { brief, tickets } from './offline.ts';
import { Director } from './record.ts';
import { configure, project, seed } from './stage.ts';

const plan = 'Tidepool should work offline';

const conflicts: Question = {
	header: 'Conflicts',
	question: 'An offline edit syncs after the note changed elsewhere. What wins?',
	multiSelect: false,
	options: [
		{ label: 'Newest edit per field (Recommended)', description: 'Both edits survive when they touch different fields' },
		{ label: 'Last device to sync', description: 'Simpler, but it can drop an edit' }
	]
};

const banner: Question = {
	header: 'Banner',
	question: 'Should people see that they are offline?',
	multiSelect: false,
	options: [
		{ label: 'Banner with queued edits (Recommended)', description: 'Offline · 3 edits waiting' },
		{ label: 'No indicator', description: 'Edits sync quietly in the background' }
	]
};

const setup: Question = {
	header: 'Setup',
	question: 'Record ./scripts/setup as the Setup Command? It ran clean in a fresh clone.',
	multiSelect: false,
	options: [{ label: 'Record it' }, { label: 'Change it' }]
};

function conversation(root: string) {
	const bin = process.env.STAMPEDE_E2E_BINARY!;
	const data = ['--data-dir', join(root, 'data')];
	writeFileSync(join(root, 'brief.md'), brief);
	const chat = new Chat(seed)
		.prompt()
		.say("I'll grill you on the plan. First round:")
		.ask([conflicts, 'Newest edit per field (Recommended)'], [banner, 'Banner with queued edits (Recommended)'])
		.say('Settled: the newest edit wins per field, and a banner counts queued edits. Send `/create-epic` when you are ready.')
		.end('Settled.')
		.prompt()
		.say('Writing the Brief.')
		.run(bin, 'epic', 'add', 'offline', '--title', 'Work offline', '--brief', join(root, 'brief.md'), ...data)
		.bash(`stampede epic add offline --title "Work offline" --brief - <<'EOF'\n${brief}EOF`, 'Create the Epic', 'Added Epic offline to the Project at ~/projects/tidepool/.git.')
		.say('Created the Epic **offline**.')
		.end('Created offline.')
		.prompt();
	chat.entries.push({ gate: 'opened' });
	chat.say('Slicing the Brief into Tickets, in dependency order.');
	tickets.forEach((ticket, index) => {
		const number = String(index + 1);
		const body = join(root, `ticket-${number}.md`);
		writeFileSync(body, `${ticket.body}\n`);
		const blockers = ticket.blockers.length ? ['--blocked-by', ticket.blockers.join(',')] : [];
		chat.run(bin, 'ticket', 'add', 'offline', number, '--title', ticket.title, '--tags', ticket.tags, ...blockers, '--body', body, ...data)
			.bash(`stampede ticket add offline ${number} --title "${ticket.title}" --tags ${ticket.tags}${blockers.length ? ` --blocked-by ${blockers[1]}` : ''} --body - <<'EOF'\n${ticket.body}\nEOF`, `Add Ticket ${number}`, `Added offline/${number}.`);
	});
	return chat
		.bash('git clone --no-hardlinks . ../tidepool-check && cd ../tidepool-check && ./scripts/setup', 'Try the Setup Command in a fresh clone', 'Worktree ready')
		.ask([setup, 'Record it'])
		.run(bin, 'epic', 'set-setup', 'offline', './scripts/setup', ...data)
		.bash('stampede epic set-setup offline ./scripts/setup', 'Record the Setup Command', 'Recorded the Setup Command of offline.')
		.say('Five Tickets are in, with their Blockers. The Epic is ready to Run.')
		.end('Ready to Run.');
}

test('grill and create-epic build an Epic', async ({ browser, baseURL }) => {
	test.setTimeout(240_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	project(root, 'tidepool');
	const fixture = join(root, 'grill.jsonl');
	const gates = join(root, 'gates');
	mkdirSync(gates);
	conversation(root).write(fixture);

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, [{ name: 'Claude', fixture, shortcut: 'c', gates }]);
	await desktop.goto('/');
	await desktop.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await desktop.getByRole('option', { name: /\/tidepool$/ }).click();
	await expect(desktop.getByRole('dialog')).toBeHidden();
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) await desktop.getByRole('button', { name: 'Sidebar' }).click();
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	const epics = sidebar.getByRole('region', { name: 'Epics' });
	await expect(epics).toContainText('No Epics yet');
	await desktop.getByRole('button', { name: 'New Tab' }).first().click();
	await desktop.getByRole('menuitem', { name: 'Claude', exact: true }).click();
	const tab = await pinned(desktop.getByRole('tab', { name: /^Claude/ }));
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const panel = desktop.getByRole('tabpanel').filter({ has: desktop.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await desktop.mouse.move(640, 400);

	const director = new Director('grill');
	await director.record('desktop', desktop);
	await director.hold(0.8);
	director.mark('desktop', 'start');
	await director.type('desktop', composer, '/');
	const menu = panel.getByRole('region', { name: 'Commands' });
	await expect(menu).toBeVisible();
	await director.focus('desktop', 'commands', menu);
	await director.hold(0.6);
	await director.click('desktop', menu.getByRole('button', { name: /^\/grill/ }), 0.5);
	await expect(composer).toHaveValue('/grill ');
	await composer.press('End');
	await director.type('desktop', composer, plan);
	await director.click('desktop', panel.getByRole('button', { name: 'Send' }), 0.5);
	director.mark('desktop', 'sent');
	await director.rest('desktop', desktop, 900, 300);

	const question = panel.getByRole('region', { name: 'Question', exact: true });
	await expect(question).toBeVisible({ timeout: 30_000 });
	await director.focus('desktop', 'question', question);
	await director.hold(0.6);
	await director.click('desktop', question.getByRole('radio', { name: /^Newest edit/ }), 0.5);
	await director.click('desktop', question.getByRole('radio', { name: /^Banner with/ }), 0.5);
	await director.click('desktop', question.getByRole('button', { name: 'Send answers', exact: true }), 0.4);
	director.mark('desktop', 'answered');
	await director.rest('desktop', desktop, 900, 300);
	await desktop.mouse.wheel(0, 6000);
	await expect(tab.getByTitle('done')).toBeVisible({ timeout: 30_000 });

	director.mark('desktop', 'spec');
	await director.type('desktop', composer, '/create-epic');
	await composer.press('Enter');
	await expect(composer).toHaveValue('/create-epic ');
	await director.click('desktop', panel.getByRole('button', { name: 'Send' }), 0.5);
	director.mark('desktop', 'specced');
	await director.rest('desktop', desktop, 900, 300);
	const card = epics.getByRole('button', { name: /^offline/ });
	await expect(card).toBeVisible({ timeout: 30_000 });
	await director.focus('desktop', 'card', card);
	director.mark('desktop', 'created');
	await expect.poll(() => existsSync(join(gates, 'opened')), { timeout: 30_000 }).toBe(true);
	director.mark('desktop', 'specified');
	await director.hold(0.3);
	await director.click('desktop', card, 0.6);
	const epic = desktop.getByRole('tabpanel', { name: 'Epic offline' });
	await expect(epic).toBeVisible();
	await director.hold(0.3);
	director.keys('desktop', 'Ctrl + ; + S');
	await desktop.keyboard.press('Control+;');
	await desktop.keyboard.press('s');
	await expect(desktop.getByRole('tablist', { name: 'Right Tabs' }).getByRole('tab', { name: 'Epic offline' })).toBeVisible();
	await director.click('desktop', sidebar.getByRole('button', { name: 'Sidebar' }), 0.5);
	await expect(sidebar).toBeHidden();
	const divider = await director.point('desktop', desktop.getByRole('separator'), 0.5);
	const [x, y] = [divider.x + divider.width / 2, divider.y + divider.height / 2];
	await desktop.mouse.move(x, y);
	await desktop.mouse.down();
	director.events.push({ t: Date.now() / 1000, device: 'desktop', type: 'click' }, { t: Date.now() / 1000 + 0.05, device: 'desktop', type: 'move', x: x - 140, y });
	for (let step = 1; step <= 20; step++) {
		await desktop.mouse.move(x - 140 * (1 - (1 - step / 20) ** 2), y);
		await desktop.waitForTimeout(35);
	}
	await desktop.mouse.up();
	director.mark('desktop', 'split');
	await director.hold(0.4);
	rmSync(join(gates, 'opened'));
	director.mark('desktop', 'ticketing');
	await director.rest('desktop', desktop, 1000, 300);
	const rows = epic.getByRole('region', { name: 'Tickets' });
	await expect(rows).toContainText('Cache the app shell', { timeout: 30_000 });
	await director.focus('desktop', 'tickets', rows);
	await expect(rows).toContainText('Resolve conflicting edits', { timeout: 30_000 });
	director.mark('desktop', 'sliced');
	const confirm = panel.getByRole('region', { name: 'Question', exact: true });
	await expect(confirm).toBeVisible({ timeout: 30_000 });
	await director.focus('desktop', 'setup', confirm);
	await director.hold(0.5);
	await director.click('desktop', confirm.getByRole('radio', { name: /^Record it/ }), 0.5);
	await director.click('desktop', confirm.getByRole('button', { name: 'Send answer', exact: true }), 0.4);
	director.mark('desktop', 'confirmed');
	await director.rest('desktop', desktop, 1000, 300);
	const run = epic.getByRole('region', { name: 'Run', exact: true });
	await expect(run.getByRole('button', { name: 'Start Run' })).toBeEnabled({ timeout: 30_000 });
	await director.focus('desktop', 'run', run);
	await expect(tab.getByTitle('done')).toBeVisible({ timeout: 30_000 });
	director.mark('desktop', 'ready');
	await director.hold(1.8);
	director.mark('desktop', 'end');
	await director.finish();
});
