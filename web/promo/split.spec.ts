import type { Page } from '@playwright/test';
import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { Chat } from './chat.ts';
import { Codex } from './codex.ts';
import { Director } from './record.ts';
import { configure, project, seed } from './stage.ts';

const prompt = 'Add tags to notes and filter the list by tag.';
const field = ['\tpinned: boolean;\n\tupdated: number;', '\tpinned: boolean;\n\ttags: string[];\n\tupdated: number;'] as const;
const filter = ["pinned: false, updated: Date.now() };\n}", `pinned: false, tags: [], updated: Date.now() };
}

export function notesWithTag(notes: Note[], tag?: string) {
	return tag ? notes.filter((note) => note.tags.includes(tag)) : notes;
}`] as const;
const output = ' ✓ src/lib/notes.test.ts (7 tests) 41ms\n\n Test Files  1 passed (1)\n      Tests  7 passed (7)';

function send(page: Page, chatId: string, text: string) {
	return page.evaluate(([chatId, text]) => new Promise<void>((resolve) => {
		const socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/ws`);
		socket.onopen = () => {
			socket.send(JSON.stringify({ type: 'hello', deviceId: crypto.randomUUID() }));
			socket.send(JSON.stringify({ type: 'chat.send', chatId, text }));
			setTimeout(() => { socket.close(); resolve(); }, 200);
		};
	}), [chatId, text]);
}

test('Claude Code and Codex share one interface', async ({ browser, baseURL }) => {
	test.setTimeout(180_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	project(root, 'tidepool');
	const claude = new Chat(seed)
		.prompt()
		.say("I'll check how notes are stored first.")
		.todos([['Add a tags field', 'in_progress'], ['Filter the list by tag', 'pending'], ['Run the tests', 'pending']])
		.read('src/lib/notes.ts')
		.say('Notes are plain objects, so tags can live on each one.')
		.edit('src/lib/notes.ts', ...field)
		.edit('src/lib/notes.ts', ...filter)
		.todos([['Add a tags field', 'completed'], ['Filter the list by tag', 'completed'], ['Run the tests', 'in_progress']])
		.approve('pnpm test src/lib/notes.test.ts', 'Run the notes tests', 'pnpm test:*', output)
		.end('Done.');
	const codex = new Codex(seed, prompt)
		.say("I'll check how notes are stored first.")
		.plan([['Add a tags field', 'inProgress'], ['Filter the list by tag', 'pending'], ['Run the tests', 'pending']])
		.read('src/lib/notes.ts')
		.say('Notes are plain objects, so tags can live on each one.')
		.edit('src/lib/notes.ts', ...field)
		.edit('src/lib/notes.ts', ...filter)
		.plan([['Add a tags field', 'completed'], ['Filter the list by tag', 'completed'], ['Run the tests', 'inProgress']])
		.approve('pnpm test src/lib/notes.test.ts', 'Run the notes tests?', output)
		.end('Done.');
	const fixtures = { claude: join(root, 'split-claude.jsonl'), codex: join(root, 'split-codex.jsonl') };
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
	const chats: string[] = [];
	for (const rig of ['Claude', 'Codex']) {
		await desktop.getByRole('button', { name: 'New Tab' }).first().click();
		await desktop.getByRole('menuitem', { name: rig, exact: true }).click();
		const tab = desktop.getByRole('tab', { name: new RegExp(`^${rig}`) });
		await expect(tab).toHaveAttribute('aria-selected', 'true');
		chats.push((await tab.getAttribute('data-tab'))!);
	}
	await desktop.keyboard.press('Control+;');
	await desktop.keyboard.press('s');
	await expect(desktop.getByRole('tablist', { name: 'Right Tabs' }).getByRole('tab', { name: /^Codex/ })).toBeVisible();
	await desktop.mouse.move(1270, 790);

	const director = new Director('split');
	await director.record('desktop', desktop);
	await director.hold(0.8);
	director.mark('desktop', 'start');
	await Promise.all(chats.map((chat) => send(desktop, chat, prompt)));
	const cards = desktop.getByRole('region', { name: 'Approval' });
	await expect(cards).toHaveCount(2, { timeout: 30_000 });
	director.mark('desktop', 'asked');
	await director.hold(2);
	director.mark('desktop', 'end');
	await director.finish();
});
