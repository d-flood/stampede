import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { Chat } from './chat.ts';
import { Director } from './record.ts';
import { configure, project, seed } from './stage.ts';

const brief = `# Work offline

Tidepool should keep working with no connection: the app loads offline, edits are queued, and the queue syncs when the network returns.

## Approach
- Cache the app shell with a service worker.
- Record every edit in an IndexedDB outbox before it is applied.
- Replay the outbox when \`online\` fires; the newest edit wins per field.

## Out of scope
Merging edits across devices beyond newest-wins.
`;

const tickets = [
	{ title: 'Cache the app shell', body: 'Register a service worker that precaches the build and serves it offline.', tags: 'routine', blockers: [], seconds: 3.2 },
	{ title: 'Queue edits in an outbox', body: 'Write each edit to an IndexedDB outbox before saving it.', tags: 'judgment', blockers: [], seconds: 2.2 },
	{ title: 'Show an offline banner', body: 'Tell people when they are offline and how many edits are waiting.', tags: 'frontend-design', blockers: [], seconds: 3.8 },
	{ title: 'Sync the outbox when back online', body: 'Replay queued edits in order when the `online` event fires.', tags: 'judgment', blockers: [2], seconds: 2.6 },
	{ title: 'Resolve conflicting edits', body: 'Keep the newest edit per field when a synced note changed elsewhere.', tags: 'judgment', blockers: [4], seconds: 2.4 }
];

const payloads: Record<string, Record<string, string>> = {
	1: { 'src/service-worker.ts': `import { build, files, version } from '$service-worker';\n\nconst cache = \`tidepool-\${version}\`;\n\nself.addEventListener('install', (event) => {\n\tevent.waitUntil(caches.open(cache).then((store) => store.addAll([...build, ...files])));\n});\n\nself.addEventListener('fetch', (event) => {\n\tevent.respondWith(caches.match(event.request).then((hit) => hit ?? fetch(event.request)));\n});\n` },
	2: { 'src/lib/outbox.ts': `import type { Note } from './notes';\n\nexport type Edit = { id: string; note: Partial<Note> & { id: string }; at: number };\n\nconst queue: Edit[] = [];\n\nexport function enqueue(note: Edit['note']) {\n\tqueue.push({ id: crypto.randomUUID(), note, at: Date.now() });\n}\n\nexport function drain() {\n\treturn queue.splice(0);\n}\n` },
	3: { 'src/lib/components/OfflineBanner.svelte': `<script lang="ts">\n\tlet { waiting }: { waiting: number } = $props();\n\tlet online = $state(navigator.onLine);\n</script>\n\n<svelte:window ononline={() => (online = true)} onoffline={() => (online = false)} />\n\n{#if !online}\n\t<p role="status">Offline · {waiting} edits waiting</p>\n{/if}\n` },
	4: { 'src/lib/sync.ts': `import { drain } from './outbox';\nimport { saveNote, type Note } from './notes';\n\nexport function syncWhenOnline(load: (id: string) => Promise<Note>) {\n\taddEventListener('online', async () => {\n\t\tfor (const edit of drain()) await saveNote({ ...(await load(edit.note.id)), ...edit.note });\n\t});\n}\n` },
	5: { 'src/lib/conflicts.ts': `import type { Edit } from './outbox';\n\nexport function newest(edits: Edit[]) {\n\tconst byNote = new Map<string, Edit>();\n\tfor (const edit of edits) {\n\t\tconst seen = byNote.get(edit.note.id);\n\t\tbyNote.set(edit.note.id, seen && seen.at > edit.at ? seen : { ...edit, note: { ...seen?.note, ...edit.note } });\n\t}\n\treturn [...byNote.values()];\n}\n` },
	fix: { 'src/lib/outbox.ts': `import { openDB } from 'idb';\nimport type { Note } from './notes';\n\nexport type Edit = { id: string; note: Partial<Note> & { id: string }; at: number };\n\nconst outbox = openDB('tidepool-outbox', 1, { upgrade: (db) => db.createObjectStore('edits', { keyPath: 'id' }) });\n\nexport async function enqueue(note: Edit['note']) {\n\tawait (await outbox).put('edits', { id: crypto.randomUUID(), note, at: Date.now() });\n}\n\nexport async function drain(): Promise<Edit[]> {\n\tconst db = await outbox;\n\tconst edits = await db.getAll('edits');\n\tawait db.clear('edits');\n\treturn edits.sort((a, b) => a.at - b.at);\n}\n` }
};

function stage(root: string) {
	const payload = join(root, 'payload');
	for (const [name, files] of Object.entries(payloads)) {
		for (const [path, text] of Object.entries(files)) {
			mkdirSync(dirname(join(payload, name, path)), { recursive: true });
			writeFileSync(join(payload, name, path), text);
		}
	}
	const durations = join(root, 'durations');
	writeFileSync(durations, tickets.map((ticket) => ticket.seconds).join('\n') + '\n');
	const ticket = new Chat(seed)
		.prompt()
		.say('Picking up this Ticket. Reading the Brief and the code it touches.')
		.read('src/lib/notes.ts')
		.run('sh', '-c', `n=$(git rev-parse --abbrev-ref HEAD); n=\${n##*/}; sleep $(sed -n "\${n}p" ${durations}); cp -R ${payload}/$n/. .`)
		.say('Done. The change is in and the tests pass.')
		.epic({ action: 'complete', text: 'Complete the Ticket' })
		.end('Done.');
	const reviewer = new Chat(seed)
		.prompt()
		.say('Reviewing the Epic branch against the Brief.')
		.read('src/lib/notes.ts')
		.sleep(1.6)
		.epic({ action: 'finding', title: 'Queued edits are lost on reload', text: 'The outbox lives in memory until the first sync, so closing the tab while offline drops every queued edit.\n' })
		.epic({ action: 'reviewed' })
		.end('One Finding.');
	const remediator = new Chat(seed)
		.prompt()
		.say('Judging the Findings.')
		.sleep(1.2)
		.epic({ action: 'accept', finding: 1, text: 'Real: the outbox has to persist before an edit applies.' })
		.run('cp', '-R', `${payload}/fix/.`, '.')
		.sleep(1)
		.epic({ action: 'remediated', text: 'Persist the outbox in IndexedDB' })
		.end('Fixed.');
	const rigs = { Claude: ticket, 'Claude Review': reviewer, 'Claude Fix': remediator };
	return Object.entries(rigs).map(([name, chat]) => {
		const fixture = join(root, `${name.replace(' ', '-').toLowerCase()}.jsonl`);
		chat.write(fixture);
		return { name, fixture };
	});
}

test('an Epic Run from Brief to Landing', async ({ browser, baseURL }) => {
	test.setTimeout(240_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	const path = project(root, 'tidepool');
	const cli = (stdin: string, ...args: string[]) => execFileSync(process.env.STAMPEDE_E2E_BINARY!, [...args, '--data-dir', join(root, 'data')], { cwd: path, input: stdin });
	cli(brief, 'epic', 'add', 'offline', '--title', 'Work offline', '--brief', '-');
	tickets.forEach((ticket, index) => {
		const blockers = ticket.blockers.length ? ['--blocked-by', ticket.blockers.join(',')] : [];
		cli(`${ticket.body}\n`, 'ticket', 'add', 'offline', String(index + 1), '--title', ticket.title, '--tags', ticket.tags, ...blockers, '--body', '-');
	});
	cli('', 'epic', 'set-setup', 'offline', './scripts/setup');

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, stage(root));
	await desktop.goto('/');
	await desktop.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await desktop.getByRole('option', { name: /\/tidepool$/ }).click();
	await expect(desktop.getByRole('dialog')).toBeHidden();
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) await desktop.getByRole('button', { name: 'Sidebar' }).click();
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	await sidebar.getByRole('region', { name: 'Epics' }).getByRole('button', { name: /^offline/ }).click();
	const panel = desktop.getByRole('tabpanel', { name: 'Epic offline' });
	const run = panel.getByRole('region', { name: 'Run', exact: true });
	await run.getByRole('combobox', { name: 'Rig' }).selectOption('Claude');
	await run.getByRole('button', { name: 'Reviewers: none' }).click();
	await desktop.getByRole('menuitemcheckbox', { name: 'Claude Review' }).click();
	await desktop.keyboard.press('Escape');
	await run.getByRole('combobox', { name: 'Remediator' }).selectOption('Claude Fix');
	await sidebar.getByRole('button', { name: 'Sidebar' }).click();
	await expect(sidebar).toBeHidden();
	await desktop.mouse.move(700, 300);

	const director = new Director('epic');
	await director.record('desktop', desktop);
	await director.hold(1);
	director.mark('desktop', 'start');
	await director.focus('desktop', 'run', run);
	const limit = run.getByRole('group', { name: 'Parallel' });
	await director.click('desktop', limit.getByRole('button', { name: 'More' }));
	await director.click('desktop', limit.getByRole('button', { name: 'More' }), 0.35);
	await expect(limit.locator('output')).toHaveText('3');
	await director.click('desktop', run.getByRole('button', { name: 'Start Run' }), 0.6);
	director.mark('desktop', 'started');
	await director.rest('desktop', desktop, 1240, 420);
	await expect(run).toContainText('Run live');
	await director.focus('desktop', 'tickets', panel.getByRole('region', { name: 'Tickets' }));
	const review = panel.getByRole('region', { name: 'Review' });
	await expect(review).toBeVisible({ timeout: 60_000 });
	director.mark('desktop', 'review');
	await director.focus('desktop', 'review', review);
	await expect(review).toContainText('Remediated at', { timeout: 60_000 });
	const finding = review.locator('details', { hasText: 'Queued edits are lost on reload' });
	await director.click('desktop', finding.locator('summary'), 0.6);
	await expect(finding).toContainText('Accepted: Real');
	await director.hold(1.6);
	const land = panel.getByRole('button', { name: /^Land( into .+)?$/ });
	await expect(land).toBeEnabled({ timeout: 30_000 });
	director.mark('desktop', 'land');
	await director.click('desktop', land);
	const confirm = desktop.getByRole('dialog', { name: 'Land offline into main?' });
	await director.focus('desktop', 'confirm', confirm);
	await director.hold(0.8);
	await director.click('desktop', confirm.getByRole('button', { name: 'Land', exact: true }), 0.5);
	await expect(panel.getByRole('region', { name: 'Integration Worktree' })).toContainText('Landed into main at');
	await director.rest('desktop', desktop, 1240, 420);
	await director.hold(2);
	director.mark('desktop', 'end');
	await director.finish();
});
