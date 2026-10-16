import { expect, test } from '@playwright/test';

test('a saved edit to a tracked file shows a Change marker on that line', async ({ page }, { project }) => {
	const file = `${project.name}.txt`;
	await page.goto('/');
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await page.getByRole('option', { name: /\/alpha$/ }).click();
	const sidebar = page.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) await page.getByRole('button', { name: 'Sidebar' }).click();
	await sidebar.getByRole('button', { name: file }).click();

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
});
