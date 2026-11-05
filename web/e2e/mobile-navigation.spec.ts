import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';
import { newTab, openProject, openSettings, openSwitcher, showScreen } from './shell.ts';

async function settled(page: Page, name: string) {
	await expect.poll(() => page.locator('.screens').evaluate((element, name) => {
		const target = element.querySelector<HTMLElement>(`[data-screen="${name}"]`)!;
		return !target.inert && Math.abs(element.scrollLeft - target.offsetLeft) < 1 && getComputedStyle(element).scrollSnapType === 'x mandatory';
	}, name), { intervals: [16, 32, 50] }).toBe(true);
}

function back(page: Page) {
	return page.locator('.phone > header > button').first();
}

for (const side of ['left', 'right']) {
	test(`phone screens follow the ${side} sidebar setting, name their destinations and keep the current tab`, { tag: '@phone' }, async ({ page }) => {
		const title = page.getByRole('heading', { level: 1 });
		await openProject(page, 'alpha');
		await showScreen(page, 'sidebar');
		await page.getByRole('complementary', { name: 'Sidebar' }).getByRole('button', { name: 'greeting.ts', exact: true }).click();
		await settled(page, 'tabs');
		await openSettings(page, 'General');
		const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
		await settings.getByRole('combobox', { name: 'Sidebar position' }).selectOption(side);
		await settings.getByRole('button', { name: 'Done', exact: true }).click();
		await settled(page, 'projects');
		const order = side === 'left' ? ['projects', 'sidebar', 'tabs'] : ['projects', 'tabs', 'sidebar'];
		expect(await page.locator('.screen').evaluateAll((elements) => elements.map((element) => (element as HTMLElement).dataset.screen))).toEqual(order);

		const destinations = { projects: 'Projects', sidebar: 'Files', tabs: 'Tabs' } as const;
		for (const from of order as (keyof typeof destinations)[]) {
			await showScreen(page, from);
			const index = order.indexOf(from);
			const to = (order[index - 1] ?? order[index + 1]) as keyof typeof destinations;
			await expect(back(page)).toHaveAccessibleName(destinations[to]);
			await back(page).click();
			await settled(page, to);
		}

		await showScreen(page, 'tabs');
		await expect(title).toHaveText(/^greeting\.ts/);
		await showScreen(page, 'sidebar');
		await page.getByRole('complementary', { name: 'Sidebar' }).getByRole('button', { name: 'greeting.ts', exact: true }).click();
		await settled(page, 'tabs');
		await expect(title).toHaveText(/^greeting\.ts/);
		await showScreen(page, 'projects');
		await expect(title).toHaveText('Projects');
		await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'alpha', exact: true }).click();
		await settled(page, 'tabs');
		await page.reload();
		await settled(page, 'tabs');
		expect(await page.locator('.screen').evaluateAll((elements) => elements.map((element) => (element as HTMLElement).dataset.screen))).toEqual(order);
		await expect(title).toHaveText(/^greeting\.ts/);
	});
}

test('phone chrome is one header whose title switches Tabs', { tag: '@phone' }, async ({ page }) => {
	await openProject(page, 'alpha');
	const title = page.getByRole('heading', { level: 1 });
	await showScreen(page, 'sidebar');
	await page.getByRole('complementary', { name: 'Sidebar' }).getByRole('button', { name: 'greeting.ts', exact: true }).click();
	await settled(page, 'tabs');
	await newTab(page, 'Fake Claude');
	const panel = page.locator('.panel[role="tabpanel"]:visible');
	const transcript = panel.locator('.transcript:visible');
	await expect(panel.getByRole('textbox', { name: 'Composer' })).toBeVisible();
	await expect(title).toContainText('alpha, 2 tabs');

	await expect(page.getByRole('tablist', { name: 'Tabs', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Settings', exact: true })).toBeHidden();
	await expect(page.getByRole('button', { name: 'Drawer', exact: true })).toBeHidden();
	await expect(panel.getByRole('navigation', { name: 'User messages' })).toHaveCount(0);
	const header = page.locator('.phone > header');
	await expect(page.locator('[data-screen="tabs"] header:visible')).toHaveCount(0);
	await expect.poll(async () => {
		const [bar, below] = await Promise.all([header.boundingBox(), transcript.boundingBox()]);
		return { height: bar!.height <= 64, top: below!.y <= 64, adjacent: below!.y - (bar!.y + bar!.height) <= 1 };
	}).toEqual({ height: true, top: true, adjacent: true });

	const switcher = await openSwitcher(page);
	const tabs = switcher.getByRole('list', { name: 'Open Tabs' }).getByRole('listitem');
	await expect(tabs).toHaveCount(2);
	await expect(switcher.locator('[aria-current="true"]')).toHaveCount(1);
	await expect(switcher.getByRole('region', { name: 'New Tab' }).getByRole('button', { name: 'Fake Claude', exact: true })).toBeVisible();
	await expect(switcher.getByRole('region', { name: 'New Tab' }).getByRole('button', { name: 'Terminal', exact: true })).toBeVisible();
	await expect.poll(() => switcher.locator('.row').evaluateAll((rows) => Math.min(...rows.map((row) => row.getBoundingClientRect().height)))).toBeGreaterThanOrEqual(48);
	await switcher.getByRole('button', { name: 'greeting.ts', exact: true }).click();
	await expect(switcher).toBeHidden();
	await expect(title).toHaveText(/^greeting\.ts/);

	await (await openSwitcher(page)).getByRole('button', { name: 'Terminal', exact: true }).click();
	await expect(title).toContainText('alpha, 3 tabs');
});

test('phone Projects screen holds Settings and the Drawer', { tag: '@phone' }, async ({ page }) => {
	await page.goto('/');
	await showScreen(page, 'projects');
	const header = page.locator('.phone > header');
	await header.getByRole('button', { name: 'Settings', exact: true }).click();
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	await expect(settings).toBeVisible();
	await settings.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(settings).toBeHidden();
	await header.getByRole('button', { name: 'Drawer', exact: true }).click();
	await expect(page.getByRole('region', { name: 'Drawer' })).toBeVisible();
});

test('phone swipes switch screens in both directions', { tag: '@phone' }, async ({ page }) => {
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
			await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
			await settled(page, direction === -1 ? 'sidebar' : 'projects');
		}
	} finally {
		await session.detach();
	}
});

test('phone left button respects reduced motion', { tag: '@phone' }, async ({ page }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/');
	await settled(page, 'tabs');
	await page.getByRole('button', { name: 'Files', exact: true }).click();
	await settled(page, 'sidebar');
	await page.getByRole('button', { name: 'Projects', exact: true }).click();
	await settled(page, 'projects');
	await expect(back(page)).toHaveAccessibleName('Files');
});
