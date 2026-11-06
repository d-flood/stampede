import type { Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { pinned } from '../e2e/shell.ts';
import { Chat } from './chat.ts';
import { Director } from './record.ts';
import { configure, project, seed } from './stage.ts';

const prompt = 'Show me the tag filter working in a browser.';

const notes = [
	{ title: 'Tide tables', tags: ['coast', 'weekend'], body: 'Low tide 6:40 · high tide 12:55' },
	{ title: 'Beach day', tags: ['coast'], body: 'Towels, kite, the good thermos.' },
	{ title: 'Chowder', tags: ['recipes'], body: 'Clams, leeks, a splash of cream.' },
	{ title: 'Fish stew', tags: ['recipes', 'weekend'], body: 'Saffron, fennel, crusty bread.' },
	{ title: 'Lighthouse walk', tags: ['coast'], body: 'North path, about two hours.' },
	{ title: 'Standup', tags: ['work'], body: 'Ship tags, then offline sync.' }
];

const app = `<!doctype html>
<style>
* { box-sizing: border-box }
body { margin: 0; height: 100vh; display: grid; grid-template-columns: 280px 1fr; font: 15px/1.45 system-ui, sans-serif; background: #12161a; color: #e6e3de; overflow: hidden }
aside { border-right: 1px solid #262c33; padding: 22px 18px; background: #161b20 }
.brand { display: flex; align-items: center; gap: 10px; font: 700 20px Georgia, serif; margin-bottom: 18px }
.search { border: 1px solid #2c333b; border-radius: 9px; padding: 8px 12px; color: #7d8793; margin-bottom: 14px }
.chips, .tags { display: flex; flex-wrap: wrap; gap: 7px; align-items: center }
.chips { margin-bottom: 16px }
.chip { padding: 3px 11px; border-radius: 999px; background: #232a31; color: #b4bec9; font-size: 13.5px; transition: background .2s, color .2s }
.chip.on { background: #0e7490; color: #ecfeff }
.chip.new { animation: pop .35s ease-out }
@keyframes pop { from { transform: scale(.6); opacity: 0 } }
.item { padding: 9px 10px; border-radius: 9px; margin-bottom: 2px }
.item b { display: block; font-weight: 600 }
.item small { color: #7d8793 }
.item.selected { background: #232a31 }
main { padding: 36px 44px }
h1 { margin: 0 0 14px; font: 600 38px Georgia, serif }
.add { border: 1px dashed #3a434d; border-radius: 999px; padding: 3px 11px; color: #7d8793; font-size: 13.5px; min-width: 96px }
.add.typing { border-style: solid; border-color: #0e7490; color: #e6e3de }
p { color: #a3adb8; margin: 22px 0 0; font-size: 17px }
#cursor { position: fixed; left: 0; top: 0; width: 22px; height: 30px; pointer-events: none; filter: drop-shadow(0 2px 3px #0008); transition: transform .55s cubic-bezier(.5, 0, .2, 1) }
.ripple { position: fixed; width: 34px; height: 34px; margin: -17px; border-radius: 50%; border: 2px solid #5eead4; animation: ripple .5s ease-out forwards; pointer-events: none }
@keyframes ripple { from { transform: scale(.3); opacity: 1 } to { transform: scale(1.4); opacity: 0 } }
</style>
<aside>
	<div class="brand"><svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="#5eead4" stroke-width="2" stroke-linecap="round"><path d="M2 14c2.5 0 2.5-2 5-2s2.5 2 5 2 2.5-2 5-2 2.5 2 5 2M2 19c2.5 0 2.5-2 5-2s2.5 2 5 2 2.5-2 5-2 2.5 2 5 2M7 9a5 5 0 0 1 10 0"/></svg>Tidepool</div>
	<div class="search">Search notes</div>
	<div class="chips" id="chips"></div>
	<div id="list"></div>
</aside>
<main id="editor"></main>
<svg id="cursor" viewBox="-1 -1 15 21"><path d="M0 0 L0 16 L4.2 12.3 L7 18.6 L9.8 17.4 L7.1 11.2 L12.6 11.2 Z" fill="#fff" stroke="#111" stroke-width="1.1" stroke-linejoin="round"/></svg>
<script>
const notes = ${JSON.stringify(notes)};
const state = { tag: undefined, selected: 'Tide tables', draft: undefined, fresh: undefined };
const chip = (tag, extra = '') => '<span class="chip ' + extra + '" data-tag="' + tag + '">#' + tag + '</span>';
function render() {
	const tags = [...new Set(notes.flatMap((note) => note.tags))];
	document.getElementById('chips').innerHTML = tags.map((tag) => chip(tag, (tag === state.tag ? 'on' : '') + (tag === state.fresh ? ' new' : ''))).join('');
	document.getElementById('list').innerHTML = notes.filter((note) => !state.tag || note.tags.includes(state.tag)).map((note) => '<div class="item ' + (note.title === state.selected ? 'selected' : '') + '" data-note="' + note.title + '"><b>' + note.title + '</b><small>' + note.tags.map((tag) => '#' + tag).join(' ') + '</small></div>').join('');
	const note = notes.find((note) => note.title === state.selected);
	document.getElementById('editor').innerHTML = '<h1>' + note.title + '</h1><div class="tags">' + note.tags.map((tag) => chip(tag, tag === state.fresh ? 'new' : '')).join('') + '<span class="add ' + (state.draft === undefined ? '' : 'typing') + '">' + (state.draft === undefined ? '+ Add tag' : state.draft + '|') + '</span></div><p>' + note.body + '</p>';
}
const cursor = document.getElementById('cursor');
const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function point(selector) {
	const box = document.querySelector(selector).getBoundingClientRect();
	cursor.style.transform = 'translate(' + (box.x + box.width / 2) + 'px, ' + (box.y + box.height / 2) + 'px)';
	await wait(650);
	const ripple = document.createElement('div');
	ripple.className = 'ripple';
	ripple.style.left = box.x + box.width / 2 + 'px';
	ripple.style.top = box.y + box.height / 2 + 'px';
	document.body.append(ripple);
	setTimeout(() => ripple.remove(), 600);
}
window.play = async () => {
	await wait(300);
	await point('[data-tag="coast"]');
	state.tag = 'coast';
	render();
	await wait(450);
	await point('[data-note="Beach day"]');
	state.selected = 'Beach day';
	render();
	await wait(450);
	await point('.add');
	state.draft = '';
	render();
	for (const letter of 'sunset') {
		await wait(110);
		state.draft += letter;
		render();
	}
	await wait(350);
	notes.find((note) => note.title === 'Beach day').tags.push('sunset');
	state.draft = undefined;
	state.fresh = 'sunset';
	render();
	await wait(700);
};
window.pose = () => {
	state.tag = 'coast';
	cursor.style.display = 'none';
	render();
};
cursor.style.transform = 'translate(600px, 420px)';
render();
</script>`;

async function film(page: Page, path: string) {
	const frames = join(path, 'frames');
	mkdirSync(frames, { recursive: true });
	const cdp = await page.context().newCDPSession(page);
	const shots: { t: number; file: string }[] = [];
	cdp.on('Page.screencastFrame', ({ data, metadata, sessionId }) => {
		const file = join(frames, `${String(shots.length).padStart(5, '0')}.jpg`);
		writeFileSync(file, Buffer.from(data, 'base64'));
		shots.push({ t: metadata.timestamp!, file });
		cdp.send('Page.screencastFrameAck', { sessionId }).catch(() => {});
	});
	await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 95, maxWidth: 1920, maxHeight: 1200 });
	await page.waitForTimeout(300);
	await page.evaluate(() => (window as unknown as { play(): Promise<void> }).play());
	await cdp.send('Page.stopScreencast');
	const list = shots.map((shot, index) => `file '${shot.file}'\nduration ${((shots[index + 1]?.t ?? shot.t + 0.1) - shot.t).toFixed(4)}`).join('\n');
	writeFileSync(join(path, 'frames.txt'), `${list}\nfile '${shots.at(-1)!.file}'\n`);
	const video = join(path, 'tag-filter.webm');
	execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', join(path, 'frames.txt'), '-vf', 'fps=30,scale=1920:1200,format=yuv420p', '-c:v', 'libvpx-vp9', '-crf', '30', '-b:v', '0', '-row-mt', '1', video]);
	return video;
}

function publish(root: string, path: string, title: string): string {
	const output = execFileSync(process.env.STAMPEDE_E2E_BINARY!, ['chat-tools', '--data-dir', join(root, 'data')], { input: JSON.stringify({ id: 1, method: 'tools/call', params: { name: 'render_media', arguments: { path, title } } }) + '\n' });
	const result = JSON.parse(output.toString()).result;
	expect(result.isError).toBe(false);
	return result.structuredContent.asset;
}

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

test('an agent shows screenshots and recordings in the chat', async ({ browser, baseURL }) => {
	test.setTimeout(180_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	project(root, 'tidepool');

	const studio = await browser.newContext({ viewport: { width: 960, height: 600 }, deviceScaleFactor: 2, colorScheme: 'dark' });
	const mock = await studio.newPage();
	const results = join(root, 'results');
	mkdirSync(results, { recursive: true });
	await mock.setContent(app);
	const video = await film(mock, results);
	const still = await studio.newPage();
	await still.setContent(app);
	await still.evaluate(() => (window as unknown as { pose(): void }).pose());
	const screenshot = join(results, 'tag-filter.png');
	await still.screenshot({ path: screenshot });
	await studio.close();
	const shown = { image: 'Tag filter with #coast selected', video: 'Filtering by tag, then tagging a note' };
	const assets = { image: publish(root, screenshot, shown.image), video: publish(root, video, shown.video) };

	const fixture = join(root, 'media.jsonl');
	new Chat(seed)
		.prompt()
		.say('I ran Tidepool in a browser and tried the filter. Here it is with **#coast** selected:')
		.media('/tmp/stampede-chat/test-results/tag-filter.png', shown.image, assets.image)
		.sleep(1)
		.say('And a recording of filtering, then tagging a note:')
		.media('/tmp/stampede-chat/test-results/tag-filter.webm', shown.video, assets.video)
		.end('Done.')
		.write(fixture);

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, [{ name: 'Claude', fixture, shortcut: 'c' }]);
	await desktop.goto('/');
	await desktop.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await desktop.getByRole('option', { name: /\/tidepool$/ }).click();
	await expect(desktop.getByRole('dialog')).toBeHidden();
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (await sidebar.isVisible()) await sidebar.getByRole('button', { name: 'Sidebar' }).click();
	await expect(sidebar).toBeHidden();
	await desktop.getByRole('button', { name: 'New Tab' }).first().click();
	await desktop.getByRole('menuitem', { name: 'Claude', exact: true }).click();
	const tab = await pinned(desktop.getByRole('tab', { name: /^Claude/ }));
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const chat = (await tab.getAttribute('data-tab'))!;
	const panel = desktop.getByRole('tabpanel').filter({ has: desktop.getByRole('textbox', { name: 'Composer' }) });
	await desktop.mouse.move(1000, 300);

	const director = new Director('media');
	await director.record('desktop', desktop);
	await director.hold(0.8);
	director.mark('desktop', 'start');
	await send(desktop, chat, prompt);
	director.mark('desktop', 'sent');
	const image = panel.getByRole('img', { name: shown.image });
	await expect(image).toBeVisible({ timeout: 30_000 });
	await expect.poll(() => image.evaluate((node: HTMLImageElement) => node.complete && node.naturalWidth > 0)).toBe(true);
	await director.focus('desktop', 'image', panel.getByRole('figure', { name: shown.image }));
	const player = panel.locator(`video[aria-label="${shown.video}"]`);
	await expect(player).toBeVisible({ timeout: 30_000 });
	await expect.poll(() => player.evaluate((node: HTMLVideoElement) => node.readyState)).toBeGreaterThanOrEqual(2);
	await expect(tab.getByTitle('done')).toBeVisible({ timeout: 30_000 });
	await director.focus('desktop', 'video', panel.getByRole('figure', { name: shown.video }));
	await director.click('desktop', player, 0.6);
	director.mark('desktop', 'play');
	await expect.poll(() => player.evaluate((node: HTMLVideoElement) => node.currentTime)).toBeGreaterThan(0);
	await director.rest('desktop', desktop, 1240, 420);
	await expect.poll(() => player.evaluate((node: HTMLVideoElement) => node.ended), { timeout: 15_000 }).toBe(true);
	await director.hold(0.3);
	director.mark('desktop', 'end');
	await director.finish();
});
