import { expect, test } from './fixtures.ts';
import { execFileSync } from 'node:child_process';
import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

test('a Diff tab opened from Changes shows the change', async ({ page }, { project }) => {
	const line = `export const ${project.name} = true;`;
	appendFileSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/alpha/greeting.ts`, `${line}\n`);
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Changes' }).click();
	await sidebar.getByRole('list', { name: 'Changes', exact: true }).getByRole('button', { name: 'greeting.ts', exact: true }).click();
	await expect(page.getByRole('tab', { name: /greeting\.ts/ })).toHaveAttribute('aria-selected', 'true');
	await expect(page.getByRole('tabpanel').locator('.cm-changedLine', { hasText: line })).toBeVisible();
});

test('an open staged rename Diff follows HEAD after the rename is committed', async ({ page }, { project }) => {
	const name = `rename-${project.name}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(root);
	const git = (...args: string[]) => execFileSync('git', ['-C', root, '-c', 'user.name=stampede', '-c', 'user.email=e2e@stampede.invalid', '-c', 'commit.gpgsign=false', ...args]);
	git('init', '-q');
	writeFileSync(`${root}/before.ts`, 'export const one = 1;\nexport const two = 2;\nexport const three = 3;\n');
	git('add', '.');
	git('commit', '-qm', 'Initial file');
	git('mv', 'before.ts', 'after.ts');
	appendFileSync(`${root}/after.ts`, 'export const four = 4;\n');
	git('add', '.');
	await openProject(page, name);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Changes' }).click();
	await sidebar.getByRole('list', { name: 'Staged Changes' }).getByRole('button', { name: 'after.ts', exact: true }).click();
	const diff = page.locator('.diff:not([hidden])');
	await expect(diff.locator('.path')).toHaveText('before.ts → after.ts');
	await expect(diff.locator('.cm-changedLine', { hasText: 'export const four' })).toBeVisible();
	git('commit', '-qm', 'Rename file');
	await expect(diff.locator('.cm-changedLine')).toHaveCount(0);
	await expect(diff.locator('.path')).toHaveText('after.ts');
});
