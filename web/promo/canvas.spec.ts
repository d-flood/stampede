import type { Page } from '@playwright/test';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { Chat } from './chat.ts';
import { Director } from './record.ts';
import { configure, project, seed } from './stage.ts';

const prompt = 'Sketch the tag filter for desktop and phone.';

const desk = (side: string, main: string) => `<div class="desk"><div class="side">${side}</div><div class="main">${main}</div></div>`;
const lines = (...widths: number[]) => widths.map((width) => `<i class="line" style="width:${width}%"></i>`).join('');
const notes = ['Tide tables', 'Beach day', 'Lighthouse walk'].map((title, index) => `<div class="note${index ? '' : ' picked'}"><b>${title}</b>${lines(70)}</div>`).join('');
const editor = `<b class="title">Tide tables</b><div class="chips"><span class="chip on">#coast</span><span class="chip">#weekend</span></div>${lines(92, 80, 86, 54)}`;

const mockup = `<style>
body { background: #101317; color: #d8d4ce; font: 13px/1.35 system-ui, sans-serif }
header { display: flex; align-items: baseline; gap: 10px; margin-bottom: 12px }
h1 { margin: 0; font: 600 21px Georgia, serif }
.sub { color: #7d8793 }
.option { margin-bottom: 14px; padding: 10px 12px 12px; border: 1px solid #262c33; border-radius: 12px }
.label { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; font-weight: 600 }
.label b { display: inline-grid; place-items: center; width: 20px; height: 20px; border-radius: 6px; background: #262c33 }
.badge, .clear, .sheet { display: none }
.pair { display: grid; grid-template-columns: 1fr 116px; gap: 12px; align-items: stretch }
.desk, .phone { border: 1px dashed #4a525c; border-radius: 9px; overflow: hidden; position: relative }
.desk { display: grid; grid-template-columns: 40% 1fr; height: 218px }
.side { border-right: 1px dashed #4a525c; padding: 8px 7px }
.main { padding: 10px 12px }
.phone { height: 218px; padding: 8px 7px; border-radius: 16px }
.search { height: 16px; border: 1px dashed #4a525c; border-radius: 6px; margin-bottom: 7px }
.chips { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 7px }
.phone .chips { flex-wrap: nowrap; overflow: hidden }
.chip { padding: 1px 7px; border: 1px dashed #5a636e; border-radius: 99px; font-size: 11.5px; white-space: nowrap; color: #9aa3ad }
.note { padding: 4px 5px; border-radius: 6px; margin-bottom: 2px }
.note b { display: block; font-size: 11.5px; font-weight: 600 }
.picked { background: #1c2127 }
.line { display: block; height: 5px; margin: 5px 0; border-radius: 3px; background: #2a3037 }
.title { display: block; margin-bottom: 6px; font-size: 16px }
.tags { margin-bottom: 7px }
.tag { display: flex; justify-content: space-between; padding: 2px 5px; font-size: 11.5px; color: #9aa3ad }
.drawer { margin: 4px 0 6px; font-size: 11.5px; color: #7d8793 }
/* color */
/* polish */
</style>
<header><h1>Tag filter</h1><span class="sub">Two directions · desktop and phone</span></header>
<section class="option a">
	<div class="label"><b>A</b>Chip row<span class="badge">Recommended</span></div>
	<div class="pair">
		${desk(`<div class="search"></div><div class="chips"><span class="chip on" data-n="3">#coast</span><span class="chip" data-n="2">#recipes</span><span class="chip" data-n="1">#work</span><span class="chip clear">× Clear</span></div>${notes}`, editor)}
		<div class="phone"><div class="search"></div><div class="chips"><span class="chip on" data-n="3">#coast</span><span class="chip" data-n="2">#recipes</span><span class="chip" data-n="1">#work</span></div>${notes}</div>
	</div>
</section>
<section class="option b">
	<div class="label"><b>B</b>Tag drawer</div>
	<div class="pair">
		${desk(`<div class="search"></div><div class="drawer">Tags ▾</div><div class="tags">${['coast', 'recipes', 'work', 'weekend'].map((tag, index) => `<div class="tag${index ? '' : ' on'}"><span>#${tag}</span><span>${[3, 2, 1, 2][index]}</span></div>`).join('')}</div>${notes}`, editor)}
		<div class="phone"><div class="search"></div><div class="drawer">Tags ▴</div>${notes}<div class="sheet"><b>Filter by tag</b><div class="chips"><span class="chip on">#coast</span><span class="chip">#recipes</span><span class="chip">#work</span><span class="chip">#weekend</span></div></div></div>
	</div>
</section>`;

const color = `.desk, .phone { border: 1px solid #2b323a; background: #161b20 }
.side { border-right: 1px solid #2b323a; background: #13171c }
.search { border: 1px solid #2c333b; background: #101317 }
.chip { border: 0; background: #232a31; color: #b4bec9 }
.chip.on, .tag.on { background: #0e7490; color: #ecfeff }
.chip[data-n]::after { content: attr(data-n); margin-left: 5px; opacity: 0.65 }
.tag.on { border-radius: 6px }
.title { font-family: Georgia, serif; color: #f1ede7 }
.picked { background: #232a31 }
/* polish */`;

const polish = `.option.a { border-color: #0e7490; box-shadow: 0 0 0 3px #0e749033 }
.badge { display: inline-block; padding: 1px 8px; border-radius: 99px; background: #134e4a; color: #5eead4; font-size: 10.5px }
.chip.clear { display: inline-block; background: none; border: 1px solid #3a434d; color: #9aa3ad }
.sheet { display: block; position: absolute; inset: auto 0 0; padding: 8px 7px 10px; border-radius: 12px 12px 0 0; background: #1f252c; box-shadow: 0 -8px 18px #0008; font-size: 11.5px }
.sheet .chips { flex-wrap: wrap; margin: 6px 0 0 }`;

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

test('mockups open on a live canvas that keeps every version', async ({ browser, baseURL }) => {
	test.setTimeout(180_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	project(root, 'tidepool');
	const canvases = join(root, 'data', 'canvases', 'design');
	const gates = join(root, 'gates');
	mkdirSync(canvases, { recursive: true });
	mkdirSync(gates);
	const path = join(canvases, 'tag-filter.html');
	const chat = new Chat(seed)
		.prompt()
		.say("I'll sketch two directions on a canvas, each on desktop and phone.")
		.create(path, mockup);
	chat.entries.push({ gate: 'opened' });
	const fixture = join(root, 'canvas.jsonl');
	chat
		.sleep(0.3)
		.say('Going with the chip row: color, and a note count on each tag.')
		.edit(path, '/* color */\n/* polish */', color)
		.sleep(0.8)
		.say('Last pass: a clear chip, and a sheet for the drawer on phones.')
		.edit(path, '/* polish */', polish)
		.say('Both directions are on the canvas. I recommend **A**.')
		.end('Done.')
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
	await desktop.getByRole('button', { name: 'New Tab' }).first().click();
	await desktop.getByRole('menuitem', { name: 'Claude', exact: true }).click();
	const tab = desktop.getByRole('tab', { name: /^Claude/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const id = (await tab.getAttribute('data-tab'))!;
	const panel = desktop.getByRole('tabpanel').filter({ has: desktop.getByRole('textbox', { name: 'Composer' }) });
	await desktop.mouse.move(1000, 300);

	const director = new Director('canvas');
	await director.record('desktop', desktop);
	await director.hold(0.8);
	director.mark('desktop', 'start');
	await send(desktop, id, prompt);
	director.mark('desktop', 'sent');
	const card = panel.locator('.canvas').filter({ hasText: 'Wrote canvas' });
	await expect(card).toBeVisible({ timeout: 30_000 });
	await expect.poll(() => existsSync(join(gates, 'opened')), { timeout: 15_000 }).toBe(true);
	await director.focus('desktop', 'card', card);
	await director.hold(0.6);
	await director.click('desktop', card.getByRole('button', { name: 'Open', exact: true }));
	const canvas = desktop.getByRole('region', { name: 'tag-filter' });
	const content = canvas.locator('iframe').contentFrame();
	await expect(content.getByRole('heading', { name: 'Tag filter' })).toBeVisible();
	await expect(desktop.getByRole('tablist', { name: 'Right Tabs' }).getByRole('tab', { name: 'tag-filter' })).toHaveAttribute('aria-selected', 'true');
	director.mark('desktop', 'opened');
	await director.focus('desktop', 'canvas', canvas);
	await director.rest('desktop', desktop, 560, 640);
	await director.hold(0.7);
	rmSync(join(gates, 'opened'));
	await expect(canvas.getByText('v2 of 2')).toBeVisible({ timeout: 30_000 });
	director.mark('desktop', 'v2');
	await expect(canvas.getByText('v3 of 3')).toBeVisible({ timeout: 30_000 });
	director.mark('desktop', 'v3');
	await expect(tab.getByTitle('done')).toBeVisible({ timeout: 30_000 });
	await director.hold(0.8);
	await director.focus('desktop', 'versions', canvas.locator('.heading'));
	director.mark('desktop', 'flip');
	const previous = canvas.getByRole('button', { name: 'Previous version' });
	await director.click('desktop', previous);
	await expect(canvas.getByText('v2 of 3')).toBeVisible();
	await director.hold(0.7);
	await director.click('desktop', previous, 0.35);
	await expect(canvas.getByText('v1 of 3')).toBeVisible();
	await director.hold(1);
	const next = canvas.getByRole('button', { name: 'Next version' });
	await director.click('desktop', next, 0.5);
	await expect(canvas.getByText('v2 of 3')).toBeVisible();
	await director.hold(0.7);
	await director.click('desktop', next, 0.35);
	await expect(next).toBeDisabled();
	await director.hold(0.4);
	await director.rest('desktop', desktop, 560, 420);
	await director.hold(0.6);
	director.mark('desktop', 'end');
	await director.finish();
});
