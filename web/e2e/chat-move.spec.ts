import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { openProject, showSidebar } from './shell.ts';

const alpha = () => `${process.env.STAMPEDE_E2E_ROOT}/projects/alpha`;
const git = (dir: string, ...args: string[]) => execFileSync('git', ['-C', dir, ...args], { encoding: 'utf8' }).trimEnd();

test('a Chat moves to a new worktree with the uncommitted changes and a note in its Composer', async ({ page }, { project }) => {
	git(alpha(), 'config', 'user.name', 'stampede');
	git(alpha(), 'config', 'user.email', 'e2e@stampede.invalid');
	await openProject(page, 'alpha');
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('Say hello');
	await panel.getByRole('button', { name: 'Send', exact: true }).click();
	await expect(panel.getByText('Hello and', { exact: false })).toBeVisible();
	await expect(page.getByRole('tab', { name: /^Fake Claude/ }).getByTitle('done')).toBeVisible();
	await expect.poll(() => panel.locator('.composer .actions').evaluate((element) => {
		const buttons = ['Attach image', 'New Chat from here…', 'Move to new worktree…', 'Send']
			.map((label) => element.querySelector(`button[aria-label="${label}"]`)?.getBoundingClientRect());
		const row = element.getBoundingClientRect();
		return buttons.every((button) => button && Math.abs(button.top - buttons[0]!.top) < 1 && button.left >= row.left && button.right <= row.right);
	})).toBe(true);
	writeFileSync(`${alpha()}/${project.name}.txt`, 'moved\n');

	await panel.getByRole('button', { name: 'Move to new worktree…' }).click();
	const dialog = page.getByRole('dialog', { name: 'Move to new worktree' });
	const branch = dialog.getByRole('textbox', { name: 'New branch' });
	await expect(branch).toBeFocused();
	await expect.poll(() => dialog.evaluate((element) => {
		const rect = element.getBoundingClientRect();
		return rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight;
	})).toBe(true);
	await branch.fill('bad..name');
	await dialog.getByRole('button', { name: 'Move', exact: true }).click();
	await expect(dialog.getByRole('alert')).toHaveText('bad..name is not a valid branch name');
	await branch.fill('moved');
	await page.screenshot({ path: test.info().outputPath('move-to-worktree.png') });
	await dialog.getByRole('button', { name: 'Move', exact: true }).click();
	await expect(dialog).toBeHidden();

	const worktree = git(alpha(), 'worktree', 'list', '--porcelain').split('\n').find((line) => line.startsWith('worktree ') && line.endsWith('/moved'))!.slice('worktree '.length);
	await expect(page.getByRole('tab', { name: /^Fake Claude/ })).toHaveAttribute('aria-selected', 'true');
	await expect(composer).toHaveValue(new RegExp(`^FYI: Stampede moved this Chat to a new git worktree\\. The working directory is now \`${worktree}\`, on the new branch \`moved\`, cut from \`main\` at \`\\w+\`, and the uncommitted changes moved with it\\.`));
	expect(git(worktree, 'status', '--porcelain')).toBe(` M ${project.name}.txt`);
	expect(git(alpha(), 'status', '--porcelain')).toBe('');
	const sidebar = await showSidebar(page);
	await expect(sidebar.getByRole('paragraph').filter({ has: page.getByRole('img', { name: 'Branch' }) })).toHaveText('moved');
});
