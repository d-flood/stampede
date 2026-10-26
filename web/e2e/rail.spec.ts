import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';
import { openProject, screen } from './shell.ts';

test.beforeEach(({}, { project }) => {
	test.skip(project.name !== 'desktop', 'the rail is desktop only');
});

function toggle(page: Page) {
	return page.evaluate(
		() =>
			new Promise<{ widths: number[]; heights: number[] }>((resolve) => {
				const rail = document.querySelector<HTMLElement>('nav[aria-label="Projects"]')!;
				const square = rail.querySelector<HTMLElement>('[data-project]')!;
				const widths = [rail.getBoundingClientRect().width];
				const heights = [square.getBoundingClientRect().height];
				const start = performance.now();
				rail.querySelector<HTMLButtonElement>('button[aria-label$="project rail"]')!.click();
				requestAnimationFrame(function frame(now) {
					widths.push(rail.getBoundingClientRect().width);
					heights.push(square.getBoundingClientRect().height);
					if (now - start < 500) requestAnimationFrame(frame);
					else resolve({ widths, heights });
				});
			})
	);
}

function between(values: number[]) {
	const [first, last] = [values[0], values.at(-1)!];
	return new Set(values.filter((value) => value > Math.min(first, last) + 0.5 && value < Math.max(first, last) - 0.5).map((value) => Math.round(value * 10))).size;
}

test('the project rail and its squares grow and shrink over several frames', async ({ page }) => {
	await openProject(page, 'beta');
	await openProject(page, 'alpha');
	const opening = await toggle(page);
	expect(opening.widths.at(-1)! - opening.widths[0]).toBeGreaterThan(150);
	expect(opening.heights.at(-1)!).toBeGreaterThan(opening.heights[0]);
	expect(between(opening.widths)).toBeGreaterThanOrEqual(3);
	expect(between(opening.heights)).toBeGreaterThanOrEqual(3);
	const closing = await toggle(page);
	expect(closing.widths.at(-1)).toBe(opening.widths[0]);
	expect(closing.heights.at(-1)).toBe(opening.heights[0]);
	expect(between(closing.widths)).toBeGreaterThanOrEqual(3);
	expect(between(closing.heights)).toBeGreaterThanOrEqual(3);
});

test('a Terminal resizes once, not every frame, while the project rail or sidebar opens or closes', async ({ page }) => {
	const resizes: string[] = [];
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			if (typeof payload === 'string' && payload.includes('"terminal.resize"')) resizes.push(payload);
		})
	);
	await openProject(page, 'alpha');
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	const terminal = page.getByRole('tabpanel').locator('.terminal').filter({ visible: true });
	await expect(terminal).not.toHaveClass(/resizing/);
	for (const name of ['Expand project rail', 'Collapse project rail', 'Sidebar', 'Sidebar']) {
		const before = resizes.length;
		await page.getByRole('button', { name, exact: true }).click();
		await expect.poll(() => resizes.length).toBeGreaterThan(before);
		await expect(terminal).not.toHaveClass(/resizing/);
		expect(resizes.length - before).toBe(1);
	}
});
