import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { openProject, screen, showSidebar } from './shell.ts';

test('theme selection updates the UI, editor, and running Terminal and survives reload', async ({ page }, { project }) => {
	let terminalId: string | undefined;
	page.on('websocket', (socket) => socket.on('framereceived', ({ payload }) => {
		if (typeof payload !== 'string') return;
		const message = JSON.parse(payload);
		if (message.type === 'terminal.created') terminalId = message.terminal.id;
	}));
	await openProject(page, project.name === 'desktop' ? 'alpha' : 'beta');
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect.poll(() => terminalId).toBeDefined();
	await expect(page.locator(`[data-tab="${terminalId}"]`)).toHaveAttribute('aria-selected', 'true');
	const input = page.getByRole('tabpanel').locator('.terminal textarea');
	await expect.poll(() => input.evaluate((element) => window.terminal?.textarea === element)).toBe(true);
	await input.focus();
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	await page.keyboard.type(`echo theme-before-${project.name}`);
	await page.keyboard.press('Enter');
	await expect.poll(() => screen(page)).toMatch(new RegExp(`^theme-before-${project.name}$`, 'm'));
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	await page.getByRole('button', { name: 'Change theme' }).click();
	const dialog = page.getByRole('dialog', { name: 'Choose a theme' });
	const search = dialog.getByRole('combobox', { name: 'Search themes' });
	const dracula = dialog.getByRole('option', { name: /^Dracula(?: Current theme)?$/ });
	await search.fill('no-theme-matches-this');
	await expect(dialog).toContainText('No matching themes');
	await search.fill('Dracula');
	await expect(dracula).toBeVisible();
	if (project.name === 'desktop') await search.press('Enter');
	else await dracula.click();
	await expect(dialog).toBeHidden();
	await expect(page.getByRole('button', { name: 'Change theme' })).toHaveAttribute('title', 'Theme: Dracula');
	await page.getByRole('dialog', { name: 'Settings', exact: true }).getByRole('button', { name: 'Done', exact: true }).click();
	await expect.poll(() => page.locator('body').evaluate((element) => getComputedStyle(element).backgroundColor)).toBe('rgb(40, 42, 54)');
	await expect.poll(() => page.evaluate(() => window.terminal?.options.theme?.background)).toBe('#282a36');
	await expect.poll(() => screen(page)).toMatch(new RegExp(`^theme-before-${project.name}$`, 'm'));
	await input.focus();
	await page.keyboard.type(`echo theme-after-${project.name}`);
	await page.keyboard.press('Enter');
	await expect.poll(() => screen(page)).toMatch(new RegExp(`^theme-after-${project.name}$`, 'm'));
	expect(readFileSync(`${process.env.OIKONOMOS_E2E_ROOT}/config.toml`, 'utf8')).toContain('theme = "Dracula"');

	await page.reload();
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	await expect(page.getByRole('button', { name: 'Change theme' })).toHaveAttribute('title', 'Theme: Dracula');
	await expect.poll(() => screen(page)).toMatch(new RegExp(`^theme-after-${project.name}$`, 'm'));
	await page.getByRole('button', { name: 'Change theme' }).click();
	await dialog.getByRole('combobox', { name: 'Search themes' }).fill('Dracula');
	await expect(dracula.getByLabel('Current theme')).toBeVisible();
	await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
	if (project.name === 'pixel') {
		await expect.poll(() => dialog.evaluate((element) => {
			const box = element.getBoundingClientRect();
			return box.top === 0 && box.left === 0 && box.width === window.innerWidth && box.height === window.innerHeight;
		})).toBe(true);
	}
	await page.screenshot({ path: test.info().outputPath('theme-picker.png') });
	await page.keyboard.press('Escape');
	await expect(dialog).toBeHidden();
	await page.getByRole('dialog', { name: 'Settings', exact: true }).getByRole('button', { name: 'Done', exact: true }).click();

	if (project.name === 'desktop') {
		const sidebar = await showSidebar(page);
		await sidebar.getByRole('button', { name: 'greeting.ts', exact: true }).click();
		await expect.poll(() => page.locator('.cm-editor').evaluate((element) => getComputedStyle(element).backgroundColor)).toBe('rgb(40, 42, 54)');
	}
});
