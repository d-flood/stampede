import type { Page } from '@playwright/test';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { Chat } from './chat.ts';
import { Codex } from './codex.ts';
import { Director, footage } from './record.ts';
import { configure, project, seed } from './stage.ts';

const projects = { tidepool: '🌊', harbor: '⚓', lighthouse: '💡', atlas: '🗺️' };

function start(page: Page, name: string, rig: string, text: string) {
	return page.evaluate(([name, rig, text]) => new Promise<void>((resolve) => {
		const socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/ws`);
		socket.onopen = () => {
			socket.send(JSON.stringify({ type: 'hello', deviceId: crypto.randomUUID() }));
			socket.send(JSON.stringify({ type: 'projects.list' }));
		};
		socket.onmessage = ({ data }) => {
			const message = typeof data === 'string' && JSON.parse(data);
			if (message?.type === 'projects.changed') {
				const projectId = message.projects.find((project: { path: string }) => project.path.endsWith(`/${name}`))?.id;
				if (projectId) socket.send(JSON.stringify({ type: 'chat.create', projectId, rig }));
			}
			if (message?.type === 'chat.created') {
				socket.send(JSON.stringify({ type: 'chat.send', chatId: message.chat.id, text }));
				setTimeout(() => { socket.close(); resolve(); }, 200);
			}
		};
	}), [name, rig, text]);
}

async function icon(page: Page, name: string, emoji: string) {
	const id = await page.evaluate((name) => new Promise<string>((resolve) => {
		const socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/ws`);
		socket.onopen = () => socket.send(JSON.stringify({ type: 'projects.list' }));
		socket.onmessage = ({ data }) => {
			const message = typeof data === 'string' && JSON.parse(data);
			if (message?.type !== 'projects.changed') return;
			socket.close();
			resolve(message.projects.find((project: { path: string }) => project.path.endsWith(`/${name}`)).id);
		};
	}), name);
	const png = await page.evaluate((emoji) => {
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = 256;
		const context = canvas.getContext('2d')!;
		context.font = '200px "Noto Color Emoji"';
		context.textAlign = 'center';
		context.textBaseline = 'middle';
		context.fillText(emoji, 128, 140);
		return canvas.toDataURL('image/png').split(',')[1];
	}, emoji);
	const response = await page.request.put(`/api/projects/${id}/icon`, { data: Buffer.from(png, 'base64'), headers: { 'Content-Type': 'image/png' } });
	expect(response.ok()).toBe(true);
}

// Reads resident and proportional memory, in MiB, of this server's own processes: the server and its Terminal holders.
function resident(data: string) {
	const sample = { server: 0, serverPss: 0, holders: 0, holdersPss: 0, terminals: 0 };
	for (const pid of readdirSync('/proc').filter((name) => /^\d+$/.test(name))) {
		try {
			const command = readFileSync(`/proc/${pid}/cmdline`, 'utf8').split('\0');
			if (command[command.indexOf('--data-dir') + 1] !== data) continue;
			const rollup = readFileSync(`/proc/${pid}/smaps_rollup`, 'utf8');
			const [rss, pss] = ['Rss', 'Pss'].map((field) => Number(rollup.match(new RegExp(`^${field}:\\s+(\\d+)`, 'm'))![1]) / 1024);
			if (command[1] === 'serve') {
				sample.server += rss;
				sample.serverPss += pss;
			}
			if (command[1] === 'hold') {
				sample.holders += rss;
				sample.holdersPss += pss;
				sample.terminals++;
			}
		} catch {
			continue;
		}
	}
	return sample;
}

async function drawerShell(page: Page, root: string, name: string) {
	const drawer = page.getByRole('region', { name: 'Drawer' });
	await drawer.getByRole('button', { name: 'New Drawer Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
	await expect(drawer.locator('div.terminal').last()).toBeVisible();
	await page.waitForTimeout(1500);
	await page.keyboard.type(`exec env -i HOME=${root} PATH=/usr/bin:/bin TERM=xterm-256color GIT_PAGER=cat PS1='\\W $ ' PROMPT_COMMAND='printf "\\033]0;%s\\007" "\${PWD##*/}"' bash --norc --noprofile\n`);
	await page.keyboard.type(`cd ${join(root, 'projects', name)} && clear\n`);
	await expect(drawer.getByRole('tablist', { name: 'Drawer Tabs' }).getByRole('tab').last()).toContainText(name);
}

test('one small server behind all of it', async ({ browser, baseURL }) => {
	test.setTimeout(180_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	for (const name of Object.keys(projects)) project(root, name);
	const claude = new Chat(seed)
		.prompt()
		.say('Profiling the sync loop before changing it.')
		.read('src/lib/notes.ts').sleep(2.5)
		.grep('searchNotes', ['src/lib/notes.ts', 'src/routes/+page.svelte']).sleep(2.5)
		.read('src/routes/+page.svelte').sleep(2.5)
		.bash('pnpm vitest bench', 'Benchmark note search', 'search 10k notes   1.9ms\nsearch 100k notes  21.4ms').sleep(2.5)
		.say('Search scans every note on each keystroke. Checking where the list re-renders.')
		.read('src/lib/components/NoteList.svelte').sleep(2.5)
		.grep('$derived\\(', ['src/routes/+page.svelte', 'src/lib/components/NoteEditor.svelte']).sleep(2.5)
		.read('src/lib/notes.test.ts').sleep(2.5)
		.read('src/lib/components/NoteEditor.svelte').sleep(2.5)
		.bash('pnpm vitest bench --run search', 'Benchmark search on 100k notes', 'search 100k notes  21.2ms').sleep(2.5)
		.read('package.json')
		.sleep(90)
		.end('Done.');
	const codex = new Codex(seed, 'Update the changelog for 2.4')
		.say('Collecting the merged changes since 2.3.')
		.plan([['Collect merged changes', 'inProgress'], ['Group them by area', 'pending'], ['Write the 2.4 entry', 'pending']])
		.read('README.md').sleep(2.5)
		.read('package.json').sleep(2.5)
		.say('Notes, search and offline work changed. Reading each area.')
		.read('src/lib/notes.ts').sleep(2.5)
		.read('src/routes/+page.svelte').sleep(2.5)
		.read('src/lib/components/NoteList.svelte').sleep(2.5)
		.read('src/lib/components/NoteEditor.svelte').sleep(2.5)
		.read('src/lib/notes.test.ts')
		.sleep(90)
		.end('Done.');
	const fixtures = { claude: join(root, 'claude.jsonl'), codex: join(root, 'codex.jsonl') };
	claude.write(fixtures.claude);
	codex.write(fixtures.codex);

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, [{ name: 'Claude', fixture: fixtures.claude, shortcut: 'c' }], { codex: fixtures.codex });
	await desktop.goto('/');
	const rail = desktop.getByRole('navigation', { name: 'Projects' });
	for (const name of ['atlas', 'lighthouse', 'harbor', 'tidepool']) {
		await rail.getByRole('button', { name: 'Open Project' }).click();
		await desktop.getByRole('option', { name: new RegExp(`/${name}$`) }).click();
		await expect(desktop.getByRole('dialog')).toBeHidden();
	}
	for (const [name, emoji] of Object.entries(projects)) await icon(desktop, name, emoji);
	await start(desktop, 'harbor', 'Claude', 'Speed up search for large notebooks');
	await start(desktop, 'lighthouse', 'Codex', 'Update the changelog for 2.4');
	await start(desktop, 'atlas', 'Claude', 'Speed up tile loading');

	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) await desktop.getByRole('button', { name: 'Sidebar' }).click();
	for (const name of ['src', 'lib']) await sidebar.getByRole('button', { name, exact: true }).click();
	await sidebar.getByRole('button', { name: 'notes.ts', exact: true }).click();
	await expect(desktop.getByRole('tab', { name: 'notes.ts' })).toHaveAttribute('aria-selected', 'true');
	await desktop.getByRole('button', { name: 'New Tab' }).first().click();
	await desktop.getByRole('menuitem', { name: 'Claude', exact: true }).click();
	const panel = desktop.getByRole('tabpanel').filter({ has: desktop.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('textbox', { name: 'Composer' }).fill('Profile note search on a 100k-note notebook');
	await panel.getByRole('button', { name: 'Send' }).click();
	await desktop.getByRole('tab', { name: 'notes.ts' }).click();
	await desktop.keyboard.press('Control+;');
	await desktop.keyboard.press('s');
	await expect(desktop.getByRole('tablist', { name: 'Right Tabs' }).getByRole('tab', { name: 'notes.ts' })).toBeVisible();
	await sidebar.getByRole('button', { name: 'Sidebar' }).click();
	await expect(sidebar).toBeHidden();

	await desktop.keyboard.press('Control+Backquote');
	const drawer = desktop.getByRole('region', { name: 'Drawer' });
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).toBe('none');
	await drawerShell(desktop, root, 'tidepool');
	await desktop.keyboard.type('git log --oneline\n');
	await drawerShell(desktop, root, 'harbor');
	await desktop.keyboard.type('git status --short --branch\n');
	await desktop.waitForTimeout(500);
	await desktop.keyboard.press('Control+Backquote');
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).not.toBe('none');
	await expect(panel.getByText(/tool calls/)).toBeVisible({ timeout: 30_000 });
	await desktop.mouse.move(700, 420);

	const data = join(root, 'data');
	const cdp = await desktop.context().newCDPSession(desktop);
	await cdp.send('Performance.enable');
	const stats: (ReturnType<typeof resident> & { t: number; heap: number })[] = [];
	const sampling = setInterval(async () => {
		const t = Date.now() / 1000;
		const sample = resident(data);
		const { metrics } = await cdp.send('Performance.getMetrics');
		stats.push({ t, ...sample, heap: metrics.find((metric) => metric.name === 'JSHeapUsedSize')!.value / 2 ** 20 });
	}, 250);

	const director = new Director('light');
	await director.record('desktop', desktop);
	await director.hold(1.2);
	director.mark('desktop', 'start');
	await director.hold(1.2);
	for (const name of ['harbor', 'lighthouse', 'atlas']) {
		await director.click('desktop', rail.getByRole('button', { name, exact: true }), 0.5);
		await director.hold(0.7);
	}
	await director.click('desktop', rail.getByRole('button', { name: 'tidepool', exact: true }), 0.5);
	await director.rest('desktop', desktop, 700, 420);
	director.mark('desktop', 'drawer');
	director.keys('desktop', 'Ctrl + `');
	await desktop.keyboard.press('Control+Backquote');
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).toBe('none');
	await director.focus('desktop', 'drawer', drawer);
	await director.hold(1.6);
	await desktop.keyboard.press('Control+Backquote');
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).not.toBe('none');
	director.mark('desktop', 'lsp');
	const lsp = desktop.getByRole('switch', { name: 'Language server' });
	await expect(lsp).toHaveAttribute('aria-checked', 'false');
	await director.point('desktop', lsp, 0.6);
	await director.focus('desktop', 'lsp', lsp);
	await director.hold(3.2);
	director.mark('desktop', 'end');
	await director.finish();
	clearInterval(sampling);
	writeFileSync(join(footage, 'light', 'stats.json'), JSON.stringify(stats));
});
