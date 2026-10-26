import { expect, test } from './fixtures.ts';
import { execFileSync } from 'node:child_process';
import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

test('an open staged rename Diff follows HEAD after the rename is committed', { tag: '@desktop' }, async ({ page }) => {
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/rename`;
	mkdirSync(root);
	const git = (...args: string[]) => execFileSync('git', ['-C', root, '-c', 'user.name=stampede', '-c', 'user.email=e2e@stampede.invalid', '-c', 'commit.gpgsign=false', ...args]);
	git('init', '-q');
	writeFileSync(`${root}/before.ts`, 'export const one = 1;\nexport const two = 2;\nexport const three = 3;\n');
	git('add', '.');
	git('commit', '-qm', 'Initial file');
	git('mv', 'before.ts', 'after.ts');
	appendFileSync(`${root}/after.ts`, 'export const four = 4;\n');
	git('add', '.');
	await openProject(page, 'rename');
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
