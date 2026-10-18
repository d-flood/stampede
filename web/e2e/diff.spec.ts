import { expect, test } from './fixtures.ts';
import { execFileSync } from 'node:child_process';
import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

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

test('an open rename Diff follows HEAD after the rename is committed', async ({ page }, { project }) => {
	const name = `rename-${project.name}`;
	const root = `${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`;
	mkdirSync(root);
	const git = (...args: string[]) => execFileSync('git', ['-C', root, '-c', 'user.name=oikonomos', '-c', 'user.email=e2e@oikonomos.invalid', '-c', 'commit.gpgsign=false', ...args]);
	git('init', '-q');
	writeFileSync(`${root}/before.ts`, 'export const one = 1;\nexport const two = 2;\nexport const three = 3;\n');
	git('add', '.');
	git('commit', '-qm', 'Initial file');
	git('mv', 'before.ts', 'after.ts');
	appendFileSync(`${root}/after.ts`, 'export const four = 4;\n');
	await openProject(page, name);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Changes' }).click();
	await sidebar.getByRole('list', { name: 'Renamed' }).getByRole('button', { name: 'after.ts' }).click();
	const diff = page.locator('.diff:not([hidden])');
	await expect(diff.locator('.cm-changedLine', { hasText: 'export const four' })).toBeVisible();
	git('add', '.');
	git('commit', '-qm', 'Rename file');
	await expect(diff.locator('.cm-changedLine')).toHaveCount(0);
	await expect(diff.locator('.path')).toHaveText('after.ts');
});
