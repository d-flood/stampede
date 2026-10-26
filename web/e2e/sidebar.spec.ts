import { expect, test } from './fixtures.ts';
import { openProject, showSidebar } from './shell.ts';

test('desktop sidebar resizes by dragging and keyboard within its limits', async ({ page }, { project }) => {
	test.skip(project.name !== 'desktop');
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	const resizer = page.getByRole('separator', { name: 'Resize sidebar' });
	const initial = (await sidebar.boundingBox())!;
	const handle = (await resizer.boundingBox())!;
	await page.mouse.move(handle.x + handle.width / 2, handle.y + 200);
	await page.mouse.down();
	await page.mouse.move(handle.x + handle.width / 2 + 120, handle.y + 200, { steps: 8 });
	await expect.poll(async () => (await sidebar.boundingBox())?.width).toBe(initial.width + 120);
	await page.mouse.up();
	await page.mouse.move(900, 400);
	await expect.poll(async () => (await sidebar.boundingBox())?.width).toBe(initial.width + 120);
	await page.getByRole('button', { name: 'Sidebar', exact: true }).click();
	await expect(sidebar).toBeHidden();
	await page.getByRole('button', { name: 'Sidebar', exact: true }).click();
	await expect.poll(async () => (await sidebar.boundingBox())?.width).toBe(initial.width + 120);
	await resizer.focus();
	await page.keyboard.press('ArrowLeft');
	await expect(resizer).toHaveAttribute('aria-valuenow', '424');
	await page.keyboard.press('Home');
	await expect(resizer).toHaveAttribute('aria-valuenow', '160');
	await expect.poll(async () => (await sidebar.boundingBox())?.width).toBe(162);
	const sections = sidebar.getByRole('group', { name: 'Sections' });
	await expect.poll(() => sections.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		const buttons = [...element.querySelectorAll('button')].map((button) => button.getBoundingClientRect());
		return buttons.every((button) => button.left >= bounds.left && button.right <= bounds.right)
			&& buttons.at(-1)!.top > buttons[0].top;
	})).toBe(true);
	await sections.getByRole('button', { name: 'Epics', exact: true }).click();
	await expect(sections.getByRole('button', { name: 'Epics', exact: true })).toHaveAttribute('aria-pressed', 'true');
	const usage = sidebar.getByRole('region', { name: 'Usage windows' });
	for (const name of ['Claude', 'Codex']) {
		const agent = usage.getByRole('group', { name, exact: true });
		await expect(agent.getByText(name, { exact: true })).toBeHidden();
		await expect(agent.locator('.brand svg')).toBeVisible();
	}
	for (const meter of await usage.locator('.meter').all()) {
		await expect(meter.locator('.ring')).toBeVisible();
		await expect(meter.locator('.label')).toBeHidden();
		await expect(meter.locator('.reset')).toBeHidden();
	}
	const firstRing = usage.getByRole('meter', { name: 'Claude · 5h' });
	await expect(usage.locator('[title]')).toHaveCount(0);
	await expect(firstRing).toHaveAttribute('data-delay-duration', '0');
	await firstRing.hover();
	const tooltip = page.getByRole('tooltip');
	await expect(tooltip).toHaveText(await usage.locator('.reset').first().evaluate((element) => [...element.children].map((child) => child.textContent).join(' ') || element.textContent!));
	await expect.poll(() => tooltip.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		const style = getComputedStyle(element);
		return bounds.left >= 0 && bounds.right <= innerWidth && bounds.top >= 0 && bounds.bottom <= innerHeight
			&& style.whiteSpace === 'pre-line' && style.borderTopWidth === '1px';
	})).toBe(true);
	await page.screenshot({ path: test.info().outputPath('usage-tooltip.png') });
	await page.keyboard.press('Escape');
	await expect(tooltip).toBeHidden();
	await usage.getByRole('meter', { name: 'Claude · week', exact: true }).focus();
	await expect(tooltip).toHaveText(await usage.locator('.reset').nth(1).evaluate((element) => [...element.children].map((child) => child.textContent).join(' ') || element.textContent!));
	await page.keyboard.press('Escape');
	await expect(tooltip).toBeHidden();
	await resizer.focus();
	await page.mouse.move(900, 400);
	await expect.poll(() => usage.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await usage.screenshot({ path: test.info().outputPath('usage-narrow.png') });
	await sidebar.screenshot({ path: test.info().outputPath('sidebar-narrow.png') });
	const edge = (await resizer.boundingBox())!;
	await page.mouse.move(edge.x + edge.width / 2, edge.y + 200);
	await page.mouse.down();
	await page.mouse.move(0, 200);
	await expect(resizer).toHaveAttribute('aria-valuenow', '160');
	await page.mouse.move(1200, 200);
	await expect(resizer).toHaveAttribute('aria-valuenow', '640');
	await page.mouse.up();
	for (const name of ['Claude', 'Codex']) await expect(usage.getByText(name, { exact: true })).toBeVisible();
	for (const meter of await usage.locator('.meter').all()) {
		await expect(meter.locator('.label')).toBeVisible();
		await expect(meter.locator('.reset')).toBeVisible();
	}
	await expect.poll(async () => (await page.locator('main').boundingBox())!.width).toBeGreaterThan(500);
	await page.screenshot({ path: test.info().outputPath('sidebar-wide.png') });
	await page.setViewportSize({ width: 800, height: 800 });
	await expect(resizer).toHaveAttribute('aria-valuemax', '480');
	await expect(resizer).toHaveAttribute('aria-valuenow', '480');
	await expect.poll(async () => (await sidebar.boundingBox())?.width).toBe(482);
});

test('phone sidebar fills its screen without a resize handle', async ({ page }, { project }) => {
	test.skip(project.name !== 'pixel');
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	await expect(page.getByRole('separator', { name: 'Resize sidebar' })).toHaveCount(0);
	await expect.poll(async () => (await sidebar.boundingBox())?.width).toBe(page.viewportSize()!.width);
});

test('desktop sidebar slides open and closed over several frames', async ({ page }, { project }) => {
	test.skip(project.name !== 'desktop');
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	const slide = () =>
		page.evaluate(
			() =>
				new Promise<number[]>((resolve) => {
					const main = document.querySelector('main')!;
					const lefts = [main.getBoundingClientRect().left];
					const start = performance.now();
					document.querySelector<HTMLButtonElement>('button[aria-label="Sidebar"]')!.click();
					requestAnimationFrame(function frame(now) {
						lefts.push(main.getBoundingClientRect().left);
						if (now - start < 500) requestAnimationFrame(frame);
						else resolve(lefts);
					});
				})
		);
	const closing = await slide();
	await expect(sidebar).toBeHidden();
	const opening = await slide();
	await expect(sidebar).toBeVisible();
	expect(opening.at(-1)).toBe(closing[0]);
	for (const lefts of [closing, opening]) {
		const [first, last] = [lefts[0], lefts.at(-1)!];
		expect(Math.abs(last - first)).toBeGreaterThan(150);
		expect(new Set(lefts.filter((left) => left > Math.min(first, last) + 0.5 && left < Math.max(first, last) - 0.5).map(Math.round)).size).toBeGreaterThanOrEqual(3);
	}
});
