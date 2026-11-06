import type { Page } from '@playwright/test';
import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { Chat, type Question } from './chat.ts';
import { Director } from './record.ts';
import { configure, project, seed } from './stage.ts';

const projects = { tidepool: '🌊', harbor: '⚓', lighthouse: '💡', atlas: '🗺️' };

const release: Question = {
	header: 'Release',
	question: 'Ship the map tiles in this release or the next?',
	multiSelect: false,
	options: [{ label: 'This release', description: 'Tiles are ready behind a flag' }, { label: 'Next release', description: 'Wait for the offline cache' }]
};

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

test('every project and agent in one place', async ({ browser, baseURL }) => {
	test.setTimeout(180_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	for (const name of Object.keys(projects)) project(root, name);
	const working = new Chat(seed)
		.prompt()
		.say('Profiling the sync loop before changing it.')
		.read('src/lib/notes.ts')
		.sleep(2)
		.bash('pnpm vitest bench', 'Benchmark note search', 'search 10k notes   1.9ms\nsearch 100k notes  21.4ms')
		.sleep(30)
		.end('Done.');
	const asking = new Chat(seed).prompt().say('The tiles work offline already. One decision before I merge:').ask([release, 'This release']).end('Done.');
	const fixtures = { working: join(root, 'working.jsonl'), asking: join(root, 'asking.jsonl') };
	working.write(fixtures.working);
	asking.write(fixtures.asking);

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, [{ name: 'Claude', fixture: fixtures.working, shortcut: 'c' }, { name: 'Claude Opus', fixture: fixtures.asking }]);
	await desktop.goto('/');
	const rail = desktop.getByRole('navigation', { name: 'Projects' });
	for (const name of ['atlas', 'lighthouse', 'harbor', 'tidepool']) {
		await rail.getByRole('button', { name: 'Open Project' }).click();
		await desktop.getByRole('option', { name: new RegExp(`/${name}$`) }).click();
		await expect(desktop.getByRole('dialog')).toBeHidden();
	}
	for (const [name, emoji] of Object.entries(projects)) await icon(desktop, name, emoji);
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) await desktop.getByRole('button', { name: 'Sidebar' }).click();
	await sidebar.getByRole('button', { name: 'README.md', exact: true }).click();
	await desktop.getByRole('radio', { name: 'Preview', exact: true }).click();

	await desktop.keyboard.press('Control+Backquote');
	const drawer = desktop.getByRole('region', { name: 'Drawer' });
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).toBe('none');
	await drawer.getByRole('button', { name: 'New Drawer Tab' }).click();
	await desktop.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
	await expect(drawer.locator('div.terminal')).toBeVisible();
	await desktop.waitForTimeout(1500);
	await desktop.keyboard.type(`exec env -i HOME=${root} PATH=/usr/bin:/bin TERM=xterm-256color GIT_PAGER=cat PS1='\\W $ ' PROMPT_COMMAND='printf "\\033]0;%s\\007" "\${PWD##*/}"' bash --norc --noprofile\n`);
	await desktop.keyboard.type(`cd ${join(root, 'projects', 'tidepool')} && clear\n`);
	await expect(drawer.getByRole('tablist', { name: 'Drawer Tabs' }).getByRole('tab').last()).toContainText('tidepool');
	await desktop.keyboard.press('Control+Backquote');
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).not.toBe('none');
	await desktop.mouse.move(700, 420);
	const tabs = drawer.getByRole('tablist', { name: 'Drawer Tabs' });
	await tabs.evaluate((element) => {
		const seen = new Set<string>();
		new MutationObserver(() => seen.add(element.textContent ?? '')).observe(element, { subtree: true, childList: true, characterData: true });
		Object.assign(window, { seen });
	});

	const director = new Director('fleet');
	await director.record('desktop', desktop);
	await director.hold(0.8);
	director.mark('desktop', 'start');
	await start(desktop, 'harbor', 'Claude', 'Speed up search for large notebooks');
	await director.hold(0.6);
	await start(desktop, 'lighthouse', 'Codex', 'Update the changelog for 2.4');
	await director.hold(0.6);
	await start(desktop, 'atlas', 'Claude Opus', 'Finish the offline map tiles');
	const list = desktop.getByRole('button', { name: 'Attention list' });
	await expect(rail.getByRole('button', { name: 'atlas', exact: true })).toContainText('1', { timeout: 15_000 });
	await director.hold(1.2);
	await director.click('desktop', list);
	const entry = desktop.getByRole('menuitem', { name: 'Claude Opus in atlas' });
	await expect(entry).toBeVisible();
	await director.focus('desktop', 'attention', desktop.getByRole('menu'));
	await director.hold(0.8);
	await director.click('desktop', entry, 0.5);
	const question = desktop.getByRole('region', { name: 'Question', exact: true });
	await expect(question).toBeVisible();
	await director.focus('desktop', 'question', question);
	await director.rest('desktop', desktop, 1180, 260);
	await director.hold(1.4);
	director.mark('desktop', 'drawer');
	director.keys('desktop', 'Ctrl + `');
	await desktop.keyboard.press('Control+Backquote');
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).toBe('none');
	await director.focus('desktop', 'drawer', drawer);
	await director.hold(0.3);
	await desktop.keyboard.type('git log --oneline\n', { delay: 45 });
	await director.hold(2.2);
	director.mark('desktop', 'end');
	await director.finish();
	expect(await desktop.evaluate(() => [...(window as unknown as { seen: Set<string> }).seen].filter((text) => text.includes('—')))).toEqual([]);
});
