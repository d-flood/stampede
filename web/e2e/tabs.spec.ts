import { expect, test } from '@playwright/test';
import { openProject, showSidebar } from './shell.ts';

test('active tabs expose a close button and preserve unsaved edits', async ({ page }, { project }) => {
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	const file = `${project.name}.txt`;
	await sidebar.getByRole('button', { name: file, exact: true }).click();
	if (project.name === 'pixel') {
		await expect.poll(() => page.locator('.screens').evaluate((element) =>
			Math.abs(element.scrollLeft - element.clientWidth) < 1
		)).toBe(true);
	}
	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true });
	const close = tabs.getByRole('button', { name: `Close ${file}`, exact: true });
	await expect(close).toBeVisible();
	await expect.poll(async () => {
		const box = await close.boundingBox();
		return box && box.width >= (project.name === 'pixel' ? 48 : 26) && box.height >= (project.name === 'pixel' ? 48 : 26);
	}).toBe(true);
	await page.screenshot({ path: test.info().outputPath('close-file-tab.png') });
	await page.locator('.editor:not([hidden]) .cm-content').click();
	await page.keyboard.type('unsaved');
	if (project.name === 'pixel') await close.tap();
	else await close.click();
	const dialog = page.getByRole('dialog', { name: 'Discard unsaved changes?' });
	await expect(dialog).toBeVisible();
	await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(tabs.getByRole('tab', { name: file })).toHaveAttribute('aria-selected', 'true');
	await close.click();
	await dialog.getByRole('button', { name: 'Discard', exact: true }).click();
	await expect(tabs.getByRole('tab', { name: file })).toHaveCount(0);
});

test('terminal and drawer tabs close directly on desktop and tablet', async ({ page }, { project }) => {
	if (project.name === 'pixel') await page.setViewportSize({ width: 1024, height: 768 });
	await page.goto('/');
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await page.getByRole('option', { name: /\/beta$/ }).click();
	await expect(page.getByRole('dialog')).toBeHidden();
	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true });
	for (let i = 0; i < 2; i++) {
		await page.getByRole('button', { name: 'New Tab', exact: true }).click();
		await page.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
		await expect(tabs.getByRole('tab')).toHaveCount(i + 1);
	}
	await expect(tabs.getByRole('button', { name: /^Close / })).toHaveCount(1);
	const close = tabs.getByRole('button', { name: /^Close / });
	await expect.poll(async () => {
		const box = await close.boundingBox();
		return box && box.width >= (project.name === 'pixel' ? 48 : 26) && box.height >= (project.name === 'pixel' ? 48 : 26);
	}).toBe(true);
	await page.screenshot({ path: test.info().outputPath('close-terminal-tab.png') });
	if (project.name === 'pixel') await close.tap();
	else await close.click();
	await expect(tabs.getByRole('tab')).toHaveCount(1);
	await expect(tabs.getByRole('tab')).toHaveAttribute('aria-selected', 'true');
	await close.click();
	await expect(tabs.getByRole('tab')).toHaveCount(0);
	await page.keyboard.press('Control+Backquote');
	const drawer = page.getByRole('region', { name: 'Drawer' });
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).toBe('none');
	await drawer.getByRole('button', { name: 'New Drawer Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
	await expect(drawer.getByRole('tab')).toHaveCount(1);
	const closeDrawerTab = drawer.getByRole('button', { name: /^Close / });
	if (project.name === 'pixel') await closeDrawerTab.tap();
	else await closeDrawerTab.click();
	await expect(drawer.getByRole('tab')).toHaveCount(0);
});
