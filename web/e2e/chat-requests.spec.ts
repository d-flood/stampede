import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { closeProject, openProject } from './shell.ts';

const prompt = 'Run the shell command `mkdir -p out && touch out/approved.txt` exactly as written, then reply with the single word Done.';

async function launch(page: Page, rig: string, name: string) {
	mkdirSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: rig, exact: true }).click();
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
		await expect(card.getByRole('button', { name: 'Always allow' })).toBeEnabled();
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
	await composer.fill('/');
	const menu = panel.getByRole('region', { name: 'Commands' });
	await expect(menu.getByRole('button', { name: /^\/clear/ })).toBeDisabled();
	await expect(menu.getByRole('button', { name: /^\/compact/ })).toBeDisabled();
	await composer.fill('/effort low');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(card).toHaveCount(0);
	await expect(composer).toHaveValue('');
	await expect(tab.getByTitle('done')).toBeVisible();
	await expect(panel.getByText('/effort low', { exact: true })).toHaveCount(0);
});

for (const agent of ['Claude', 'Codex']) {
	test(`${agent} approval rules show shortened prefixes and scope`, async ({ page }, { project }) => {
		const { card, panel } = await launch(page, `${agent} Approval`, `rules-${agent}-${project.name}`);
		await card.getByRole('button', { name: 'Always allow' }).click();
		await expect(card.getByLabel('Rule scope')).toHaveValue('chat');
		await card.getByLabel('Rule scope').selectOption('always');
		await card.getByRole('button', { name: 'mkdir', exact: true }).click();
		await card.getByRole('button', { name: 'touch', exact: true }).click();
		await expect(card.locator('output')).toHaveText(['mkdir *', 'touch *']);
		await expect.poll(() => card.evaluate((node) => node.scrollWidth <= node.clientWidth && node.getBoundingClientRect().bottom <= window.innerHeight)).toBe(true);
		await card.getByRole('button', { name: 'Allow rules', exact: true }).click();
		await expect(card).toHaveCount(0);
		await expect(panel.getByText('Done', { exact: true })).toBeVisible();
		await closeProject(page, `rules-${agent}-${project.name}`);
	});

	test(`${agent} questions step through choices and dock a plan`, async ({ page }, { project }) => {
		const { panel } = await launch(page, `${agent} Questions`, `questions-${agent}-${project.name}`);
		const question = panel.getByRole('region', { name: 'Question', exact: true });
		await expect(question).toContainText('Question 1 of 2');
		await expect(question.getByText(agent === 'Claude' ? 'Choose a color' : 'Which color should the text file contain?', { exact: true })).toBeVisible();
		await expect(question.getByText(agent === 'Claude' ? 'Warm' : 'Choose Red for the file content.', { exact: true })).toBeVisible();
		if (agent === 'Claude') {
			const preview = question.locator('.preview');
			await expect(preview).toBeVisible();
			await expect(preview).toHaveText('Color   Temperature   Use\nRed     warm          Highlight commands that need attention\nBlue    cool          Show commands that are still running');
			await expect(preview).toHaveCSS('white-space', 'pre');
		}
		if (agent === 'Codex') await expect(panel.getByText('I’ll collect both choices first, then write the short plan without creating the file.', { exact: true })).toBeVisible();
		await expect.poll(() => question.evaluate((node) => node.scrollWidth <= node.clientWidth && node.getBoundingClientRect().bottom <= window.innerHeight)).toBe(true);
		await question.getByRole('button', { name: /^Red/ }).click();
		await question.getByRole('button', { name: 'Next', exact: true }).click();
		await expect(question).toContainText('Question 2 of 2');
		await question.getByRole('textbox', { name: 'Other answer' }).fill('Triangle');
		await question.getByRole('button', { name: 'Submit', exact: true }).click();
		const plan = panel.getByRole('region', { name: 'Plan', exact: true });
		await expect(plan).toBeVisible();
		await plan.getByRole('button', { name: 'Approve', exact: true }).click();
		await expect(plan).toHaveCount(0);
		const history = panel.getByRole('group', { name: 'Answered questions', exact: true });
		await expect(history).not.toHaveAttribute('open');
		await history.locator('summary').click();
		await expect(history.getByText('Answer: Red', { exact: true })).toBeVisible();
		await expect(history.getByText('Answer: Triangle', { exact: true })).toBeVisible();
		await expect(history.getByText(agent === 'Claude' ? 'Warm' : 'Choose Red for the file content.', { exact: true })).toBeVisible();
		if (agent === 'Claude') await expect(history.locator('.preview')).toBeVisible();
		await expect.poll(() => history.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
		await page.reload();
		await expect(history).not.toHaveAttribute('open');
		await history.locator('summary').click();
		await expect(history.getByText('Answer: Triangle', { exact: true })).toBeVisible();
		await closeProject(page, `questions-${agent}-${project.name}`);
	});
}
