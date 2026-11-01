import { devices } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { showScreen } from '../e2e/shell.ts';
import { Chat, type Question } from './chat.ts';
import { Director, footage } from './record.ts';
import { configure, project, seed } from './stage.ts';

const prompt = 'Add tags to notes. I want to tag a note and filter the list by tag.';

const mockup = `<style>
body { margin: 0; font: 15px/1.4 system-ui, sans-serif; background: #171a1e; color: #e8e6e3 }
.wrap { display: grid; grid-template-columns: 180px 1fr; height: 210px }
aside { border-right: 1px solid #2b2f35; padding: 14px 12px }
.chips, .tags { display: flex; flex-wrap: wrap; gap: 6px; align-items: center }
.chips { margin-bottom: 12px }
.chip { padding: 3px 10px; border-radius: 999px; background: #262b31; color: #b9c2cc; font-size: 13px }
.on { background: #0e7490; color: #ecfeff }
.item { padding: 7px 8px; border-radius: 8px; font-size: 14px }
.selected { background: #262b31 }
main { padding: 18px 22px }
h1 { margin: 0 0 10px; font: 600 22px Georgia, serif }
.add { border: 1px dashed #3a4048; border-radius: 999px; padding: 3px 10px; color: #7d8794; font-size: 13px }
p { color: #9aa3ad; margin: 14px 0 0 }
</style>
<div class="wrap">
<aside><div class="chips"><span class="chip on">#coast</span><span class="chip">#recipes</span><span class="chip">#work</span></div><div class="item selected">Tide tables</div><div class="item">Beach day</div></aside>
<main><h1>Tide tables</h1><div class="tags"><span class="chip on">#coast</span><span class="chip">#weekend</span><span class="add">+ Add tag</span></div><p>Low tide 6:40 · high tide 12:55</p></main>
</div>`;

const filter: Question = {
	header: 'Filter',
	question: 'Where should the tag filter live?',
	multiSelect: false,
	options: [
		{ label: 'Sidebar chips', description: 'One chip per tag above the note list', preview: '#coast  #recipes  #work\n──────────────────────\nTide tables\nBeach day' },
		{ label: 'Search syntax', description: 'Type #coast in the search box', preview: 'Search: #coast tide\n──────────────────────\nTide tables' }
	]
};

const themes = ['Catppuccin Mocha', 'Rose Pine Dawn', 'TokyoNight Night', 'Gruvbox Dark', 'Flexoki Light', 'Kanagawa Wave', 'Dracula'];

function conversation() {
	return new Chat(seed)
		.prompt()
		.say("I'll map how notes are stored and searched first.")
		.agent('Map note storage and search', 'Explore', 'Find where Tidepool stores, lists and filters notes, and report the functions involved.', (scout) => scout
			.read('src/lib/notes.ts')
			.sleep(0.7)
			.grep('searchNotes', ['src/lib/notes.ts', 'src/routes/+page.svelte', 'src/lib/notes.test.ts'])
			.sleep(0.7)
			.read('src/routes/+page.svelte')
			.sleep(0.4)
			.say('Notes live in IndexedDB through `idb`. `listNotes` sorts pinned notes first and `searchNotes` filters in memory; `+page.svelte` calls both.'),
		'Notes live in IndexedDB through `idb`. `listNotes` sorts pinned notes first and `searchNotes` filters in memory; `+page.svelte` calls both.')
		.todos([['Add a tags field to notes', 'in_progress'], ['Tag chips in the editor', 'pending'], ['Filter the list by tag', 'pending'], ['Cover tags in tests', 'pending']])
		.say('Here is what I have in mind for the editor:')
		.html('Tag chips', mockup)
		.sleep(1.2)
		.ask(filter, 'Sidebar chips')
		.say('Sidebar chips it is. Adding the field, the chips, and the filter.')
		.edit('src/lib/notes.ts', '\tpinned: boolean;\n\tupdated: number;', '\tpinned: boolean;\n\ttags: string[];\n\tupdated: number;')
		.edit('src/lib/notes.ts', "pinned: false, updated: Date.now() };\n}", `pinned: false, tags: [], updated: Date.now() };
}

export function notesWithTag(notes: Note[], tag?: string) {
	return tag ? notes.filter((note) => note.tags.includes(tag)) : notes;
}

export function allTags(notes: Note[]) {
	return [...new Set(notes.flatMap((note) => note.tags))].sort();
}`)
		.todos([['Add a tags field to notes', 'completed'], ['Tag chips in the editor', 'in_progress'], ['Filter the list by tag', 'pending'], ['Cover tags in tests', 'pending']])
		.edit('src/lib/components/NoteEditor.svelte', '\t\ttimer = setTimeout(() => saveNote(note), 400);\n\t}', `\t\ttimer = setTimeout(() => saveNote(note), 400);
	}

	function addTag(event: KeyboardEvent & { currentTarget: HTMLInputElement }) {
		const tag = event.currentTarget.value.trim().toLowerCase();
		if (event.key !== 'Enter' || !tag || note.tags.includes(tag)) return;
		note.tags = [...note.tags, tag];
		event.currentTarget.value = '';
		changed();
	}`)
		.edit('src/lib/components/NoteEditor.svelte', '\t</header>\n', `\t</header>
	<ul class="tags">
		{#each note.tags as tag (tag)}
			<li>#{tag}</li>
		{/each}
		<li><input placeholder="Add tag" onkeydown={addTag} aria-label="Add tag" /></li>
	</ul>
`)
		.todos([['Add a tags field to notes', 'completed'], ['Tag chips in the editor', 'completed'], ['Filter the list by tag', 'in_progress'], ['Cover tags in tests', 'pending']])
		.edit('src/routes/+page.svelte', `	import { createNote, listNotes, saveNote, searchNotes, type Note } from '$lib/notes';

	let notes = $state<Note[]>([]);
	let query = $state('');
	let selected = $state<string>();

	const visible = $derived(searchNotes(notes, query));`, `	import { allTags, createNote, listNotes, notesWithTag, saveNote, searchNotes, type Note } from '$lib/notes';

	let notes = $state<Note[]>([]);
	let query = $state('');
	let selected = $state<string>();
	let tag = $state<string>();

	const visible = $derived(searchNotes(notesWithTag(notes, tag), query));`)
		.edit('src/routes/+page.svelte', '\t\t<input type="search" bind:value={query} placeholder="Search notes" />\n', `\t\t<input type="search" bind:value={query} placeholder="Search notes" />
		<nav class="tags" aria-label="Tags">
			{#each allTags(notes) as name (name)}
				<button class:active={name === tag} onclick={() => (tag = tag === name ? undefined : name)}>#{name}</button>
			{/each}
		</nav>
`)
		.todos([['Add a tags field to notes', 'completed'], ['Tag chips in the editor', 'completed'], ['Filter the list by tag', 'completed'], ['Cover tags in tests', 'in_progress']])
		.edit('src/lib/notes.test.ts', "\tit('matches every search term', () => {", `\tit('filters by tag', () => {
		const notes = [{ ...note('Tide tables'), tags: ['coast'] }, note('Groceries')];
		expect(notesWithTag(notes, 'coast').map((n) => n.title)).toEqual(['Tide tables']);
	});

	it('matches every search term', () => {`)
		.edit('src/lib/notes.test.ts', 'import { createNote, deleteNote, listNotes, saveNote, searchNotes } from', 'import { createNote, deleteNote, listNotes, notesWithTag, saveNote, searchNotes } from')
		.say('Now the tests.')
		.approve('pnpm test src/lib/notes.test.ts', 'Run the notes tests', 'pnpm test:*', ' ✓ src/lib/notes.test.ts (7 tests) 41ms\n\n Test Files  1 passed (1)\n      Tests  7 passed (7)\n   Start at  09:41:12\n   Duration  612ms')
		.todos([['Add a tags field to notes', 'completed'], ['Tag chips in the editor', 'completed'], ['Filter the list by tag', 'completed'], ['Cover tags in tests', 'completed']])
		.say('Tags are in. Notes carry a `tags` list, the editor adds a tag on **Enter**, and chips above the list filter by tag. All 7 note tests pass.')
		.end('Tags are in.');
}

async function settle(page: import('@playwright/test').Page, before: string) {
	const color = () => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
	await expect.poll(color).not.toBe(before);
	let previous = '';
	await expect.poll(async () => {
		const current = await color();
		const stable = current === previous;
		previous = current;
		return stable;
	}, { intervals: [200, 200, 200, 400] }).toBe(true);
	return previous;
}

test('chat, phone approval, review and themes', async ({ browser, baseURL }) => {
	test.setTimeout(240_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	project(root, 'tidepool');
	const fixture = join(root, 'chat.jsonl');
	conversation().write(fixture);

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	const phone = await (await browser.newContext({ ...devices['Pixel 8 Pro'], baseURL, viewport: { width: 448, height: 998 }, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, [{ name: 'Claude', fixture, shortcut: 'c' }]);

	await desktop.goto('/');
	await desktop.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await desktop.getByRole('option', { name: /\/tidepool$/ }).click();
	await expect(desktop.getByRole('dialog')).toBeHidden();
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) await desktop.getByRole('button', { name: 'Sidebar' }).click();
	for (const name of ['src', 'lib']) await sidebar.getByRole('button', { name, exact: true }).click();
	await sidebar.getByRole('button', { name: 'notes.ts', exact: true }).click();
	await expect(desktop.getByRole('tab', { name: 'notes.ts' })).toHaveAttribute('aria-selected', 'true');
	await sidebar.getByRole('button', { name: 'Changes', exact: true }).click();

	await phone.goto('/');
	await showScreen(phone, 'projects');
	await phone.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'tidepool', exact: true }).click();
	await showScreen(phone, 'tabs');
	await desktop.mouse.move(640, 400);

	const director = new Director('chat');
	await director.record('desktop', desktop);
	await director.record('phone', phone);
	await director.hold(1);
	director.mark('desktop', 'start');
	await director.click('desktop', desktop.getByRole('button', { name: 'New Tab' }));
	await director.click('desktop', desktop.getByRole('menuitem', { name: 'Claude', exact: true }), 0.5);
	const tab = desktop.getByRole('tab', { name: /^Claude/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const panel = desktop.getByRole('tabpanel').filter({ has: desktop.getByRole('textbox', { name: 'Composer' }) });
	await director.hold(0.4);
	await director.type('desktop', panel.getByRole('textbox', { name: 'Composer' }), prompt);
	await director.click('desktop', panel.getByRole('button', { name: 'Send' }), 0.5);
	director.mark('desktop', 'sent');
	await director.rest('desktop', desktop, 1000, 330);
	const scout = panel.getByRole('region', { name: 'Subagents' });
	await expect(scout.getByRole('button', { name: /^Map note storage/ })).toBeVisible({ timeout: 30_000 });
	await director.focus('desktop', 'subagent', scout);
	await expect(panel.getByRole('button', { name: /^1 Subagent · done/ })).toBeVisible({ timeout: 30_000 });
	director.mark('desktop', 'scouted');

	const html = panel.getByRole('region', { name: 'Tag chips' });
	await expect(html).toBeVisible({ timeout: 30_000 });
	await expect.poll(async () => (await html.boundingBox())?.height ?? 0).toBeGreaterThan(150);
	await director.focus('desktop', 'mockup', html);
	const question = panel.getByRole('region', { name: 'Question', exact: true });
	await expect(question).toBeVisible({ timeout: 30_000 });
	await director.focus('desktop', 'question', question);
	await director.hold(1.2);
	await director.click('desktop', question.getByRole('button', { name: /^Sidebar chips/ }), 0.6);
	await director.hold(0.5);
	await director.click('desktop', question.getByRole('button', { name: 'Submit', exact: true }), 0.5);
	director.mark('desktop', 'answered');
	await director.rest('desktop', desktop, 1000, 330);
	await desktop.mouse.wheel(0, 6000);

	const card = panel.getByRole('region', { name: 'Approval' });
	await expect(card).toBeVisible({ timeout: 30_000 });
	await director.focus('desktop', 'approval', card);
	await expect(tab.getByTitle('waiting')).toBeVisible();
	await director.hold(2);
	director.mark('desktop', 'end');

	const phoneCard = phone.getByRole('region', { name: 'Approval' });
	await expect(phoneCard).toBeVisible();
	director.mark('phone', 'start');
	await director.focus('phone', 'approval', phoneCard);
	await director.hold(1.6);
	await director.tap('phone', phoneCard.getByRole('button', { name: 'Allow', exact: true }), 0.6);
	await expect(phoneCard).toHaveCount(0);
	await expect(phone.getByRole('tab', { name: /^Claude/ }).getByTitle('done')).toBeVisible({ timeout: 30_000 });
	await director.hold(2.5);
	director.mark('phone', 'end');

	await expect(tab.getByTitle('done')).toBeVisible();
	director.mark('desktop', 'review');
	await director.click('desktop', sidebar.getByRole('list', { name: 'Changes', exact: true }).getByRole('button', { name: '+page.svelte', exact: true }));
	const diff = desktop.locator('.diff:not([hidden])').filter({ has: desktop.locator('.sides') });
	await expect(diff.locator('.cm-merge-b .cm-changedLine').first()).toBeVisible();
	await director.focus('desktop', 'diff', diff);
	await director.hold(2.2);
	await director.click('desktop', sidebar.getByRole('button', { name: 'Stage All Changes' }));
	await expect(sidebar.getByRole('list', { name: 'Staged Changes' })).toBeVisible();
	await director.hold(0.4);
	await director.click('desktop', sidebar.getByRole('button', { name: 'Draft' }), 0.5);
	const message = sidebar.getByRole('textbox', { name: 'Commit message' });
	await expect(message).toHaveValue(/^Add tags to notes/, { timeout: 15_000 });
	await director.focus('desktop', 'message', message);
	await director.hold(1.4);
	await director.click('desktop', sidebar.getByRole('button', { name: 'Commit', exact: true }), 0.5);
	await expect(sidebar.getByRole('list', { name: 'Staged Changes' })).toHaveCount(0);
	await director.hold(0.4);
	await director.click('desktop', sidebar.getByRole('button', { name: 'History', exact: true }), 0.5);
	await expect(sidebar.getByRole('list', { name: 'Commits' })).toContainText('Add tags to notes');
	await director.hold(1.6);
	director.mark('desktop', 'reviewed');

	await desktop.getByRole('tab', { name: 'notes.ts' }).click();
	await desktop.mouse.move(1270, 790);
	await director.finish();

	const stills = join(footage, 'themes', 'desktop');
	mkdirSync(stills, { recursive: true });
	await director.hold(1);
	let background = await desktop.evaluate(() => getComputedStyle(document.body).backgroundColor);
	const frames = [];
	for (const [index, theme] of ['Default', ...themes].entries()) {
		if (index) {
			await desktop.request.put('/api/theme', { data: JSON.stringify(theme), headers: { 'Content-Type': 'application/json' } });
			background = await settle(desktop, background);
		}
		const file = `${String(index).padStart(2, '0')}.png`;
		await desktop.screenshot({ path: join(stills, file) });
		frames.push({ t: index, file, name: theme });
	}
	writeFileSync(join(stills, 'frames.json'), JSON.stringify({ viewport: { width: 1280, height: 800 }, scale: 2, frames }));
});
