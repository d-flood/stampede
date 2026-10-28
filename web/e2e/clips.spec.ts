import { readFileSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { openProject, openSettings } from './shell.ts';

test('Clips made in Settings insert at the caret of a Chat Composer and send as typed', { tag: '@desktop' }, async ({ page }) => {
	const sent: string[] = [];
	page.on('websocket', (socket) => socket.on('framesent', ({ payload }) => {
		if (typeof payload !== 'string') return;
		const message = JSON.parse(payload);
		if (message.type === 'chat.send') sent.push(message.text);
	}));
	await page.goto('/');
	await openSettings(page, 'Clips');
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	const section = settings.getByRole('region', { name: 'Clips', exact: true });
	const clip = section.getByRole('combobox', { name: 'Clip', exact: true });
	const name = section.getByRole('textbox', { name: 'Clip name', exact: true });
	const text = section.getByRole('textbox', { name: 'Clip text', exact: true });
	const save = section.getByRole('button', { name: 'Save clip', exact: true });
	for (const [clipName, clipText] of [['verify', 'Run the tests.\nThen commit.'], ['review', 'Review the diff.']]) {
		await clip.selectOption('');
		await name.fill(clipName);
		await text.fill(clipText);
		await save.click();
		await expect(section.getByRole('status')).toHaveText('Clip saved.');
	}
	await name.fill('two words');
	await save.click();
	await expect(section.getByRole('alert')).toHaveText('Clip names cannot contain spaces');
	await clip.selectOption('review');
	await section.getByRole('button', { name: 'Move up', exact: true }).click();
	await expect(section.getByRole('status')).toHaveText('Clip order saved.');
	await expect(clip.locator('option:checked')).toHaveText('1. review');
	await expect.poll(() => settings.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	const config = readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/config.toml`, 'utf8');
	expect(config.indexOf('name = "review"')).toBeLessThan(config.indexOf('name = "verify"'));
	expect(config).toContain('text = """\nRun the tests.\nThen commit."""');
	await settings.getByRole('button', { name: 'Done', exact: true }).click();

	await openProject(page, 'beta');
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	await expect(page.getByRole('tab', { name: /^Fake Claude/ }).getByTitle('done')).toBeVisible();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	const menu = panel.getByRole('region', { name: 'Clips' });

	await composer.fill('Fix the bug;');
	await expect(menu).toBeHidden();
	await composer.fill('Fix the bug ;');
	await expect(menu.getByRole('button')).toHaveText([/^;review/, /^;verify/]);
	await composer.pressSequentially('VE');
	await expect(menu.getByRole('button')).toHaveText([/^;verify/]);
	await composer.press('Enter');
	const expected = 'Fix the bug Run the tests.\nThen commit.';
	await expect(composer).toHaveValue(expected);
	await expect.poll(() => composer.evaluate((node: HTMLTextAreaElement) => node.selectionStart)).toBe(expected.length);
	await expect(menu).toBeHidden();
	expect(sent).toEqual([]);

	await composer.pressSequentially(' ;none');
	await expect(menu).toBeHidden();
	await composer.fill(expected);
	await composer.pressSequentially(' ;');
	await expect(menu).toBeVisible();
	await composer.press('Escape');
	await expect(menu).toBeHidden();
	await composer.press('Backspace');
	await composer.press('Backspace');
	await panel.getByRole('button', { name: 'Send', exact: true }).click();
	await expect.poll(() => sent).toEqual([expected]);
});

test('a Terminal Composer saves its text as a Clip and inserts it again', async ({ page }) => {
	await openProject(page, 'alpha');
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await page.getByRole('button', { name: 'Composer', exact: true }).click();
	const composer = page.getByRole('textbox', { name: 'Composer', exact: true });
	const saveAs = page.getByRole('button', { name: 'Save as Clip', exact: true });
	await expect(saveAs).toHaveCount(0);
	await composer.fill('git status --short');
	await saveAs.click();
	const dialog = page.getByRole('dialog', { name: 'Save as Clip', exact: true });
	await expect(dialog.getByRole('textbox', { name: 'Clip text', exact: true })).toHaveValue('git status --short');
	await expect(dialog.getByRole('combobox', { name: 'Clip', exact: true })).toHaveCount(0);
	await dialog.getByRole('textbox', { name: 'Clip name', exact: true }).fill('st');
	await dialog.getByRole('button', { name: 'Save clip', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect.poll(() => readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/config.toml`, 'utf8')).toContain('[[clip]]\nname = "st"\ntext = "git status --short"');

	await saveAs.click();
	await dialog.getByRole('textbox', { name: 'Clip name', exact: true }).fill('st');
	await dialog.getByRole('button', { name: 'Save clip', exact: true }).click();
	await expect(dialog.getByRole('alert')).toHaveText('A Clip with this name already exists');
	await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(dialog).toBeHidden();

	await composer.fill(';s');
	const menu = page.getByRole('region', { name: 'Clips' });
	await menu.getByRole('button', { name: ';st' }).click();
	await expect(composer).toHaveValue('git status --short');
});
