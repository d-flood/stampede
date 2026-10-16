import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { openProject } from './shell.ts';

const prompt = 'Run the shell command `mkdir -p out && touch out/approved.txt` exactly as written, then reply with the single word Done.';

async function launch(page: Page, rig: string, name: string) {
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: `${rig} as Chat` }).click();
	const tab = page.getByRole('tab', { name: new RegExp(`^${rig}`) });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill(prompt);
	await panel.getByRole('button', { name: 'Send' }).click();
	return { tab, panel, composer, card: panel.getByRole('region', { name: 'Approval' }) };
}

for (const agent of ['Claude', 'Codex']) {
	test(`tapping Allow on a ${agent} approval card resolves it`, async ({ page }, { project }) => {
		const { tab, panel, composer, card } = await launch(page, `${agent} Approval`, `requests-${agent}-${project.name}`);
		await expect(card.getByRole('heading', { name: `${agent} wants to run a command` })).toBeVisible();
		await expect(card.locator('pre')).toHaveText('mkdir -p out\ntouch out/approved.txt');
		await expect(card.getByRole('button', { name: 'Always allow' })).toBeDisabled();
		await expect(tab.getByTitle('waiting')).toBeVisible();
		await expect(composer).toHaveAttribute('placeholder', `Or tell ${agent} what to do instead…`);

		await card.getByRole('button', { name: 'Allow', exact: true }).click();
		await expect(card).toHaveCount(0);
		await expect(panel.getByText('Done', { exact: true })).toBeVisible();
		await expect(tab.getByTitle('done')).toBeVisible();
		await expect(composer).toHaveAttribute('placeholder', `Message ${agent} Approval`);
	});
}

test('sending from the Composer while a card is up denies the request instead', async ({ page }, { project }) => {
	const { tab, panel, composer, card } = await launch(page, 'Claude Approval', `requests-deny-${project.name}`);
	await expect(card).toBeVisible();
	await composer.fill('Make a directory called elsewhere instead');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(card).toHaveCount(0);
	await expect(composer).toHaveValue('');
	await expect(tab.getByTitle('done')).toBeVisible();
	await expect(panel.getByText('Make a directory called elsewhere instead')).toHaveCount(0);
});
