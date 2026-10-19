import { expect, test } from './fixtures.ts';

test.beforeEach(async ({ page }, { project }) => {
	await page.goto('/');
	if (project.name === 'pixel') {
		await page.getByRole('button', { name: 'Previous screen' }).click();
		await expect.poll(() => page.locator('.screens').evaluate((element) => element.scrollLeft)).toBe(0);
	}
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
});

test('opening the project picker focuses its input', async ({ page }) => {
	const input = page.getByRole('dialog').getByRole('combobox');
	await expect(input).toBeFocused();
	await page.keyboard.type('beta');
	await expect(page.getByRole('option')).toHaveCount(1);
	await page.keyboard.press('Escape');
	await expect(page.getByRole('dialog')).toBeHidden();
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await expect(input).toBeFocused();
	await expect(input).toHaveValue('');
});

test('project paths can be selected with arrow keys and opened with Enter', async ({ page }) => {
	const dialog = page.getByRole('dialog');
	const paths = dialog.getByRole('option');
	await expect(paths).toHaveCount(2);
	await page.keyboard.press('ArrowDown');
	await expect(paths.nth(1)).toHaveAttribute('aria-selected', 'true');
	await page.keyboard.press('ArrowUp');
	await expect(paths.nth(0)).toHaveAttribute('aria-selected', 'true');
	await page.keyboard.press('ArrowDown');
	await page.keyboard.press('Enter');
	await expect(dialog).toBeHidden();
	await expect(page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'beta', exact: true })).toHaveAttribute('aria-current', 'true');
});
