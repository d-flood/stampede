import { expect, test } from './fixtures.ts';
import { openProject, screen, showSidebar } from './shell.ts';

test('text size defaults, changes, persists, and resets', async ({ page }, { project }) => {
	const defaultSize = project.name === 'pixel' ? 16 : 13;
	await page.goto('/');
	await expect(page.locator('body')).toHaveCSS('font-size', `${defaultSize}px`);
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	const size = settings.getByRole('combobox', { name: 'Text size', exact: true });
	await expect(size).toHaveValue(String(defaultSize));
	await expect(settings.getByRole('button', { name: 'Reset', exact: true })).toBeDisabled();
	if (project.name === 'pixel') {
		await expect.poll(() => settings.evaluate((element) => {
			const box = element.getBoundingClientRect();
			return box.top === 0 && box.left === 0 && box.width === innerWidth && box.height === innerHeight;
		})).toBe(true);
	}
	await size.selectOption('22');
	await expect(page.locator('body')).toHaveCSS('font-size', '22px');
	await expect(settings.locator('small')).toHaveCSS('font-size', '20px');
	await expect.poll(() => settings.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await page.screenshot({ path: test.info().outputPath('settings-large.png') });
	await settings.getByRole('button', { name: 'Done', exact: true }).click();
	await expect(settings).toBeHidden();
	await page.reload();
	await expect(page.locator('body')).toHaveCSS('font-size', '22px');
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	await expect(size).toHaveValue('22');
	await settings.getByRole('button', { name: 'Reset', exact: true }).click();
	await expect(page.locator('body')).toHaveCSS('font-size', `${defaultSize}px`);
	await expect(size).toHaveValue(String(defaultSize));
	await page.reload();
	await expect(page.locator('body')).toHaveCSS('font-size', `${defaultSize}px`);
});

test('text size updates a running terminal and the editor', async ({ page }) => {
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'greeting.ts', exact: true }).click();
	await expect(page.locator('.cm-editor')).toBeVisible();
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	const input = page.getByRole('tabpanel').locator('.terminal textarea');
	await input.focus();
	await page.keyboard.type('echo size-before');
	await page.keyboard.press('Enter');
	await expect.poll(() => screen(page)).toMatch(/^size-before$/m);
	const columns = await page.evaluate(() => window.terminal!.cols);
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	await settings.getByRole('combobox', { name: 'Text size', exact: true }).selectOption('20');
	await expect.poll(() => page.evaluate(() => window.terminal?.options.fontSize)).toBe(20);
	await expect.poll(() => page.evaluate(() => window.terminal!.cols)).toBeLessThan(columns);
	await expect(page.locator('.cm-editor')).toHaveCSS('font-size', '20px');
	await expect.poll(() => screen(page)).toMatch(/^size-before$/m);
	await settings.getByRole('button', { name: 'Done', exact: true }).click();
	await input.focus();
	await page.keyboard.type('echo size-after');
	await page.keyboard.press('Enter');
	await expect.poll(() => screen(page)).toMatch(/^size-after$/m);
	await page.reload();
	await expect.poll(() => page.evaluate(() => window.terminal?.options.fontSize)).toBe(20);
	await expect.poll(() => screen(page)).toMatch(/^size-after$/m);
});
