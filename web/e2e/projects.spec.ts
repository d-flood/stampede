import { expect, test } from './fixtures.ts';

test('the project picker focuses its input and opens a path chosen with arrow keys and Enter', { tag: '@desktop' }, async ({ page }) => {
	await page.goto('/');
	const open = page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' });
	await open.click();
	const dialog = page.getByRole('dialog');
	const input = dialog.getByRole('combobox');
	await expect(input).toBeFocused();
	await page.keyboard.type('beta');
	await expect(page.getByRole('option')).toHaveCount(1);
	await page.keyboard.press('Escape');
	await expect(dialog).toBeHidden();
	await open.click();
	await expect(input).toBeFocused();
	await expect(input).toHaveValue('');
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
