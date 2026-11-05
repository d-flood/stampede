import { expect, test } from './fixtures.ts';
import { openProject, showSidebar } from './shell.ts';

test('desktop sidebar resizes by dragging and keyboard within its limits', { tag: '@desktop' }, async ({ page }) => {
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
	await expect.poll(async () => (await usage.boundingBox())?.height).toBe(60);
	for (const name of ['Claude', 'Codex']) {
		const agent = usage.getByRole('group', { name, exact: true });
		await expect(agent.locator('.brand svg')).toBeVisible();
		await expect(agent.getByRole('meter')).toHaveCount(1);
	}
	await expect(usage.locator('[title]')).toHaveCount(0);
	const trigger = usage.getByRole('button', { name: 'Usage details' });
	await trigger.hover();
	const tooltip = page.getByRole('tooltip');
	await expect(tooltip.locator('.countdown')).toHaveCount(4);
	await expect.poll(() => tooltip.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		return bounds.left >= 0 && bounds.right <= innerWidth && bounds.top >= 0 && bounds.bottom <= innerHeight;
	})).toBe(true);
	await page.keyboard.press('Escape');
	await expect(tooltip).toBeHidden();
	await trigger.focus();
	await page.keyboard.press('Enter');
	const details = page.getByRole('dialog', { name: 'Usage details' });
	await expect(details.locator('.countdown')).toHaveCount(4);
	await page.keyboard.press('Escape');
	await expect(details).toBeHidden();
	await resizer.focus();
	await page.mouse.move(900, 400);
	await expect.poll(() => usage.locator('.row').evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	const edge = (await resizer.boundingBox())!;
	await page.mouse.move(edge.x + edge.width / 2, edge.y + 200);
	await page.mouse.down();
	await page.mouse.move(0, 200);
	await expect(resizer).toHaveAttribute('aria-valuenow', '160');
	await page.mouse.move(1200, 200);
	await expect(resizer).toHaveAttribute('aria-valuenow', '640');
	await page.mouse.up();
	await expect(usage.getByRole('meter')).toHaveCount(4);
	await expect.poll(async () => (await usage.boundingBox())?.height).toBe(40);
	await expect.poll(async () => (await page.locator('main').boundingBox())!.width).toBeGreaterThan(500);
	await page.setViewportSize({ width: 800, height: 800 });
	await expect(resizer).toHaveAttribute('aria-valuemax', '480');
	await expect(resizer).toHaveAttribute('aria-valuenow', '480');
});

test('the sidebar toggle moves between the panel and tab strip while preserving focus', { tag: '@desktop' }, async ({ page }) => {
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);

	const docked = sidebar.getByRole('button', { name: 'Sidebar', exact: true });
	const strip = page.getByRole('main').getByRole('button', { name: 'Sidebar', exact: true });
	await expect(strip).toHaveCount(0);
	const [panel, corner] = [(await sidebar.boundingBox())!, (await docked.boundingBox())!];
	expect(panel.x + panel.width - (corner.x + corner.width)).toBeLessThan(12);
	expect(corner.y - panel.y).toBeLessThan(12);

	await docked.focus();
	await page.keyboard.press('Enter');
	await expect(sidebar).toBeHidden();
	await expect(strip).toBeFocused();
	await page.keyboard.press('Enter');
	await expect(sidebar).toBeVisible();
	await expect(strip).toHaveCount(0);
	await expect(docked).toBeFocused();
});

test('A saved Changes, History or Search section reopens where it moved', { tag: '@desktop' }, async ({ page }) => {
	await openProject(page, 'alpha');
	const sections = (await showSidebar(page)).getByRole('group', { name: 'Sections' });
	await sections.getByRole('button', { name: 'Compare', exact: true }).click();
	let saved = '';
	await page.route('**/api/devices/*/layout', async (route) => {
		if (route.request().method() !== 'GET') return route.fallback();
		const response = await route.fetch();
		await route.fulfill({ response, json: { ...(await response.json()), sidebar: { open: true, section: saved } } });
	});
	for (const [old, moved] of [['changes', 'Git'], ['history', 'Git'], ['search', 'Files']]) {
		saved = old;
		await page.reload();
		await expect(sections.getByRole('button', { name: moved, exact: true })).toHaveAttribute('aria-pressed', 'true');
	}
});
