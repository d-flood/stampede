import { expect, test } from './fixtures.ts';
import { readFileSync } from 'node:fs';
import type { Page } from '@playwright/test';
import { openProject, screen } from './shell.ts';

async function platformFont(page: Page, selector: string) {
	const session = await page.context().newCDPSession(page);
	try {
		await session.send('DOM.enable');
		await session.send('CSS.enable');
		const { root } = await session.send('DOM.getDocument');
		const { nodeId } = await session.send('DOM.querySelector', { nodeId: root.nodeId, selector });
		return (await session.send('CSS.getPlatformFontsForNode', { nodeId })).fonts;
	} finally {
		await session.detach();
	}
}

test('the UI font comes from the Host while code and terminals stay monospace', { tag: '@desktop' }, async ({ page }) => {
	const config = () => readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/config.toml`, 'utf8');
	const bodyFont = () => page.locator('body').evaluate((element) => getComputedStyle(element).fontFamily);
	await openProject(page, 'alpha');
	const families: string[] = await (await page.request.get('/api/fonts')).json();
	const family = families.find((name) => name === 'DejaVu Sans') ?? families.find((name) => !/JetBrains|Symbols/.test(name));
	expect(family).toBeDefined();
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect.poll(() => page.evaluate(() => window.terminal?.options.fontFamily)).toMatch(/^"JetBrains Mono"/);
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	const original = config();

	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	await page.getByRole('button', { name: 'Change font' }).click();
	const dialog = page.getByRole('dialog', { name: 'Choose a font' });
	const search = dialog.getByRole('combobox', { name: 'Search fonts' });
	await expect(search).toBeFocused();
	await expect(dialog.getByRole('option', { name: /^Default/ }).getByLabel('Current font')).toBeVisible();
	await search.fill(family!);
	await dialog.getByRole('option', { name: family, exact: true }).click();
	await expect.poll(bodyFont).toMatch(new RegExp(`^"${family}"`));
	await expect(settings.locator('.preview code')).toHaveCSS('font-family', /^"?JetBrains Mono/);
	expect(config()).toBe(original);
	await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
	await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect.poll(bodyFont).toMatch(/^"?JetBrains Mono/);

	await page.getByRole('button', { name: 'Change font' }).click();
	await search.fill(family!);
	await dialog.getByRole('option', { name: family, exact: true }).click();
	await dialog.getByRole('button', { name: 'Apply font', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(config()).toContain(`ui_font = "${family}"`);
	await expect(page.getByRole('button', { name: 'Change font' })).toHaveAttribute('title', `Font: ${family}`);
	await expect.poll(() => platformFont(page, '#host-name-help'))
		.toEqual([expect.objectContaining({ familyName: family, isCustomFont: true })]);

	await page.reload();
	await expect.poll(bodyFont).toMatch(new RegExp(`^"${family}"`));
	await expect.poll(() => page.evaluate(() => window.terminal?.options.fontFamily)).toMatch(/^"JetBrains Mono"/);
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	await page.getByRole('button', { name: 'Change font' }).click();
	await search.fill('default');
	await dialog.getByRole('option', { name: /^Default/ }).click();
	await expect.poll(bodyFont).toMatch(/^"?JetBrains Mono/);
	await dialog.getByRole('button', { name: 'Apply font', exact: true }).click();
	await expect(dialog).toBeHidden();
	expect(config()).toBe(original);
	await expect(page.getByRole('button', { name: 'Change font' })).toHaveAttribute('title', 'Font: Default');
});
