import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { openProject, showSidebar } from './shell.ts';

const alpha = () => `${process.env.STAMPEDE_E2E_ROOT}/projects/alpha`;
const git = (...args: string[]) => execFileSync('git', ['-C', alpha(), ...args], { encoding: 'utf8' }).trim();

async function projectsScreen(page: Page) {
	if (test.info().project.name !== 'pixel') return;
	const back = page.getByRole('button', { name: 'Previous screen' });
	while (!(await back.isDisabled())) await back.click();
	await expect.poll(() => page.locator('.screens').evaluate((element) => element.scrollLeft)).toBe(0);
}

async function mint(page: Page, branch: string, from?: string) {
	await projectsScreen(page);
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'alpha', exact: true }).click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'New worktree…' }).click();
	const dialog = page.getByRole('dialog', { name: 'New worktree' });
	const select = dialog.getByRole('combobox', { name: 'From' });
	await expect(select).toHaveValue('main');
	if (from) await select.selectOption(from);
	await dialog.getByRole('textbox', { name: 'New branch' }).fill(branch);
	await page.screenshot({ path: test.info().outputPath(`new-worktree-${(branch || from!).replace('/', '-')}.png`) });
	await dialog.getByRole('button', { name: branch ? 'Create' : 'Open' }).click();
	await expect(dialog).toBeHidden();
}

test('a Project at a repository root mints a worktree from a chosen ref beside it until deleted', async ({ page }) => {
	git('update-ref', 'refs/remotes/origin/main', 'HEAD');
	await openProject(page, 'beta');
	await openProject(page, 'alpha');
	await projectsScreen(page);

	const rail = page.getByRole('navigation', { name: 'Projects' });
	await rail.getByRole('button', { name: 'beta', exact: true }).click({ button: 'right' });
	await expect(page.getByRole('menuitem', { name: 'Close' })).toBeVisible();
	await expect(page.getByRole('menuitem', { name: 'New worktree…' })).toHaveCount(0);
	await page.keyboard.press('Escape');

	await mint(page, 'try/it', 'origin/main');
	const tile = rail.getByRole('group', { name: 'Worktrees of alpha' }).getByRole('button', { name: 'try/it', exact: true });
	await expect(tile.locator('.tile')).toHaveText('It');
	await expect(tile).toHaveAttribute('aria-current', 'true');
	expect(git('rev-parse', 'try/it')).toBe(git('rev-parse', 'origin/main'));
	const sidebar = await showSidebar(page);
	await expect(sidebar.getByRole('paragraph').filter({ has: page.getByRole('img', { name: 'Branch' }) })).toHaveText('try/it');

	await projectsScreen(page);
	await tile.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Delete worktree' }).click();
	await expect(tile).toHaveCount(0);
	expect(git('branch', '--list', 'try/it')).toBe('');
});

test('a worktree opened on an existing branch leaves the branch behind when deleted', async ({ page }) => {
	git('branch', 'keep/me');
	await openProject(page, 'alpha');
	await mint(page, '', 'keep/me');

	const rail = page.getByRole('navigation', { name: 'Projects' });
	const tile = rail.getByRole('group', { name: 'Worktrees of alpha' }).getByRole('button', { name: 'keep/me', exact: true });
	await expect(tile).toHaveAttribute('aria-current', 'true');
	const sidebar = await showSidebar(page);
	await expect(sidebar.getByRole('paragraph').filter({ has: page.getByRole('img', { name: 'Branch' }) })).toHaveText('keep/me');

	await projectsScreen(page);
	await tile.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Delete worktree' }).click();
	await expect(tile).toHaveCount(0);
	expect(git('branch', '--list', 'keep/me')).toBe('keep/me');
});

test('worktrees are counted on their Project and listed under it, grouped in the collapsed rail only while it is active', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	await openProject(page, 'beta');
	await openProject(page, 'alpha');
	await mint(page, 'one');
	await mint(page, 'two');
	await projectsScreen(page);

	const rail = page.getByRole('navigation', { name: 'Projects' });
	const row = rail.getByRole('listitem').filter({ has: page.getByRole('button', { name: 'alpha', exact: true }) });
	const square = row.getByRole('button', { name: 'alpha', exact: true });
	const worktrees = row.getByRole('group', { name: 'Worktrees of alpha' });
	await expect(row.locator('.count')).toHaveText('2');
	await expect(worktrees.locator('.tile')).toHaveText(['On', 'Tw']);
	if (phone) await expect(worktrees.locator('.name')).toHaveText(['one', 'two']);
	const below = async () => {
		const [top, list] = [await square.boundingBox(), await worktrees.boundingBox()];
		return !!top && !!list && list.y >= top.y + top.height - 1 && (phone ? list.x > top.x : true);
	};
	await expect.poll(below).toBe(true);
	await page.screenshot({ path: test.info().outputPath('active.png') });

	if (phone) {
		const fold = row.getByRole('button', { name: 'Hide worktrees of alpha' });
		await expect(fold).toHaveAttribute('aria-expanded', 'true');
		await fold.click();
		await expect(worktrees).toHaveCount(0);
		await expect(row.locator('.count')).toHaveText('2');
		await row.getByRole('button', { name: 'Show worktrees of alpha' }).click();
		await expect(worktrees.getByRole('button')).toHaveCount(2);
		return;
	}

	await rail.getByRole('button', { name: 'beta', exact: true }).click();
	await expect(worktrees).toHaveCount(0);
	await expect(row.locator('.count')).toHaveText('2');
	await page.screenshot({ path: test.info().outputPath('inactive.png') });
	await square.click();
	await expect(worktrees.getByRole('button')).toHaveCount(2);
	await rail.getByRole('button', { name: 'beta', exact: true }).click();

	const toggle = rail.getByRole('button', { name: 'Expand project rail' });
	await expect(toggle).toHaveAttribute('aria-expanded', 'false');
	await toggle.click();
	await expect(row).toContainText(alpha());
	await expect(worktrees.locator('.name')).toHaveText(['one', 'two']);
	await expect.poll(async () => {
		const [top, list] = [await square.boundingBox(), await worktrees.boundingBox()];
		return !!top && !!list && list.y >= top.y + top.height - 1 && list.x > top.x;
	}).toBe(true);
	await page.screenshot({ path: test.info().outputPath('expanded.png') });
	await row.getByRole('button', { name: 'Hide worktrees of alpha' }).click();
	await expect(worktrees).toHaveCount(0);
	await expect(row.locator('.count')).toHaveText('2');

	await page.reload();
	await expect(rail.getByRole('button', { name: 'Collapse project rail' })).toHaveAttribute('aria-expanded', 'true');
	await expect(row).toContainText(alpha());
});

test('a working Chat in a worktree spins on its Project square', async ({ page }) => {
	await openProject(page, 'alpha');
	await mint(page, 'spin');
	await projectsScreen(page);
	const rail = page.getByRole('navigation', { name: 'Projects' });
	const square = rail.getByRole('button', { name: 'alpha', exact: true });
	await rail.getByRole('group', { name: 'Worktrees of alpha' }).getByRole('button', { name: 'spin', exact: true }).click();

	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude/ });
	await expect(tab.getByTitle('done')).toBeVisible();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	await composer.fill('Greet the reader');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.getByText(/^Hello and welcome!/)).toBeVisible();
	await expect(square.getByTitle('working')).toHaveCount(0);
	await composer.fill('Anything else?');
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(tab.getByTitle('working')).toBeVisible();

	await projectsScreen(page);
	await expect(square.getByTitle('working')).toBeVisible();
	await page.screenshot({ path: test.info().outputPath('working.png') });
});
