import { expect, test } from './fixtures.ts';
import type { Locator, Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { openProject, screen, showSidebar } from './shell.ts';

declare global {
	interface Window {
		clicked?: number;
		animationFrames?: number;
	}
}

// A smooth phone slide that lands just as the next one starts can snap back to the screen it left.
test.use({ reducedMotion: 'reduce' });

const runs = 5;
const files = ['one.ts', 'two.ts', 'three.ts', 'four.ts', 'five.ts'];

function fixture(name: string) {
	const dir = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(dir, { recursive: true });
	const step = (n: number) => `export function step${n}(input: number): number {\n\treturn input * ${n} + ${n};\n}\n`;
	const source = Array.from({ length: 50 }, (_, n) => step(n)).join('\n');
	for (const file of files) writeFileSync(`${dir}/${file}`, source);
	return name;
}

function mark(page: Page, name: string, after = 0) {
	return page.evaluate(
		([name, after]) =>
			new Promise<number>((resolve) => {
				new PerformanceObserver((list, observer) => {
					const mark = list.getEntriesByName(name).find((mark) => mark.startTime > after);
					if (!mark) return;
					observer.disconnect();
					resolve(mark.startTime);
				}).observe({ type: 'mark', buffered: true });
			}),
		[name, after] as const
	);
}

async function newTerminal(page: Page) {
	const tabs = page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab');
	const count = await tabs.count();
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect(tabs).toHaveCount(count + 1);
	const id = (await tabs.last().getAttribute('data-tab'))!;
	await mark(page, `painted:${id}`);
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	return id;
}

async function measure(page: Page, target: Locator, tab: Locator) {
	await page.evaluate(() =>
		addEventListener('click', ({ timeStamp }) => (window.clicked = timeStamp), { capture: true, once: true })
	);
	await target.click();
	const [clicked, id] = await Promise.all([page.evaluate(() => window.clicked ?? Infinity), tab.getAttribute('data-tab')]);
	return (await mark(page, `painted:${id}`, clicked)) - clicked;
}

function check(name: string, times: number[], budget: number) {
	const median = times.toSorted((a, b) => a - b)[Math.floor(times.length / 2)];
	console.log(`${test.info().project.name} ${name}: median ${median.toFixed(1)} ms of ${times.map((time) => time.toFixed(1))}`);
	expect.soft(median, `${name} median`).toBeLessThanOrEqual(budget);
}

test('switching Tab or Project paints within 50 ms, and opening a file within 100 ms', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const [one, two] = [fixture(`perf-${project.name}-one`), fixture(`perf-${project.name}-two`)];
	await openProject(page, two);
	const other = await newTerminal(page);
	await openProject(page, one);
	const terminals = [await newTerminal(page), await newTerminal(page)];
	const tab = (id: string) => page.locator(`[data-tab="${id}"]`);

	const tabSwitches = [];
	for (let run = 0; run < runs; run++) {
		const id = terminals[run % 2];
		tabSwitches.push(await measure(page, tab(id), tab(id)));
	}
	check('Tab switch', tabSwitches, 50);

	const projectSwitches = [];
	for (let run = 0; run < runs; run++) {
		const [name, id] = run % 2 ? [one, terminals[0]] : [two, other];
		if (phone) await page.getByRole('button', { name: 'Previous screen' }).click();
		const entry = page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name, exact: true });
		projectSwitches.push(await measure(page, entry, tab(id)));
	}
	check('Project switch', projectSwitches, 50);

	const opens = [];
	for (const file of files) {
		const sidebar = await showSidebar(page);
		opens.push(await measure(page, sidebar.getByRole('button', { name: file, exact: true }), page.getByRole('tab', { name: file })));
	}
	check('Open file', opens, 100);
});

test('an idle Terminal draws nothing, and a hidden one catches up when shown', { tag: '@desktop' }, async ({ page }) => {
	await page.addInitScript(() => {
		const request = window.requestAnimationFrame.bind(window);
		window.animationFrames = 0;
		window.requestAnimationFrame = (callback) => request((time) => {
			window.animationFrames!++;
			callback(time);
		});
	});
	const frames = () => page.evaluate(async () => {
		const renderer = window.terminal!.renderer!;
		const render = renderer.render;
		let visible = 0;
		renderer.render = (...args) => (visible++, render.apply(renderer, args));
		const before = window.animationFrames!;
		await new Promise((resolve) => setTimeout(resolve, 2000));
		renderer.render = render;
		return { all: window.animationFrames! - before, beyondVisible: window.animationFrames! - before - visible };
	});
	const ready = () => expect.poll(() => page.evaluate(() => window.terminal?.hasBracketedPaste())).toBe(true);
	await openProject(page, fixture('perf-idle'));
	const first = await newTerminal(page);
	await page.locator('.terminal:visible [contenteditable="true"]').focus();
	await ready();
	const canvas = (await page.locator('.terminal:visible canvas').elementHandle())!;
	const pixels = () => canvas.evaluate((canvas: HTMLCanvasElement) => canvas.toDataURL());
	await expect.poll(async () => (await frames()).all, { message: 'frames while idle', timeout: 10_000 }).toBeLessThanOrEqual(2);

	const idle = await pixels();
	await page.keyboard.type('x');
	await expect.poll(pixels).not.toBe(idle);

	await page.keyboard.press('Backspace');
	await page.keyboard.type('sleep 1; seq 1 40');
	await page.keyboard.press('Enter');
	await newTerminal(page);
	await ready();
	expect((await frames()).beyondVisible, 'frames while hidden output arrives').toBeLessThanOrEqual(2);
	const hidden = await pixels();
	await page.locator(`[data-tab="${first}"]`).click();
	await expect.poll(pixels).not.toBe(hidden);
	await expect.poll(() => screen(page)).toMatch(/^40$/m);
});

test('a fresh load on the phone paints the active Terminal within 1.5 s', { tag: '@phone' }, async ({ page, context }) => {
	await openProject(page, fixture('perf-cold'));
	const terminal = await newTerminal(page);
	const deviceId = await page.evaluate(() => sessionStorage.getItem('deviceId'));
	const layout = () => page.request.get(`/api/devices/${deviceId}/layout`).then((response) => response.text());
	await expect.poll(layout).toContain(terminal);
	await page.close();

	const loads = [];
	for (let run = 0; run < runs; run++) {
		const fresh = await context.newPage();
		await fresh.addInitScript((id) => id && sessionStorage.setItem('deviceId', id), deviceId);
		const cdp = await context.newCDPSession(fresh);
		await cdp.send('Network.enable');
		await cdp.send('Network.setCacheDisabled', { cacheDisabled: true });
		await cdp.send('Network.emulateNetworkConditions', { offline: false, latency: 20, downloadThroughput: -1, uploadThroughput: -1 });
		await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
		await fresh.goto('/');
		loads.push(await mark(fresh, `painted:${terminal}`));
		await fresh.close();
	}
	check('Cold load', loads, 1500);
});
