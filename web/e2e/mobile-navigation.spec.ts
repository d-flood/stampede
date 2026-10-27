import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';
import { openProject, showScreen } from './shell.ts';

async function settled(page: Page, name: string) {
	await expect.poll(() => page.locator('.screens').evaluate((element, name) => {
		const target = element.querySelector<HTMLElement>(`[data-screen="${name}"]`)!;
		return !target.inert && Math.abs(element.scrollLeft - target.offsetLeft) < 1 && getComputedStyle(element).scrollSnapType === 'x mandatory';
	}, name), { intervals: [16, 32, 50] }).toBe(true);
}

for (const side of ['left', 'right']) {
	test(`phone screens follow the ${side} sidebar setting and keep the current tab`, { tag: '@phone' }, async ({ page }) => {
		await openProject(page, 'alpha');
		await showScreen(page, 'sidebar');
		await page.getByRole('complementary', { name: 'Sidebar' }).getByRole('button', { name: 'greeting.ts', exact: true }).click();
		await settled(page, 'tabs');
		await page.getByRole('button', { name: 'Settings', exact: true }).click();
		const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
		await settings.getByRole('combobox', { name: 'Sidebar position' }).selectOption(side);
		await settings.getByRole('button', { name: 'Done', exact: true }).click();
		await settled(page, 'tabs');
		await expect(page.getByRole('tab', { name: 'greeting.ts' })).toHaveAttribute('aria-selected', 'true');
		const order = side === 'left' ? ['projects', 'sidebar', 'tabs'] : ['projects', 'tabs', 'sidebar'];
		expect(await page.locator('.screen').evaluateAll((elements) => elements.map((element) => (element as HTMLElement).dataset.screen))).toEqual(order);
		await showScreen(page, 'sidebar');
		await page.getByRole('complementary', { name: 'Sidebar' }).getByRole('button', { name: 'greeting.ts', exact: true }).click();
		await settled(page, 'tabs');
		await expect(page.getByRole('tab', { name: 'greeting.ts' })).toHaveAttribute('aria-selected', 'true');
		await showScreen(page, 'projects');
		await expect(page.getByRole('heading', { level: 1 })).toHaveText('Projects');
		await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'alpha', exact: true }).click();
		await settled(page, 'tabs');
		await page.reload();
		await settled(page, 'tabs');
		expect(await page.locator('.screen').evaluateAll((elements) => elements.map((element) => (element as HTMLElement).dataset.screen))).toEqual(order);
		await expect(page.getByRole('tab', { name: 'greeting.ts' })).toHaveAttribute('aria-selected', 'true');
	});
}

test('phone swipes track the finger and settle quickly in both directions', { tag: '@phone' }, async ({ page }) => {
	await page.goto('/');
	await showScreen(page, 'projects');
	const session = await page.context().newCDPSession(page);
	try {
		for (const direction of [-1, 1]) {
			const x = direction === -1 ? 330 : 100;
			await session.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y: 400 }] });
			for (let step = 1; step <= 5; step++) {
				await session.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: x + direction * step * 28, y: 400 }] });
			}
			await expect.poll(() => page.locator('.screens').evaluate((element) => element.scrollLeft), { intervals: [16] }).toBeGreaterThan(50);
			await page.evaluate(() => {
				const element = document.querySelector<HTMLElement>('.screens')!;
				const samples: { time: number; left: number }[] = [];
				Object.assign(window, { slideSamples: samples });
				const start = performance.now();
				requestAnimationFrame(function frame(now) {
					samples.push({ time: now - start, left: element.scrollLeft });
					if (now - start < 350) requestAnimationFrame(frame);
				});
			});
			await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
			await settled(page, direction === -1 ? 'sidebar' : 'projects');
			await expect.poll(() => page.evaluate(() => (window as unknown as { slideSamples: { time: number }[] }).slideSamples.at(-1)!.time), { intervals: [16] }).toBeGreaterThan(300);
			const samples = await page.evaluate(() => (window as unknown as { slideSamples: { time: number; left: number }[] }).slideSamples);
			const destination = direction === -1 ? page.viewportSize()!.width : 0;
			const arrival = samples.findIndex((sample) => Math.abs(sample.left - destination) < 1);
			expect(arrival).toBeGreaterThan(0);
			expect(samples[arrival].time).toBeLessThan(250);
			expect(samples.slice(arrival).every((sample) => Math.abs(sample.left - destination) < 1)).toBe(true);
		}
	} finally {
		await session.detach();
	}
});

test('phone arrows respect reduced motion', { tag: '@phone' }, async ({ page }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/');
	await settled(page, 'tabs');
	await page.getByRole('button', { name: 'Previous screen' }).click();
	await settled(page, 'sidebar');
	await page.getByRole('button', { name: 'Previous screen' }).click();
	await settled(page, 'projects');
	await expect(page.getByRole('button', { name: 'Previous screen' })).toBeDisabled();
});
