import { expect, test } from '@playwright/test';
import { appendFileSync } from 'node:fs';

test('a Diff tab opened from Changes shows the change', async ({ page }, { project }) => {
	const line = `export const ${project.name} = true;`;
	appendFileSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/alpha/greeting.ts`, `${line}\n`);
	await page.goto('/');
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await page.getByRole('option', { name: /\/alpha$/ }).click();
	const toggle = page.getByRole('button', { name: 'Sidebar' });
	if ((await toggle.getAttribute('aria-expanded')) === 'false') await toggle.click();
	const sidebar = page.getByRole('complementary', { name: 'Sidebar' });
	await sidebar.getByRole('button', { name: 'Changes' }).click();
	await sidebar.getByRole('list', { name: 'Modified' }).getByRole('button', { name: 'greeting.ts' }).click();
	await expect(page.getByRole('tab', { name: /greeting\.ts/ })).toHaveAttribute('aria-selected', 'true');
	await expect(page.getByRole('tabpanel').locator('.cm-changedLine', { hasText: line })).toBeVisible();
});
