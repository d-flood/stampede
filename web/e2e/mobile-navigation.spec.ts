import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';
import { newTab, openProject, showLevel, showSidebar, swipeClosed, touchSwipe } from './shell.ts';

function shown(page: Page, level: 'projects' | 'overview' | 'section' | 'tab') {
	return expect(page.locator(`[data-level="${level}"]`)).toBeVisible();
}

async function swipeTab(page: Page, distance: number) {
	const box = (await page.locator('[data-level="tab"]').boundingBox())!;
	await touchSwipe(page, box.x + box.width / 2, box.y + box.height / 2, distance);
}

test('phone levels step down from Projects to a Tab, and the header and back both go up', { tag: '@phone' }, async ({ page }) => {
	const header = page.locator('.phone > header');
	const title = page.getByRole('heading', { level: 1 });
	await openProject(page, 'alpha');
	await expect(title).toContainText('alpha');
	await page.locator('[data-level="overview"]').getByRole('button', { name: /^Files\b/ }).click();
	await shown(page, 'section');
	await page.getByRole('complementary', { name: 'Sidebar' }).getByRole('button', { name: 'greeting.ts', exact: true }).click();
	await shown(page, 'tab');
	await expect(title.locator('.name')).toHaveText('greeting.ts');
	await expect(page.getByRole('tablist', { name: 'Tabs', exact: true })).toHaveCount(0);
	await expect(header.getByRole('button', { name: 'Settings', exact: true })).toHaveCount(0);
	expect((await header.boundingBox())!.height).toBeLessThanOrEqual(64);

	await page.goBack();
	await shown(page, 'section');
	await page.goBack();
	await shown(page, 'overview');
	await page.getByRole('list', { name: 'Open Tabs' }).getByRole('button', { name: 'greeting.ts' }).click();
	await shown(page, 'tab');
	await header.getByRole('button', { name: 'alpha', exact: true }).click();
	await shown(page, 'overview');
	await showLevel(page, 'tab');
	await header.getByRole('button', { name: 'Projects', exact: true }).click();
	await shown(page, 'projects');

	await header.getByRole('button', { name: 'Drawer', exact: true }).click();
	await expect(page.getByRole('region', { name: 'Drawer' })).toBeVisible();
	await page.getByRole('button', { name: 'Close Drawer', exact: true }).click();
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'alpha', exact: true }).click();
	await shown(page, 'overview');
	await page.reload();
	await shown(page, 'overview');
	await page.goBack();
	await shown(page, 'projects');
});

test('phone overview closes a Tab by swipe and several at once after one confirm', { tag: '@phone' }, async ({ page }) => {
	await openProject(page, 'alpha');
	await newTab(page, 'Terminal');
	await newTab(page, 'Terminal');
	await (await showSidebar(page)).getByRole('button', { name: 'greeting.ts', exact: true }).click();
	await page.locator('.editor:not([hidden]) .cm-content').click();
	await page.keyboard.type('unsaved');
	await showLevel(page, 'overview');
	const rows = page.getByRole('list', { name: 'Open Tabs' }).getByRole('listitem');
	await expect(rows).toHaveCount(3);
	await swipeClosed(rows.first());
	await expect(rows).toHaveCount(2);

	await page.getByRole('button', { name: 'Select', exact: true }).click();
	for (const row of await rows.all()) await row.getByRole('checkbox').check();
	await page.getByRole('button', { name: 'Close 2 Tabs', exact: true }).click();
	const confirm = page.getByRole('dialog', { name: 'Close 2 Tabs?' });
	await expect(confirm).toContainText('Unsaved changes in greeting.ts are discarded.');
	await confirm.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(rows).toHaveCount(0);
	await expect(page.getByRole('region', { name: 'alpha', exact: true })).toBeVisible();
});

test('phone Tab swipes step through Tabs, stop at either end and keep the current dot in view', { tag: '@phone' }, async ({ page }) => {
	await openProject(page, 'alpha');
	await newTab(page, 'Fake Claude');
	for (let count = 0; count < 11; count++) await newTab(page, 'Terminal');
	const title = page.getByRole('heading', { level: 1 });
	const position = title.locator('.position');
	await expect(position).toHaveText('11/12');
	const dots = title.locator('.dot');
	await expect.poll(() => dots.last().evaluate((dot) => getComputedStyle(dot).backgroundColor)).not.toBe(await dots.first().evaluate((dot) => getComputedStyle(dot).backgroundColor));
	const inView = () => title.locator('.dots').evaluate((row) => {
		const box = row.getBoundingClientRect();
		const current = row.querySelector('.now')!.getBoundingClientRect();
		return current.left >= box.left && current.right <= box.right;
	});

	const swipes: [number, string][] = [[-120, '12/12'], [-120, '12/12'], ...Array.from({ length: 11 }, (): [number, string] => [120, '']), [120, '1/12'], [-120, '2/12']];
	for (const [distance, expected] of swipes) {
		await swipeTab(page, distance);
		if (expected) await expect(position).toHaveText(expected);
		if (expected === '12/12') await expect(title.locator('.name')).toContainText('Fake Claude');
	}
	await expect.poll(inView).toBe(true);
	await shown(page, 'tab');
	expect(await page.locator('.levels').evaluate((element) => element.getAnimations({ subtree: true }).length)).toBe(0);
});
