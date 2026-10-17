import { expect, test } from '@playwright/test';
import { readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

test('Editor markers and Agent file changes preserve explicit edits', async ({ page }, { project }) => {
	const file = `${project.name}.txt`;
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: file, exact: true }).click();

	const line = page.locator('.cm-line', { hasText: 'two' });
	await line.click();
	await page.keyboard.press('End');
	await page.keyboard.type(' edited');
	const tab = page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab', { name: file });
	await expect(tab.getByTitle('Unsaved changes')).toBeVisible();
	if (project.name === 'desktop') await page.keyboard.press('ControlOrMeta+s');
	else await page.getByRole('button', { name: 'Save' }).click();
	await expect(tab.getByTitle('Unsaved changes')).toHaveCount(0);

	const marker = page.locator('.cm-changeGutter .cm-change-modified');
	await expect(marker).toHaveCount(1);
	await expect
		.poll(async () => {
			const [mark, text] = await Promise.all([marker.boundingBox(), line.boundingBox()]);
			return mark && text && Math.abs(mark.y + mark.height / 2 - (text.y + text.height / 2));
		})
		.toBeLessThan(2);

	const files = await showSidebar(page);
	await files.getByRole('button', { name: `empty-${project.name}.ts`, exact: true }).click();
	await page.locator('.editor:not([hidden]) .cm-content').click();
	await page.keyboard.type('export const added = true;');
	await expect(page.locator('.editor:not([hidden]) .cm-changeGutter .cm-gutterElement[class*="cm-change-"]')).toHaveCount(1);

	const tree = await showSidebar(page);
	await tree.getByRole('button', { name: 'Show ignored files' }).click();
	await tree.getByRole('button', { name: 'ignored', exact: true }).click();
	await tree.getByTitle(`ignored/${project.name}.txt`, { exact: true }).click();
	const current = page.locator('.editor:not([hidden])');
	await expect(current.locator('.cm-content')).toContainText('initial ignored contents');
	const ignored = `${process.env.OIKONOMOS_E2E_ROOT}/projects/alpha/ignored/${project.name}.txt`;
	writeFileSync(ignored, 'changed by Agent\n');
	await expect(current.locator('.cm-content')).toContainText('changed by Agent');
	await current.locator('.cm-content').click();
	await page.keyboard.press('End');
	await page.keyboard.type(' with unsaved edits');
	writeFileSync(ignored, 'changed again by Agent\n');
	await expect(current.getByRole('button', { name: 'Keep mine' })).toBeVisible();
	await current.getByRole('button', { name: 'Keep mine' }).click();
	await expect(current.locator('.cm-content')).toContainText('with unsaved edits');
	unlinkSync(ignored);
	await expect(current.getByRole('button', { name: 'Keep mine' })).toBeVisible();
	await current.getByRole('button', { name: 'Keep mine' }).click();
	if (project.name === 'desktop') {
		await current.locator('.cm-content').click();
		await page.keyboard.press('ControlOrMeta+s');
	}
	else await current.getByRole('button', { name: 'Save', exact: true }).click();
	await expect.poll(() => {
		try { return readFileSync(ignored, 'utf8'); } catch { return ''; }
	}).toContain('with unsaved edits');
});
