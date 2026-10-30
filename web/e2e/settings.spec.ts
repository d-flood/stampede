import { mkdirSync, readFileSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { closeProject, openProject, openSettings, screen, showSidebar } from './shell.ts';

test('Fira Sans loads for the UI, Literata for chat text and JetBrains Mono for code', { tag: '@desktop' }, async ({ page }) => {
	await page.goto('/');
	const settings = await openSettings(page, 'Appearance');
	for (const element of [
		page.locator('body'), settings, settings.locator('h2'),
		settings.getByRole('button', { name: 'Done', exact: true }), settings.locator('.preview span:first-child')
	]) {
		await expect(element).toHaveCSS('font-family', /^"?Fira Sans/);
	}
	await expect(settings.locator('.preview .chat')).toHaveCSS('font-family', /^"?Literata/);
	await expect(settings.locator('.preview code')).toHaveCSS('font-family', /^"?JetBrains Mono/);
	const faces = await page.evaluate(async () => Promise.all(
		['400 16px "Fira Sans"', '500 16px "Fira Sans"', '600 16px "Fira Sans"', '700 16px "Fira Sans"', 'italic 400 16px "Fira Sans"',
			'400 16px "Literata Variable"', 'italic 400 16px "Literata Variable"',
			'400 16px "JetBrains Mono"', '700 16px "JetBrains Mono"', 'italic 400 16px "JetBrains Mono"', 'italic 700 16px "JetBrains Mono"'
		].map(async (font) => {
			const loaded = await document.fonts.load(font);
			return loaded.length > 0 && loaded.every((face) => face.status === 'loaded');
		})
	));
	expect(faces.every(Boolean)).toBe(true);
	const session = await page.context().newCDPSession(page);
	try {
		await session.send('DOM.enable');
		await session.send('CSS.enable');
		const { root } = await session.send('DOM.getDocument');
		for (const [selector, familyName] of [['.preview span:first-child', 'Fira Sans'], ['.preview .chat', 'Literata'], ['.preview code', 'JetBrains Mono']]) {
			const { nodeId } = await session.send('DOM.querySelector', { nodeId: root.nodeId, selector });
			await expect.poll(async () => (await session.send('CSS.getPlatformFontsForNode', { nodeId })).fonts)
				.toEqual([expect.objectContaining({ familyName: expect.stringContaining(familyName), isCustomFont: true })]);
		}
	} finally {
		await session.detach();
	}
});

test('UI, chat and code sizes default per device, change, persist, and reset', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const defaults = { ui: 16, chat: phone ? 18 : 16, code: 16 };
	await page.goto('/');
	await expect(page.locator('body')).toHaveCSS('font-size', `${defaults.ui}px`);
	const settings = await openSettings(page, 'Appearance');
	const samples = { ui: page.locator('body'), chat: settings.locator('.preview .chat'), code: settings.locator('.preview code') };
	const sizes = {
		ui: settings.getByRole('combobox', { name: 'UI font size', exact: true }),
		chat: settings.getByRole('combobox', { name: 'Chat font size', exact: true }),
		code: settings.getByRole('combobox', { name: 'Code font size', exact: true })
	};
	for (const role of ['ui', 'chat', 'code'] as const) {
		await expect(sizes[role]).toHaveValue('');
		await expect(sizes[role].locator('option:checked')).toHaveText(`Default · ${defaults[role]} px`);
		await expect(samples[role]).toHaveCSS('font-size', `${defaults[role]}px`);
	}
	if (phone) {
		await expect.poll(() => settings.evaluate((element) => {
			const box = element.getBoundingClientRect();
			return box.top === 0 && box.left === 0 && box.width === innerWidth && box.height === innerHeight;
		})).toBe(true);
	}
	await sizes.ui.selectOption('22');
	await sizes.chat.selectOption('24');
	await sizes.code.selectOption('18');
	await expect(samples.ui).toHaveCSS('font-size', '22px');
	await expect(samples.chat).toHaveCSS('font-size', '24px');
	await expect(samples.code).toHaveCSS('font-size', '18px');
	await expect.poll(() => settings.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await settings.getByRole('button', { name: 'Done', exact: true }).click();
	await expect(settings).toBeHidden();
	await page.reload();
	await expect(page.locator('body')).toHaveCSS('font-size', '22px');
	await openSettings(page, 'Appearance');
	await expect(sizes.ui).toHaveValue('22');
	await expect(sizes.chat).toHaveValue('24');
	await expect(sizes.code).toHaveValue('18');
	for (const role of ['ui', 'chat', 'code'] as const) {
		await sizes[role].selectOption('');
		await expect(samples[role]).toHaveCSS('font-size', `${defaults[role]}px`);
	}
	await page.reload();
	await expect(page.locator('body')).toHaveCSS('font-size', `${defaults.ui}px`);
});

test('code size updates a running terminal and the editor', { tag: '@desktop' }, async ({ page }) => {
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
	const settings = await openSettings(page, 'Appearance');
	await settings.getByRole('combobox', { name: 'Code font size', exact: true }).selectOption('20');
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

test('checking origin in the background is on until turned off in the config', async ({ page }) => {
	await page.goto('/');
	const settings = await openSettings(page, 'General');
	const checking = settings.getByRole('radiogroup', { name: 'Check origin' });
	await expect(checking.getByRole('radio', { name: 'On', exact: true })).toBeChecked();
	await checking.getByRole('radio', { name: 'Off', exact: true }).click();
	await expect(checking.getByRole('radio', { name: 'Off', exact: true })).toBeChecked();
	await expect.poll(() => readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/config.toml`, 'utf8')).toMatch(/^auto_fetch = false$/m);
	await checking.getByRole('radio', { name: 'On', exact: true }).click();
	await expect.poll(() => readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/config.toml`, 'utf8')).not.toContain('auto_fetch');
});

test('available modes are chosen per Agent and limit a Chat\'s mode menu and Shift+Tab', { tag: '@desktop' }, async ({ page }) => {
	const name = 'modes';
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	const dialog = await openSettings(page, 'Agents');
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
