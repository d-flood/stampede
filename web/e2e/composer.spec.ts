import type { Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { closeProject, openProject } from './shell.ts';

test('chat composer submits with Enter on desktop and keeps mobile Enter as a newline', async ({ page, context }, { project }) => {
	const sent: { text: string; queue: boolean }[] = [];
	page.on('websocket', (socket) => socket.on('framesent', ({ payload }) => {
		if (typeof payload !== 'string') return;
		const message = JSON.parse(payload);
		if (message.type === 'chat.send') sent.push(message);
	}));
	const name = `composer-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	await expect(page.getByRole('tab', { name: /^Fake Claude/ }).getByTitle('done')).toBeVisible();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('/eff');
	await composer.press('Enter');
	await expect(composer).toHaveValue('/effort ');
	expect(sent).toHaveLength(0);
	await composer.fill('First line');
	await composer.dispatchEvent('keydown', { key: 'Enter', isComposing: true });
	await expect(composer).toHaveValue('First line');
	expect(sent).toHaveLength(0);
	await composer.press('Shift+Enter');
	await composer.press('s');
	await expect(composer).toHaveValue('First line\ns');
	expect(sent).toHaveLength(0);
	await composer.press('Enter');
	if (project.name === 'pixel') {
		await expect(composer).toHaveValue('First line\ns\n');
		expect(sent).toHaveLength(0);
		await panel.getByRole('button', { name: 'Send', exact: true }).click();
	}
	await expect(composer).toHaveValue('');
	await expect.poll(() => sent.map(({ text, queue }) => ({ text, queue }))).toEqual([
		{ text: `First line\ns${project.name === 'pixel' ? '\n' : ''}`, queue: false }
	]);
	if (project.name === 'desktop') {
		await composer.press('Enter');
		await expect(composer).toHaveValue('');
		await context.setOffline(true);
		await composer.fill('Offline draft');
		await expect(panel.getByRole('button', { name: /^(Send|Steer)$/ })).toBeDisabled();
		await composer.press('Enter');
		await expect(composer).toHaveValue('Offline draft');
		expect(sent).toHaveLength(1);
		await context.setOffline(false);
	}
	await closeProject(page, name);
});

async function launch(page: Page, name: string) {
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude/ });
	await expect(tab.getByTitle('done')).toBeVisible();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	return { tab, panel, composer: panel.getByRole('textbox', { name: 'Composer' }) };
}

test('opening a Chat focuses its composer without client errors', async ({ page }, { project }) => {
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	const { composer } = await launch(page, `composer-focus-${project.name}`);
	await expect(composer).toBeFocused();
	await composer.fill('Still responsive');
	await expect(composer).toHaveValue('Still responsive');
	expect(errors).toEqual([]);
});

test('ArrowUp and ArrowDown recall earlier prompts without replacing a draft', async ({ page }, { project }) => {
	const name = `composer-history-${project.name}`;
	const { tab, panel, composer } = await launch(page, name);
	const send = panel.getByRole('button', { name: /^(Send|Steer)$/ });
	await composer.fill('First prompt');
	await send.click();
	await expect(panel.getByText(/^Hello and welcome!/)).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();
	await composer.fill('Second prompt\nwith two lines');
	await send.click();
	await expect(panel.locator('.user', { hasText: 'Second prompt' })).toBeVisible();
	await expect(composer).toHaveValue('');

	await composer.press('ArrowUp');
	await expect(composer).toHaveValue('Second prompt\nwith two lines');
	await composer.press('ArrowDown');
	await expect(composer).toHaveValue('Second prompt\nwith two lines');
	await composer.press('ArrowDown');
	await expect(composer).toHaveValue('');
	await composer.press('ArrowUp');
	await composer.press('ArrowUp');
	await expect(composer).toHaveValue('First prompt');
	await composer.press('ArrowUp');
	await expect(composer).toHaveValue('First prompt');
	await composer.press('ArrowDown');
	await expect(composer).toHaveValue('Second prompt\nwith two lines');
	await composer.press('ArrowUp');
	await expect(composer).toHaveValue('Second prompt\nwith two lines');
	await composer.press('ArrowUp');
	await expect(composer).toHaveValue('First prompt');

	await composer.fill('A draft');
	await composer.press('ArrowUp');
	await expect(composer).toHaveValue('A draft');
	await closeProject(page, name);
});

test('a sent prompt renders its markdown', async ({ page }, { project }) => {
	const name = `composer-markdown-${project.name}`;
	const { panel, composer } = await launch(page, name);
	await composer.fill('**Bold** and `code` <b>raw</b>\n\n- item');
	await panel.getByRole('button', { name: /^(Send|Steer)$/ }).click();
	const sent = panel.locator('.user');
	await expect(sent.locator('strong')).toHaveText('Bold');
	await expect(sent.locator('code')).toHaveText('code');
	await expect(sent.getByRole('listitem')).toHaveText('item');
	await expect(sent.locator('b')).toHaveCount(0);
	await expect(sent).toContainText('<b>raw</b>');
	await closeProject(page, name);
});

test('desktop Chats dock the settings and actions under the Composer', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `composer-dock-${project.name}`;
	const { panel, composer } = await launch(page, name);
	const mode = panel.getByRole('button', { name: 'Mode: default', exact: true });
	const attach = panel.getByRole('button', { name: 'Attach image', exact: true });
	const more = panel.getByRole('button', { name: 'More', exact: true });
	const send = panel.getByRole('button', { name: 'Send', exact: true });
	const clip = page.getByRole('menuitem', { name: 'Save as Clip' });
	for (const width of [1900, 1280, 1000]) {
		await page.setViewportSize({ width, height: 800 });
		await expect.poll(async () => {
			const [text, setting, action] = await Promise.all([composer, mode, attach].map((locator) => locator.boundingBox()));
			return !!text && !!setting && !!action && setting.y >= text.y + text.height && setting.x < action.x
				&& Math.abs(setting.y + setting.height / 2 - (action.y + action.height / 2)) < 1;
		}).toBe(true);
	}
	await expect(panel.getByRole('img', { name: 'Claude Code', exact: true })).toBeVisible();
	const tooltip = (text: string) => page.getByRole('tooltip').filter({ hasText: text });
	for (const [button, text] of [[attach, 'Attach image'], [more, 'More'], [send, 'Send']] as const) {
		await expect(button).toHaveAttribute('data-delay-duration', '0');
		await button.hover();
		await expect(tooltip(text)).toBeVisible();
	}
	await more.click();
	await expect(clip).toBeDisabled();
	await page.keyboard.press('Escape');
	await composer.fill('Clip this');
	await more.click();
	await clip.click();
	const dialog = page.getByRole('dialog', { name: 'Save as Clip', exact: true });
	await expect(dialog.getByRole('textbox', { name: 'Clip text', exact: true })).toHaveValue('Clip this');
	await page.keyboard.press('Escape');
	await expect(dialog).toBeHidden();
	await mode.click();
	await page.getByRole('menuitemcheckbox', { name: 'plan' }).click();
	await expect(panel.getByRole('button', { name: 'Mode: plan', exact: true })).toBeVisible();
	await closeProject(page, name);
});

test('Shift+Tab in the composer cycles the Chat mode', async ({ page }, { project }) => {
	const name = `composer-mode-${project.name}`;
	const { panel, composer } = await launch(page, name);
	await panel.getByRole('button', { name: 'Mode: default', exact: true }).click();
	await expect(page.getByRole('menuitemcheckbox', { name: 'accept edits' })).toBeVisible();
	await page.keyboard.press('Escape');
	await composer.press('Shift+Tab');
	await expect(panel.getByRole('button', { name: 'Mode: accept edits', exact: true })).toBeVisible();
	await expect(composer).toBeFocused();
	await composer.press('Shift+Tab');
	await expect(panel.getByRole('button', { name: 'Mode: plan', exact: true })).toBeVisible();
	await closeProject(page, name);
});
