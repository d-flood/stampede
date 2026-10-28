import { expect, test } from './fixtures.ts';
import { readFileSync } from 'node:fs';
import type { Page } from '@playwright/test';
import { openProject, openSettings, screen } from './shell.ts';

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

test('UI, chat and code fonts each come from the Host and apply only to their own text', { tag: '@desktop' }, async ({ page }) => {
	const config = () => readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/config.toml`, 'utf8');
	await openProject(page, 'alpha');
	const families: string[] = await (await page.request.get('/api/fonts')).json();
	const family = families.find((name) => name === 'DejaVu Sans') ?? families.find((name) => !/JetBrains|Symbols/.test(name));
	expect(family).toBeDefined();
	const chosen = new RegExp(`^"${family}"`);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect.poll(() => page.evaluate(() => window.terminal?.options.fontFamily)).toMatch(/^"JetBrains Mono"/);
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	const original = config();

	const settings = await openSettings(page, 'Appearance');
	const roles = [
		{ name: 'UI font', key: 'ui_font', sample: '.preview span:first-child', bundled: /^"?Fira Sans/ },
		{ name: 'Chat font', key: 'chat_font', sample: '.preview .chat', bundled: /^"?Literata/ },
		{ name: 'Code font', key: 'code_font', sample: '.preview code', bundled: /^"?JetBrains Mono/ }
	];
	for (const [index, { name, key, sample, bundled }] of roles.entries()) {
		const change = settings.getByRole('button', { name: `Change ${name}`, exact: true });
		const picker = page.getByRole('dialog', { name, exact: true });
		const search = picker.getByRole('combobox', { name: 'Search fonts' });
		const saved = config();
		await change.click();
		await expect(search).toBeFocused();
		await expect(picker.getByRole('option', { name: /^Default/ }).getByLabel('Current font')).toBeVisible();
		await search.fill(family!);
		await picker.getByRole('option', { name: family, exact: true }).click();
		await expect(settings.locator(sample)).toHaveCSS('font-family', chosen);
		for (const later of roles.slice(index + 1)) await expect(settings.locator(later.sample)).toHaveCSS('font-family', later.bundled);
		expect(config()).toBe(saved);
		await picker.getByRole('button', { name: 'Cancel', exact: true }).click();
		await expect(picker).toBeHidden();
		await expect(settings.locator(sample)).toHaveCSS('font-family', bundled);

		await change.click();
		await search.fill(family!);
		await picker.getByRole('option', { name: family, exact: true }).click();
		await picker.getByRole('button', { name: 'Apply font', exact: true }).click();
		await expect(picker).toBeHidden();
		expect(config()).toContain(`${key} = "${family}"`);
		await expect(change).toHaveAttribute('title', `${name}: ${family}`);
		await expect.poll(() => platformFont(page, sample)).toEqual([expect.objectContaining({ familyName: family, isCustomFont: true })]);
	}
	await expect.poll(() => page.evaluate(() => window.terminal?.options.fontFamily)).toMatch(chosen);
	await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);

	await page.reload();
	await expect(page.locator('body')).toHaveCSS('font-family', chosen);
	await expect.poll(() => page.evaluate(() => window.terminal?.options.fontFamily)).toMatch(chosen);
	await openSettings(page, 'Appearance');
	for (const { name, sample, bundled } of roles) {
		const picker = page.getByRole('dialog', { name, exact: true });
		await expect(settings.locator(sample)).toHaveCSS('font-family', chosen);
		await settings.getByRole('button', { name: `Change ${name}`, exact: true }).click();
		await picker.getByRole('combobox', { name: 'Search fonts' }).fill('default');
		await picker.getByRole('option', { name: /^Default/ }).click();
		await picker.getByRole('button', { name: 'Apply font', exact: true }).click();
		await expect(picker).toBeHidden();
		await expect(settings.locator(sample)).toHaveCSS('font-family', bundled);
		await expect(settings.getByRole('button', { name: `Change ${name}`, exact: true })).toHaveAttribute('title', `${name}: Default`);
	}
	expect(config()).toBe(original);
	await expect.poll(() => page.evaluate(() => window.terminal?.options.fontFamily)).toMatch(/^"JetBrains Mono"/);
});
