import { expect, test } from './fixtures.ts';
import { execFileSync } from 'node:child_process';
import { openProject, showSidebar } from './shell.ts';

test('a Project at a repository root mints a worktree from a chosen ref beside it until deleted', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const alpha = `${process.env.STAMPEDE_E2E_ROOT}/projects/alpha`;
	const git = (...args: string[]) => execFileSync('git', ['-C', alpha, ...args], { encoding: 'utf8' }).trim();
	git('update-ref', 'refs/remotes/origin/main', 'HEAD');
	await openProject(page, 'beta');
	await openProject(page, 'alpha');
	const projects = async () => {
		if (!phone) return;
		const back = page.getByRole('button', { name: 'Previous screen' });
		while (!(await back.isDisabled())) await back.click();
		await expect.poll(() => page.locator('.screens').evaluate((element) => element.scrollLeft)).toBe(0);
	};
	await projects();

	const rail = page.getByRole('navigation', { name: 'Projects' });
	await rail.getByRole('button', { name: 'beta', exact: true }).click({ button: 'right' });
	await expect(page.getByRole('menuitem', { name: 'Close' })).toBeVisible();
	await expect(page.getByRole('menuitem', { name: 'New worktree…' })).toHaveCount(0);
	await page.keyboard.press('Escape');

	await rail.getByRole('button', { name: 'alpha', exact: true }).click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'New worktree…' }).click();
	const dialog = page.getByRole('dialog', { name: 'New worktree' });
	const from = dialog.getByRole('combobox', { name: 'From' });
	await expect(from).toHaveValue('main');
	await from.selectOption('origin/main');
	await dialog.getByRole('textbox', { name: 'New branch' }).fill('try/it');
	await page.screenshot({ path: test.info().outputPath('new-worktree.png') });
	await dialog.getByRole('button', { name: 'Create' }).click();
	await expect(dialog).toBeHidden();

	const tile = rail.getByRole('group', { name: 'Worktrees of alpha' }).getByRole('button', { name: 'try/it', exact: true });
	await expect(tile).toHaveText('It');
	await expect(tile).toHaveAttribute('aria-current', 'true');
	expect(git('rev-parse', 'try/it')).toBe(git('rev-parse', 'origin/main'));
	const sidebar = await showSidebar(page);
	await expect(sidebar.getByRole('paragraph').filter({ has: page.getByRole('img', { name: 'Branch' }) })).toHaveText('try/it');

	await projects();
	await tile.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Delete worktree' }).click();
	await expect(tile).toHaveCount(0);
	expect(git('branch', '--list', 'try/it')).toBe('');
});
