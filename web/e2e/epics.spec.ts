import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { openProject, showSidebar } from './shell.ts';

const oikonomos = fileURLToPath(new URL('../../target/release/oikonomos', import.meta.url));

test('an Epic made through the CLI shows live in the Epics section, its Tab and its Tickets', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const slug = `epics-${project.name}`;
	const root = `${process.env.OIKONOMOS_E2E_ROOT}/projects/${slug}`;
	mkdirSync(root, { recursive: true });
	execFileSync('git', ['-C', root, 'init', '-q']);
	const cli = (stdin: string, ...args: string[]) =>
		execFileSync(oikonomos, [...args, '--data-dir', `${process.env.OIKONOMOS_E2E_ROOT}/data`], { cwd: root, input: stdin });

	await openProject(page, slug);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Epics', exact: true }).click();
	const section = sidebar.getByRole('region', { name: 'Epics' });
	await expect(section).toContainText('No Epics yet');

	cli('# Everywhere\n\nShow Epics on every Device.\n', 'epic', 'add', slug, '--title', 'Epics everywhere', '--brief', '-');
	cli('Build the section.\n', 'ticket', 'add', slug, '1', '--title', 'The section', '--body', '-');
	const card = section.getByRole('button', { name: new RegExp(`^${slug}`) });
	await expect(card).toContainText('Epics everywhere');
	await expect(card).toContainText('0 of 1 resolved');
	cli('Open it as a **Tab**.\n', 'ticket', 'add', slug, '2', '--title', 'The tab', '--blocked-by', '1', '--body', '-');
	await expect(card).toContainText('0 of 2 resolved');
	await card.click();

	const tab = page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab', { name: `Epic ${slug}` });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const panel = page.getByRole('tabpanel', { name: `Epic ${slug}` });
	const tickets = panel.getByRole('region', { name: 'Tickets' });
	await expect(tickets.getByRole('button')).toHaveText([/^1\s*The section/, /^2\s*The tab/]);
	await expect(tickets.getByRole('button', { name: /^2\s*The tab/ })).toContainText('not started');
	await expect(panel).toContainText('2 not started');
	cli('Show the detail.\n', 'ticket', 'add', slug, '3', '--title', 'The detail', '--blocked-by', '1,2', '--body', '-');
	await expect(tickets.getByRole('button', { name: /^3\s*The detail/ })).toBeVisible();
	cli('', 'note', `${slug}/2`, 'Kept the Tab read-only.');
	cli('', 'epic', 'set-setup', slug, 'pnpm install');

	if (phone) {
		await panel.getByRole('button', { name: /^Setup/ }).click();
		await expect(panel.getByRole('heading', { name: 'Setup Command' })).toBeVisible();
		await expect(panel.getByText('pnpm install')).toBeVisible();
		await panel.getByRole('button', { name: `Back to ${slug}` }).click();
		await panel.getByRole('button', { name: /^Brief/ }).click();
		await expect(panel.getByText('Show Epics on every Device.')).toBeVisible();
		await panel.getByRole('button', { name: `Back to ${slug}` }).click();
	} else {
		await expect(panel.getByRole('region', { name: 'Brief' })).toContainText('Show Epics on every Device.');
		await expect(panel.getByRole('region', { name: 'Setup Command' })).toContainText('pnpm install');
	}

	const row = tickets.getByRole('button', { name: /^2\s*The tab/ });
	await row.click();
	if (phone) {
		await expect(panel.getByRole('heading', { name: `${slug}/2` })).toBeVisible();
		await expect(panel.getByRole('heading', { name: 'The tab' })).toBeVisible();
		await expect(panel.getByText('after 1', { exact: true })).toBeVisible();
	} else {
		await expect(row).toHaveAttribute('aria-expanded', 'true');
	}
	await expect(panel.getByRole('region', { name: 'Body' }).locator('strong', { hasText: 'Tab' })).toBeVisible();
	const notes = panel.getByRole('region', { name: 'Notes' });
	await expect(notes).toContainText('Kept the Tab read-only.');
	await expect(notes).toContainText('Agent');
	cli('', 'note', `${slug}/2`, 'A second Note arrives live.');
	await expect(notes).toContainText('A second Note arrives live.');
	await page.screenshot({ path: test.info().outputPath('ticket.png') });

	if (phone) await panel.getByRole('button', { name: `Back to ${slug}` }).click();
	else await row.click();
	await expect(notes).toHaveCount(0);
	await expect(tickets.getByRole('button')).toHaveCount(3);
	await page.screenshot({ path: test.info().outputPath('epic.png') });
});
