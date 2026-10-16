import { expect, test } from '@playwright/test';
import { appendFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test('a Diff tab opened from Changes shows the change', async ({ page }, { project }) => {
	const line = `export const ${project.name} = true;`;
	appendFileSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/alpha/greeting.ts`, `${line}\n`);
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Changes' }).click();
	await sidebar.getByRole('list', { name: 'Modified' }).getByRole('button', { name: 'greeting.ts' }).click();
	await expect(page.getByRole('tab', { name: /greeting\.ts/ })).toHaveAttribute('aria-selected', 'true');
	await expect(page.getByRole('tabpanel').locator('.cm-changedLine', { hasText: line })).toBeVisible();
});
