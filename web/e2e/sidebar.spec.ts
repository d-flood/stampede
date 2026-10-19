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
	await expect(resizer).toHaveAttribute('aria-valuenow', '240');
	const usage = sidebar.getByRole('region', { name: 'Usage windows' });
	for (const name of ['Claude', 'Codex']) {
		const agent = usage.getByRole('group', { name, exact: true });
		await expect(agent.getByText(name, { exact: true })).toBeHidden();
		await expect(agent.locator('.brand svg')).toBeVisible();
	}
	await expect.poll(() => usage.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await usage.screenshot({ path: test.info().outputPath('usage-narrow.png') });
	const edge = (await resizer.boundingBox())!;
	await page.mouse.move(edge.x + edge.width / 2, edge.y + 200);
	await page.mouse.down();
	await page.mouse.move(0, 200);
	await expect(resizer).toHaveAttribute('aria-valuenow', '240');
	await page.mouse.move(1200, 200);
	await expect(resizer).toHaveAttribute('aria-valuenow', '640');
	await page.mouse.up();
	for (const name of ['Claude', 'Codex']) await expect(usage.getByText(name, { exact: true })).toBeVisible();
	await expect.poll(async () => (await page.locator('main').boundingBox())!.width).toBeGreaterThan(500);
	await page.screenshot({ path: test.info().outputPath('sidebar-wide.png') });
	await page.setViewportSize({ width: 800, height: 800 });
	await expect(resizer).toHaveAttribute('aria-valuemax', '480');
	await expect(resizer).toHaveAttribute('aria-valuenow', '480');
	await expect.poll(async () => (await sidebar.boundingBox())?.width).toBe(481);
});

test('phone sidebar fills its screen without a resize handle', async ({ page }, { project }) => {
	test.skip(project.name !== 'pixel');
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	await expect(page.getByRole('separator', { name: 'Resize sidebar' })).toHaveCount(0);
	await expect.poll(async () => (await sidebar.boundingBox())?.width).toBe(page.viewportSize()!.width);
});
