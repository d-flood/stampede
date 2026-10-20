import { mkdirSync, readFileSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { closeProject, openProject } from './shell.ts';

test('a ! command runs at once, shows its output, and reaches Claude with the next message', async ({ page }, { project }) => {
	const name = `shell-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude/ });
	await expect(tab.getByTitle('done')).toBeVisible();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });

	await composer.fill("!printf 'hello <shell>'; exit 4");
	await expect(composer).toHaveClass(/code/);
	await expect(composer).toHaveAttribute('autocapitalize', 'off');
	await panel.getByRole('button', { name: 'Run', exact: true }).click();
	await expect(composer).toHaveValue('');
	const card = panel.locator('details.shell');
	await expect(card).toContainText('hello <shell>');
	await expect(card).toContainText('✗ failed');
	await expect(card).toContainText('next message');
	await expect.poll(() => panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);

	await composer.fill('What did it say?');
	await panel.getByRole('button', { name: 'Send', exact: true }).click();
	await expect(panel.locator('.user', { hasText: 'What did it say?' })).toHaveText('What did it say?');
	await expect(card).not.toContainText('next message');
	const received = () => readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/claude/fake-received.jsonl`, 'utf8').split('\n').filter(Boolean)
		.map((line) => JSON.parse(line)).filter((message) => message.type === 'user').map((message) => message.message.content);
	await expect.poll(received).toEqual([
		"<bash-input>printf 'hello &lt;shell&gt;'; exit 4</bash-input>\n<bash-stdout>hello &lt;shell&gt;</bash-stdout><bash-stderr></bash-stderr><bash-exit-code>4</bash-exit-code>\n\nWhat did it say?"
	]);
	await expect(panel.getByText(/^Hello and welcome!/)).toBeVisible();
	await expect(tab.getByTitle('done')).toBeVisible();

	await composer.press('ArrowUp');
	await expect(composer).toHaveValue('What did it say?');
	await composer.press('ArrowUp');
	await expect(composer).toHaveValue("!printf 'hello <shell>'; exit 4");

	await composer.fill('!sleep 30');
	if (project.name === 'desktop') await composer.press('Enter');
	else await panel.getByRole('button', { name: 'Run', exact: true }).click();
	const sleeping = panel.locator('details.shell', { hasText: 'sleep 30' });
	await expect(sleeping).toContainText('running…');
	if (project.name === 'desktop') await composer.press('Escape');
	else await panel.getByRole('button', { name: 'Interrupt', exact: true }).click();
	await expect(sleeping).toContainText('✗ failed');
	await expect(panel.getByRole('button', { name: 'Interrupt', exact: true })).toHaveCount(0);
	await closeProject(page, name);
});
