import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { pinned } from '../e2e/shell.ts';
import { Chat } from './chat.ts';
import { Codex } from './codex.ts';
import { Director } from './record.ts';
import { configure, project } from './stage.ts';

const prompt = 'Have Codex review the tags change before we commit.';
const task = 'Review the tags change in notes.ts\n\nNotes already in IndexedDB must keep working. Report any bug with its fix; do not edit files.';
const report = 'One bug. Notes saved before this change have no `tags` field, so `notesWithTag` and `allTags` throw on them. Treat a missing field as no tags: `note.tags ?? []`.';

const tags: [string, string][] = [
	['\tpinned: boolean;\n\tupdated: number;', '\tpinned: boolean;\n\ttags: string[];\n\tupdated: number;'],
	["pinned: false, updated: Date.now() };\n}", `pinned: false, tags: [], updated: Date.now() };
}

export function notesWithTag(notes: Note[], tag?: string) {
	return tag ? notes.filter((note) => note.tags.includes(tag)) : notes;
}

export function allTags(notes: Note[]) {
	return [...new Set(notes.flatMap((note) => note.tags))].sort();
}`]
];

test('Claude hands a review to Codex', async ({ browser, baseURL }) => {
	test.setTimeout(180_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	const path = project(root, 'tidepool');
	const notes = join(path, 'src/lib/notes.ts');
	writeFileSync(notes, tags.reduce((text, [before, after]) => text.replace(before, after), readFileSync(notes, 'utf8')));

	const codex = new Codex(path, task)
		.say("I'll read the tag helpers, then check notes saved before tags existed.")
		.plan([['Read the tags change', 'inProgress'], ['Check notes saved before tags', 'pending'], ['Report findings', 'pending']])
		.sleep(0.8)
		.read('src/lib/notes.ts')
		.sleep(1)
		.plan([['Read the tags change', 'completed'], ['Check notes saved before tags', 'inProgress'], ['Report findings', 'pending']])
		.say('`listNotes` returns notes exactly as stored, so older notes come back without `tags`.')
		.sleep(1.4)
		.plan([['Read the tags change', 'completed'], ['Check notes saved before tags', 'completed'], ['Report findings', 'inProgress']])
		.sleep(0.6)
		.end(report);
	const claude = new Chat(path)
		.prompt()
		.say("I'll ask Codex for a second opinion.")
		.delegate('Codex', task)
		.say('Codex caught a real one: older notes have no `tags`. Fixing both helpers.')
		.edit('src/lib/notes.ts', 'notes.filter((note) => note.tags.includes(tag))', 'notes.filter((note) => (note.tags ?? []).includes(tag))')
		.edit('src/lib/notes.ts', 'notes.flatMap((note) => note.tags)', 'notes.flatMap((note) => note.tags ?? [])')
		.say('Fixed. Notes saved before tags now count as untagged.')
		.end('Fixed.');
	const fixtures = { claude: join(root, 'delegate-claude.jsonl'), codex: join(root, 'delegate-codex.jsonl') };
	claude.write(fixtures.claude);
	codex.write(fixtures.codex);

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, [{ name: 'Claude', fixture: fixtures.claude, shortcut: 'c' }], { codex: fixtures.codex });
	await desktop.goto('/');
	await desktop.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await desktop.getByRole('option', { name: /\/tidepool$/ }).click();
	await expect(desktop.getByRole('dialog')).toBeHidden();
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (await sidebar.isVisible()) await sidebar.getByRole('button', { name: 'Sidebar' }).click();
	await expect(sidebar).toBeHidden();
	await desktop.getByRole('button', { name: 'New Tab' }).click();
	await desktop.getByRole('menuitem', { name: 'Claude', exact: true }).click();
	const tab = await pinned(desktop.getByRole('tab', { name: /^Claude/ }));
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const panel = desktop.locator('.panel[role="tabpanel"]:visible');
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await expect(composer).toBeVisible();
	await desktop.mouse.move(900, 300);

	const director = new Director('delegate');
	await director.record('desktop', desktop);
	await director.hold(0.8);
	director.mark('desktop', 'start');
	await director.type('desktop', composer, prompt);
	await director.click('desktop', panel.getByRole('button', { name: 'Send' }), 0.5);
	director.mark('desktop', 'sent');
	await director.rest('desktop', desktop, 900, 300);
	const line = panel.getByRole('button', { name: /Review the tags change/ });
	await expect(line).toBeVisible({ timeout: 30_000 });
	await director.focus('desktop', 'dock', line);
	await director.hold(1.2);
	await director.click('desktop', line, 0.6);
	director.mark('desktop', 'opened');
	const header = panel.locator('header').filter({ has: desktop.getByRole('navigation', { name: 'Breadcrumb' }) });
	await expect(header.getByRole('img', { name: 'Codex' })).toBeVisible();
	await director.focus('desktop', 'header', header);
	await director.rest('desktop', desktop, 1180, 420);
	const reported = panel.getByRole('region', { name: 'Report' });
	await expect(reported).toBeVisible({ timeout: 30_000 });
	await director.focus('desktop', 'report', reported);
	director.mark('desktop', 'reported');
	await director.hold(2.2);
	await director.click('desktop', panel.getByRole('button', { name: 'Back to Main' }), 0.6);
	director.mark('desktop', 'back');
	await director.rest('desktop', desktop, 1180, 420);
	await desktop.mouse.wheel(0, 6000);
	await expect(tab.getByTitle('done')).toBeVisible({ timeout: 30_000 });
	await director.hold(1.6);
	director.mark('desktop', 'end');
	await director.finish();
});
