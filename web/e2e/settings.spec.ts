import { mkdirSync, readFileSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { closeProject, openProject, screen, showSidebar } from './shell.ts';

test('JetBrains Mono loads for UI text and controls', { tag: '@desktop' }, async ({ page }) => {
	await page.goto('/');
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	for (const element of [
		page.locator('body'), settings, settings.locator('h2'),
		settings.locator('#host-name'), settings.locator('#text-size'),
		settings.getByRole('button', { name: 'Done', exact: true }),
		settings.locator('.preview span'), settings.locator('.preview code')
	]) {
		await expect(element).toHaveCSS('font-family', /^"?JetBrains Mono/);
	}
	const faces = await page.evaluate(async () => Promise.all(
		['400', '700', 'italic 400', 'italic 700'].map(async (style) => {
			const loaded = await document.fonts.load(`${style} 16px "JetBrains Mono"`);
			return loaded.length > 0 && loaded.every((face) => face.status === 'loaded');
		})
	));
	expect(faces).toEqual([true, true, true, true]);
	const session = await page.context().newCDPSession(page);
	try {
		await session.send('DOM.enable');
		await session.send('CSS.enable');
		const { root } = await session.send('DOM.getDocument');
		const { nodeId } = await session.send('DOM.querySelector', { nodeId: root.nodeId, selector: '#host-name-help' });
		await expect.poll(async () => (await session.send('CSS.getPlatformFontsForNode', { nodeId })).fonts)
			.toEqual([expect.objectContaining({ familyName: 'JetBrains Mono', isCustomFont: true })]);
	} finally {
		await session.detach();
	}
});

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
	await expect.poll(() => settings.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
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

test('text size updates a running terminal and the editor', { tag: '@desktop' }, async ({ page }) => {
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

test('available modes are chosen per Agent and limit a Chat\'s mode menu and Shift+Tab', { tag: '@desktop' }, async ({ page }) => {
	const name = 'modes';
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'Settings', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'Settings', exact: true });
	const claude = dialog.getByRole('group', { name: 'Claude Code', exact: true });
	await claude.getByRole('checkbox', { name: 'accept edits', exact: true }).uncheck();
	await expect.poll(() => readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/config.toml`, 'utf8')).toContain('[modes]\nclaude-code = ["default", "plan", "auto", "dontAsk", "bypassPermissions"]');
	const copilot = dialog.getByRole('group', { name: 'GitHub Copilot', exact: true });
	await copilot.getByRole('checkbox', { name: 'full access', exact: true }).uncheck();
	await expect(copilot.getByRole('checkbox', { name: 'default', exact: true })).toBeDisabled();
	await expect.poll(() => dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await dialog.getByRole('button', { name: 'Done', exact: true }).click();

	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	await panel.getByRole('button', { name: 'Mode: default', exact: true }).click();
	await expect(page.getByRole('menuitemcheckbox', { name: 'plan' })).toBeVisible();
	await expect(page.getByRole('menuitemcheckbox', { name: 'accept edits' })).toHaveCount(0);
	await page.keyboard.press('Escape');
	await panel.getByRole('textbox', { name: 'Composer' }).press('Shift+Tab');
	await expect(panel.getByRole('button', { name: 'Mode: plan', exact: true })).toBeVisible();
	await closeProject(page, name);
});
