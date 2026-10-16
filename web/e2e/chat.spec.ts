import { expect, test } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { openProject } from './shell.ts';

test('a Claude Code Chat streams its reply, shows done, and asks before closing while working', async ({ page }, { project }) => {
	const name = `chat-${project.name}`;
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude as Chat' }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude/ });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	await expect(tab.getByTitle('done')).toBeVisible();

	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('Greet the reader');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.getByText('Greet the reader')).toBeVisible();
	await expect(panel.locator('strong', { hasText: 'welcome' })).toBeVisible();
	await expect(panel.getByText(/^Hello and welcome!/)).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();

	await composer.fill('Anything else?');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(tab.getByTitle('working')).toBeVisible();
	await tab.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
	const confirm = page.getByRole('dialog', { name: 'Close Fake Claude?' });
	await expect(confirm).toBeVisible();
	await confirm.getByRole('button', { name: 'Close' }).click();
	await expect(tab).toHaveCount(0);
});
