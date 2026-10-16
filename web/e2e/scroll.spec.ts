import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { openProject, screen } from './shell.ts';

// A smooth phone slide that lands just as the next one starts can snap back to the screen it left.
test.use({ reducedMotion: 'reduce' });

const report = /\^\[\[<64;\d+;\d+M/;

async function newTerminal(page: Page, name: string) {
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
}

async function run(page: Page, command: string) {
	await page.keyboard.type(command);
	await page.keyboard.press('Enter');
}

// A fresh shell can drop input typed while its prompt is still starting up.
async function scrollback(page: Page) {
	await expect(async () => {
		await run(page, 'seq 1 300');
		await expect.poll(() => screen(page), { timeout: 2_000 }).toMatch(/^300$/m);
	}).toPass();
}

// Asks for SGR mouse reports the way Claude Code's fullscreen TUI does, then echoes what arrives.
async function mouseApp(page: Page) {
	await run(page, String.raw`printf '\e[?1049h\e[?1000h\e[?1006h'; stty -icanon -echo; command cat -v`);
	await expect.poll(() => page.evaluate(() => window.terminal?.hasMouseTracking())).toBe(true);
}

function viewport(page: Page) {
	return page.evaluate(() => window.terminal?.getViewportY());
}

async function swipe(page: Page, target: Locator, dy: number, across = 0.5) {
	const box = (await target.boundingBox())!;
	const x = box.x + box.width * across;
	const y = box.y + box.height / 2 - dy / 2;
	const cdp = await page.context().newCDPSession(page);
	await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
	for (let step = 1; step <= 10; step++) {
		await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x, y: y + (dy * step) / 10 }] });
	}
	await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
}

test('the phone scroll rail scrolls a shell and sends a mouse app wheel reports', async ({ page }, { project }) => {
	test.skip(project.name !== 'pixel', 'the rail is phone only');
	await newTerminal(page, 'scroll-pixel');
	expect(await page.evaluate(() => getComputedStyle(document.documentElement).overscrollBehaviorY)).toBe('none');
	await scrollback(page);
	const rail = page.locator('.rail').filter({ visible: true });
	await swipe(page, rail, 200, -0.6);
	await expect.poll(() => viewport(page)).toBeGreaterThan(0);
	await swipe(page, rail, -600);
	await expect.poll(() => viewport(page)).toBe(0);
	await mouseApp(page);
	await swipe(page, rail, 100);
	await expect.poll(() => screen(page)).toMatch(report);
});

test('the desktop wheel scrolls a shell and sends a mouse app wheel reports, not arrows', async ({ page }, { project }) => {
	test.skip(project.name !== 'desktop', 'the wheel is desktop only');
	await newTerminal(page, 'scroll-desktop');
	await scrollback(page);
	const canvas = page.locator('.terminal canvas').filter({ visible: true });
	const box = (await canvas.boundingBox())!;
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.wheel(0, -200);
	await expect.poll(() => viewport(page)).toBeGreaterThan(0);
	await mouseApp(page);
	await page.mouse.wheel(0, -200);
	await expect.poll(() => screen(page)).toMatch(report);
	expect(await screen(page)).not.toMatch(/\^\[\[A/);
});
