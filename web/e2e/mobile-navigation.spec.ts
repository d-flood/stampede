import { expect, test } from './fixtures.ts';
import type { Locator, Page } from '@playwright/test';
import { expectCurrent, newTab, openProject, showSidebar, touchSwipe } from './shell.ts';

async function swipeAcross(page: Page, distance: number) {
	const box = (await page.locator('.phone .page').boundingBox())!;
	await touchSwipe(page, box.x + box.width / 2, box.y + box.height / 3, distance);
}

async function liftAway(tab: Locator) {
	const box = (await tab.boundingBox())!;
	await touchSwipe(tab.page(), box.x + box.width / 2, box.y + box.height / 2, -150, true);
}

function typing(page: Page) {
	return page.evaluate(() => !!document.activeElement?.matches('input, textarea, [contenteditable="true"]'));
}

test('phone swipes open Projects and the Sidebar over the Tab, and back or a choice closes them', { tag: '@phone' }, async ({ page }) => {
	await openProject(page, 'alpha');
	const projects = page.getByRole('navigation', { name: 'Projects' });
	const sidebar = page.getByRole('complementary', { name: 'Sidebar' });

	await swipeAcross(page, 180);
	await expect(projects).toBeVisible();
	await page.goBack();
	await expect(projects).toBeHidden();

	await swipeAcross(page, -180);
	await expect(sidebar).toBeVisible();
	await sidebar.getByRole('button', { name: 'greeting.ts', exact: true }).click();
	await expect(sidebar).toBeHidden();
	await expectCurrent(page, 'greeting.ts');

	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true });
	const greeting = tabs.getByRole('tab', { name: 'greeting.ts', exact: true });
	for (const [origin, distance] of [[greeting, 180], [tabs, -180]] as const) {
		const box = (await origin.boundingBox())!;
		await touchSwipe(page, box.x + box.width / 2, box.y + box.height / 2, distance);
		await expect(projects).toBeHidden();
		await expect(sidebar).toBeHidden();
		await expect(greeting).toHaveAttribute('aria-selected', 'true');
	}

	for (const name of ['README.md', 'desktop.txt', 'pixel.txt']) {
		await (await showSidebar(page)).getByRole('button', { name, exact: true }).click();
		await expectCurrent(page, name);
	}
	const offset = () => tabs.evaluate((element) => element.scrollLeft);
	await expect.poll(offset).toBeGreaterThan(0);
	const box = (await tabs.boundingBox())!;
	const y = box.y + box.height / 2;
	await touchSwipe(page, box.x + box.width / 3, y, 220);
	await expect.poll(offset).toBe(0);
	await touchSwipe(page, box.x + box.width / 3, y, 220);
	await expectCurrent(page, 'pixel.txt');
	await expect(projects).toBeHidden();
	await touchSwipe(page, box.x + box.width * 0.8, y, -220);
	await expect.poll(offset).toBeGreaterThan(0);
	await expect(sidebar).toBeHidden();
});

test('phone Tab strip switches without the keyboard and closes Tabs dragged up unless they ask first', { tag: '@phone' }, async ({ page }) => {
	await openProject(page, 'alpha');
	await newTab(page, 'Fake Claude');
	await newTab(page, 'Terminal');
	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true });
	expect((await tabs.boundingBox())!.y).toBeGreaterThan(page.viewportSize()!.height * 0.8);

	const chat = tabs.getByRole('tab', { name: /^Fake Claude/ });
	await chat.click();
	await expect(chat).toHaveAttribute('aria-selected', 'true');
	await expect(page.getByRole('textbox', { name: 'Composer' })).toBeVisible();
	expect(await typing(page)).toBe(false);
	const terminal = tabs.getByRole('tab').last();
	await terminal.click();
	await expect(terminal).toHaveAttribute('aria-selected', 'true');
	expect(await typing(page)).toBe(false);

	await liftAway(terminal);
	await expect(tabs.getByRole('tab')).toHaveCount(1);

	await (await showSidebar(page)).getByRole('button', { name: 'greeting.ts', exact: true }).click();
	await page.locator('.editor:not([hidden]) .cm-content').click();
	await page.keyboard.type('unsaved');
	const file = tabs.getByRole('tab', { name: 'greeting.ts' });
	await liftAway(file);
	const confirm = page.getByRole('dialog', { name: 'Discard unsaved changes?' });
	await confirm.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(file).toBeVisible();
	expect((await file.boundingBox())!.y).toBeGreaterThanOrEqual((await tabs.boundingBox())!.y - 1);
});
