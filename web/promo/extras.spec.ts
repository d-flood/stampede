import type { Page } from '@playwright/test';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { Chat } from './chat.ts';
import { Director } from './record.ts';
import { configure, project, seed } from './stage.ts';

const summary = 'Continue tag suggestions in Tidepool.\n\nThe editor suggests tags from other notes, and the README has a Tags section. Next: rank suggestions by how often each tag is used. All 7 note tests pass.';

const preview = `<!doctype html><html><head><title>Tidepool</title><style>
body { margin: 0; font: 15px/1.4 system-ui, sans-serif; background: #f6f3ee; color: #1f2328; display: grid; grid-template-columns: 240px 1fr; height: 100vh }
aside { border-right: 1px solid #e2ddd5; padding: 18px 14px; background: #fbf9f6 }
h1 { font: 600 20px Georgia, serif; margin: 0 0 14px }
input { width: 100%; box-sizing: border-box; padding: 7px 10px; border: 1px solid #d8d2c8; border-radius: 8px; background: #fff; font: inherit }
nav { display: flex; gap: 6px; flex-wrap: wrap; margin: 12px 0 }
button { border: none; border-radius: 999px; padding: 3px 10px; background: #ebe6de; color: #4a4f57; font: 13px system-ui; cursor: pointer }
button.on { background: #0e7490; color: #ecfeff }
ul { list-style: none; margin: 0; padding: 0 }
li { padding: 8px; border-radius: 8px }
li.selected { background: #ebe6de }
main { padding: 28px 36px }
main h2 { font: 600 26px Georgia, serif; margin: 0 0 8px }
p { color: #5b6169 }
</style></head><body>
<aside><h1>Tidepool</h1><input placeholder="Search notes"><nav><button>#coast</button><button>#recipes</button><button>#work</button></nav>
<ul><li class="selected" data-tags="coast">Tide tables</li><li data-tags="recipes">Seaweed salad</li><li data-tags="coast">Beach day</li><li data-tags="work">Standup notes</li></ul></aside>
<main><h2>Tide tables</h2><p>Low tide 6:40 · high tide 12:55</p></main>
<script>
console.log('Tidepool dev server ready');
for (const button of document.querySelectorAll('nav button')) button.onclick = () => {
	const tag = button.textContent.slice(1);
	const on = !button.classList.contains('on');
	for (const other of document.querySelectorAll('nav button')) other.classList.toggle('on', other === button && on);
	let shown = 0;
	for (const item of document.querySelectorAll('li')) {
		const match = !on || item.dataset.tags === tag;
		item.hidden = !match;
		shown += match;
	}
	console.log(on ? 'Filter #' + tag + ': ' + shown + ' notes' : 'Filter cleared');
};
</script></body></html>`;

function conversation() {
	return new Chat(seed)
		.prompt()
		.say("I'll check how notes are stored, then add tag suggestions to the editor.")
		.read('src/lib/notes.ts')
		.sleep(1.5)
		.grep('searchNotes', ['src/lib/notes.ts', 'src/routes/+page.svelte', 'src/lib/notes.test.ts'])
		.sleep(1.5)
		.read('src/lib/components/NoteEditor.svelte')
		.sleep(2)
		.bash('pnpm test src/lib/notes.test.ts', 'Run the notes tests', ' ✓ src/lib/notes.test.ts (7 tests) 41ms\n\n Test Files  1 passed (1)\n      Tests  7 passed (7)')
		.say('Suggestions are in. The editor offers tags from your other notes, and all 7 tests pass.')
		.end('Suggestions are in.')
		.prompt()
		.say('Added a **Tags** section to the README.')
		.end('README updated.')
		.prompt()
		.say(summary)
		.end(summary);
}

async function park(page: Page) {
	await page.mouse.move(1270, 790);
}

test('the rest of the toolkit', async ({ browser, baseURL }) => {
	test.setTimeout(240_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	project(root, 'tidepool');
	const fixture = join(root, 'extras.jsonl');
	conversation().write(fixture);
	const server = createServer((_request, response) => {
		response.setHeader('Content-Type', 'text/html; charset=utf-8');
		response.end(preview);
	});
	await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
	const address = `http://localhost:${(server.address() as AddressInfo).port}/`;

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, [{ name: 'Claude', fixture, shortcut: 'c' }]);
	for (const [name, text] of [['tests', 'Then run the tests and fix anything that fails.'], ['terse', 'Keep the diff small.']]) {
		expect((await desktop.request.put('/api/clips', { data: { name, text } })).ok()).toBe(true);
	}
	await desktop.goto('/');
	await desktop.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await desktop.getByRole('option', { name: /\/tidepool$/ }).click();
	await expect(desktop.getByRole('dialog')).toBeHidden();
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) await desktop.getByRole('button', { name: 'Sidebar' }).click();
	await desktop.getByRole('button', { name: 'New Tab' }).click();
	await desktop.getByRole('menuitem', { name: 'Claude', exact: true }).click();
	const tab = desktop.getByRole('tab', { name: /^Claude/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const panel = desktop.getByRole('tabpanel').filter({ has: desktop.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('Add tag suggestions to the editor.');
	await park(desktop);

	const director = new Director('extras');
	await director.record('desktop', desktop);
	await director.hold(0.8);

	director.mark('desktop', 'clips');
	await director.rest('desktop', desktop, 900, 560);
	await director.type('desktop', composer, ' ;te');
	const clips = panel.getByRole('region', { name: 'Clips' });
	await expect(clips.getByRole('button')).toHaveCount(2);
	await director.focus('desktop', 'clips', composer);
	await director.hold(0.6);
	await director.type('desktop', composer, 's');
	await expect(clips.getByRole('button')).toHaveCount(1);
	await director.hold(0.4);
	director.keys('desktop', 'Enter');
	await composer.press('Enter');
	await expect(composer).toHaveValue('Add tag suggestions to the editor. Then run the tests and fix anything that fails.');
	await director.hold(1.2);
	director.mark('desktop', 'clips-end');

	await panel.getByRole('button', { name: 'Send', exact: true }).click();
	await expect(tab.getByTitle('working')).toBeVisible();
	await expect(panel.getByText("I'll check how notes are stored")).toBeVisible();
	await park(desktop);
	await director.hold(0.6);

	director.mark('desktop', 'queue');
	await director.type('desktop', composer, 'Then add a Tags section to the README.');
	await director.click('desktop', panel.getByRole('button', { name: 'Queue', exact: true }), 0.5);
	const queued = panel.getByRole('list', { name: 'Queued' });
	await expect(queued.getByRole('listitem')).toHaveText(['Then add a Tags section to the README.']);
	await director.focus('desktop', 'queue', queued);
	await director.hold(1.4);
	director.mark('desktop', 'queue-end');

	await expect(panel.getByText('Added a Tags section to the README.')).toBeVisible({ timeout: 30_000 });
	await expect(tab.getByTitle('done')).toBeVisible();
	const message = panel.getByRole('region', { name: 'User message 2', exact: true });
	await message.scrollIntoViewIfNeeded();
	await park(desktop);
	await director.hold(0.6);

	director.mark('desktop', 'rewind');
	await director.click('desktop', message.getByRole('button', { name: 'Rewind from message 2', exact: true }), 0.6);
	const rewind = desktop.getByRole('dialog', { name: 'Rewind before message 2?' });
	await expect(rewind).toBeVisible();
	await director.focus('desktop', 'rewind', rewind);
	await director.hold(1.6);
	director.mark('desktop', 'rewind-end');

	await rewind.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(rewind).toHaveCount(0);
	await desktop.mouse.wheel(0, 6000);
	await park(desktop);
	await director.hold(0.6);

	director.mark('desktop', 'handoff');
	await director.click('desktop', panel.getByRole('button', { name: 'More', exact: true }), 0.6);
	await director.click('desktop', desktop.getByRole('menuitem', { name: 'New Chat from here…' }), 0.4);
	const handoff = desktop.getByRole('dialog', { name: 'New Chat from here' });
	await expect(handoff).toBeVisible();
	await director.focus('desktop', 'handoff', handoff);
	const rig = handoff.getByLabel('Rig', { exact: true });
	await director.point('desktop', rig, 0.5);
	director.events.push({ t: Date.now() / 1000, device: 'desktop', type: 'click' });
	await rig.selectOption('Codex');
	await director.hold(0.5);
	await director.click('desktop', handoff.getByRole('button', { name: 'Prepare handoff' }), 0.5);
	const codex = desktop.getByRole('tab', { name: /^Codex/ });
	await expect(codex).toHaveAttribute('aria-selected', 'true', { timeout: 30_000 });
	await expect(composer).toHaveValue(summary);
	await director.rest('desktop', desktop, 1000, 300);
	await director.focus('desktop', 'summary', panel.locator('.composer'));
	await director.hold(1.6);
	director.mark('desktop', 'handoff-end');

	await park(desktop);
	await director.hold(0.4);

	director.mark('desktop', 'search');
	await director.click('desktop', sidebar.getByRole('button', { name: 'Search', exact: true }), 0.5);
	const search = sidebar.getByRole('textbox', { name: 'Search', exact: true });
	await director.type('desktop', search, 'searchNotes');
	await expect(sidebar.locator('.results')).toContainText('notes.test.ts');
	await director.focus('desktop', 'search', sidebar);
	await director.hold(1.2);
	director.mark('desktop', 'search-end');

	await desktop.getByRole('button', { name: 'New Tab', exact: true }).click();
	await desktop.getByRole('menuitem', { name: 'Browser', exact: true }).click();
	const tabs = desktop.getByRole('tabpanel', { name: 'Browser', exact: true });
	await tabs.getByRole('textbox', { name: 'Host address' }).fill(address);
	await tabs.getByRole('button', { name: 'Go', exact: true }).click();
	const frame = tabs.frameLocator('iframe');
	await expect(frame.getByRole('heading', { name: 'Tide tables' })).toBeVisible();
	await park(desktop);
	await director.hold(0.6);

	director.mark('desktop', 'browser');
	await director.focus('desktop', 'browser', tabs);
	await director.click('desktop', frame.getByRole('button', { name: '#coast' }), 0.6);
	await expect(frame.getByText('Seaweed salad')).toBeHidden();
	await director.hold(0.4);
	await director.click('desktop', tabs.getByRole('button', { name: /^Console / }), 0.5);
	await expect(tabs.getByRole('log', { name: 'Console output' })).toContainText('Filter #coast: 2 notes');
	await director.hold(1.2);
	director.mark('desktop', 'browser-end');

	await park(desktop);
	await director.hold(0.4);

	director.mark('desktop', 'usage');
	const usage = sidebar.getByRole('region', { name: 'Usage windows' });
	await expect(usage.getByRole('meter', { name: 'Claude · 5h' })).toBeVisible();
	await director.focus('desktop', 'usage', usage);
	await director.point('desktop', usage.getByRole('meter', { name: 'Claude · 5h' }), 0.6);
	await director.hold(0.8);
	await director.point('desktop', usage.getByRole('meter', { name: 'Codex · week' }), 0.6);
	await director.hold(1.2);
	director.mark('desktop', 'usage-end');
	await director.finish();
	server.close();
});
