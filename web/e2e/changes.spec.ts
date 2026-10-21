import { expect, test } from './fixtures.ts';
import type { Locator, Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

function repository(name: string) {
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(root);
	const git = (...args: string[]) => execFileSync('git', ['-C', root, ...args], { encoding: 'utf8' }).trim();
	git('init', '-q', '-b', 'main');
	for (const [key, value] of [['user.name', 'stampede'], ['user.email', 'e2e@stampede.invalid'], ['commit.gpgsign', 'false']]) git('config', key, value);
	return { root, git };
}

async function changesPanel(page: Page, name: string) {
	await openProject(page, name);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Changes', exact: true }).click();
	return sidebar;
}

const transparent = (locator: Locator) => locator.evaluate((element) => getComputedStyle(element).backgroundColor === 'rgba(0, 0, 0, 0)');

test('Changes shows a folder tree of changed files to stage, commit and undo', async ({ page }) => {
	const { root, git } = repository('tree');
	writeFileSync(`${root}/README.md`, '# tree\n');
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	mkdirSync(`${root}/src/lib`, { recursive: true });
	writeFileSync(`${root}/src/lib/one.ts`, 'export const one = 1;\n');
	writeFileSync(`${root}/src/lib/two.ts`, 'export const two = 2;\n');
	writeFileSync(`${root}/README.md`, '# tree, edited\n');
	const sidebar = await changesPanel(page, 'tree');
	const changes = sidebar.getByRole('list', { name: 'Changes', exact: true });
	const staged = sidebar.getByRole('list', { name: 'Staged Changes' });
	const numbers = { name: /^(one|two)\.ts$/ };

	await expect(changes.getByRole('button', { name: 'src/lib', exact: true })).toHaveAttribute('aria-expanded', 'true');
	await expect(changes.getByRole('button', numbers)).toHaveCount(2);
	await sidebar.getByRole('button', { name: 'Stage src/lib/', exact: true }).click();
	await expect(staged.getByRole('button', numbers)).toHaveCount(2);
	await expect(changes.getByRole('button', numbers)).toHaveCount(0);
	await expect(changes.getByRole('button', { name: 'README.md', exact: true })).toBeVisible();
	await page.screenshot({ path: test.info().outputPath('changes.png') });

	const message = sidebar.getByRole('textbox', { name: 'Commit message' });
	await message.fill('Add numbers');
	await sidebar.getByRole('button', { name: 'Commit', exact: true }).click();
	await expect(staged).toHaveCount(0);
	await expect(message).toHaveValue('');
	expect(git('log', '-1', '--format=%s')).toBe('Add numbers');

	await sidebar.getByRole('button', { name: 'More Git actions' }).click();
	await page.getByRole('menuitem', { name: 'Undo Last Commit' }).click();
	await expect(message).toHaveValue('Add numbers');
	await expect(staged.getByRole('button', numbers)).toHaveCount(2);
	expect(git('log', '-1', '--format=%s')).toBe('Initial commit');

	await sidebar.getByRole('button', { name: 'Unstage All Staged Changes' }).click();
	await expect(staged).toHaveCount(0);
	await sidebar.getByRole('button', { name: 'Discard README.md' }).click();
	const confirm = page.getByRole('dialog', { name: 'Discard changes?' });
	await expect(confirm).toContainText('README.md');
	await confirm.getByRole('button', { name: 'Discard' }).click();
	await expect(changes.getByRole('button', { name: 'README.md', exact: true })).toHaveCount(0);
	expect(readFileSync(`${root}/README.md`, 'utf8')).toBe('# tree\n');
});

test('a folder holding over 200 changes starts collapsed, again after switching Projects', async ({ page }, { project }) => {
	test.skip(project.name !== 'desktop');
	const { root } = repository('bulk');
	for (const folder of ['plugins', 'uploads']) {
		mkdirSync(`${root}/export/${folder}`, { recursive: true });
		for (let n = 0; n < 150; n++) writeFileSync(`${root}/export/${folder}/${n}.php`, '');
	}
	mkdirSync(`${root}/src`);
	writeFileSync(`${root}/src/app.ts`, '');
	await openProject(page, 'alpha');
	const sidebar = await changesPanel(page, 'bulk');
	const changes = sidebar.getByRole('list', { name: 'Changes', exact: true });
	const bulk = changes.getByRole('button', { name: 'export', exact: true });
	const projects = page.getByRole('navigation', { name: 'Projects' });

	await expect(bulk).toHaveAttribute('aria-expanded', 'false');
	await expect(changes.getByRole('button', { name: 'app.ts', exact: true })).toBeVisible();
	await expect(changes.getByRole('listitem')).toHaveCount(3);

	await bulk.click();
	await expect(changes.getByRole('button', { name: 'plugins', exact: true })).toHaveAttribute('aria-expanded', 'true');
	await expect(changes.getByRole('button', { name: /^\d+\.php$/ })).toHaveCount(300);

	await projects.getByRole('button', { name: 'alpha', exact: true }).click();
	await projects.getByRole('button', { name: 'bulk', exact: true }).click();
	await expect(bulk).toHaveAttribute('aria-expanded', 'false');
	await expect(changes.getByRole('listitem')).toHaveCount(3);
});

test('Changes takes LazyGit keys', async ({ page }, { project }) => {
	test.skip(project.name !== 'desktop');
	const { root, git } = repository('keys');
	writeFileSync(`${root}/a.txt`, 'a\n');
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	writeFileSync(`${root}/a.txt`, 'a, edited\n');
	writeFileSync(`${root}/b.txt`, 'b\n');
	const sidebar = await changesPanel(page, 'keys');
	const changes = sidebar.getByRole('list', { name: 'Changes', exact: true });
	const staged = sidebar.getByRole('list', { name: 'Staged Changes' });

	await changes.getByRole('button', { name: 'a.txt', exact: true }).focus();
	await page.keyboard.press('Space');
	await expect(staged.getByRole('button', { name: 'a.txt', exact: true })).toBeFocused();
	await page.keyboard.press('j');
	await page.keyboard.press('j');
	await expect(changes.getByRole('button', { name: 'b.txt', exact: true })).toBeFocused();
	await page.keyboard.press('d');
	const confirm = page.getByRole('dialog', { name: 'Discard changes?' });
	await expect(confirm).toContainText('This deletes 1 untracked file.');
	await confirm.getByRole('button', { name: 'Discard' }).click();
	await expect.poll(() => existsSync(`${root}/b.txt`)).toBe(false);

	await staged.getByRole('button', { name: 'a.txt', exact: true }).focus();
	await page.keyboard.press('c');
	await expect(sidebar.getByRole('textbox', { name: 'Commit message' })).toBeFocused();
	await page.keyboard.type('Edit a');
	await page.keyboard.press('Control+Enter');
	await expect(staged).toHaveCount(0);
	expect(git('log', '-1', '--format=%s')).toBe('Edit a');
});

test('a whole new or deleted file is marked in the gutter without tinting its lines', async ({ page }) => {
	const { root, git } = repository('whole');
	writeFileSync(`${root}/gone.ts`, 'export const gone = true;\n');
	writeFileSync(`${root}/kept.ts`, 'export const kept = 1;\n');
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	rmSync(`${root}/gone.ts`);
	writeFileSync(`${root}/fresh.ts`, 'export const fresh = true;\n');
	writeFileSync(`${root}/kept.ts`, 'export const kept = 2;\n');
	const sidebar = await changesPanel(page, 'whole');
	const diff = page.locator('.diff:not([hidden])');

	await sidebar.getByRole('list', { name: 'Changes', exact: true }).getByRole('button', { name: 'fresh.ts', exact: true }).click();
	const added = diff.locator('.cm-merge-b .cm-changedLine', { hasText: 'fresh' });
	await expect(added).toBeVisible();
	await expect.poll(() => transparent(added)).toBe(true);
	await expect.poll(() => transparent(diff.locator('.cm-merge-b .cm-changedLineGutter').first())).toBe(false);

	await (await showSidebar(page)).getByRole('list', { name: 'Changes', exact: true }).getByRole('button', { name: 'gone.ts', exact: true }).click();
	const removed = diff.locator('.cm-merge-a .cm-changedLine, .cm-deletedChunk').first();
	await expect(removed).toContainText('gone');
	await expect.poll(() => transparent(removed)).toBe(true);
	await page.screenshot({ path: test.info().outputPath('deleted.png') });

	await (await showSidebar(page)).getByRole('list', { name: 'Changes', exact: true }).getByRole('button', { name: 'kept.ts', exact: true }).click();
	const modified = diff.locator('.cm-merge-b .cm-changedLine', { hasText: 'kept = 2' });
	await expect(modified).toBeVisible();
	await expect.poll(() => transparent(modified)).toBe(false);
});

test('a staged file diffs HEAD with the index and an unstaged one the index with disk', async ({ page }) => {
	const { root, git } = repository('sides');
	writeFileSync(`${root}/file.ts`, 'export const value = 1;\n');
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	writeFileSync(`${root}/file.ts`, 'export const value = 2;\n');
	git('add', '.');
	writeFileSync(`${root}/file.ts`, 'export const value = 3;\n');
	const sidebar = await changesPanel(page, 'sides');
	const diff = page.locator('.diff:not([hidden])');

	await sidebar.getByRole('list', { name: 'Staged Changes' }).getByRole('button', { name: 'file.ts', exact: true }).click();
	await expect(diff.locator('.sides')).toHaveText('HEAD ↔ index');
	await expect(diff.locator('.cm-merge-b .cm-changedLine', { hasText: 'value = 2' })).toBeVisible();
	await (await showSidebar(page)).getByRole('list', { name: 'Changes', exact: true }).getByRole('button', { name: 'file.ts', exact: true }).click();
	await expect(diff.locator('.sides')).toHaveText('index ↔ disk');
	await expect(diff.locator('.cm-merge-b .cm-changedLine', { hasText: 'value = 3' })).toBeVisible();
});

test('the commit Rig drafts a message for the staged changes', async ({ page }) => {
	const { root, git } = repository('draft');
	git('commit', '-q', '--allow-empty', '-m', 'Initial commit');
	writeFileSync(`${root}/feature.ts`, 'export const feature = true;\n');
	const sidebar = await changesPanel(page, 'draft');
	const draft = sidebar.getByRole('button', { name: 'Draft' });
	await expect(draft).toBeDisabled();
	await sidebar.getByRole('button', { name: 'Stage All Changes' }).click();
	await expect(draft).toHaveAttribute('title', 'Draft a message with Fake Claude');
	await draft.click();
	await expect(sidebar.getByRole('textbox', { name: 'Commit message' })).toHaveValue('Hello and **welcome**! 👋');
});

test('Settings chooses the Rig that drafts commit messages', async ({ page }) => {
	await page.goto('/');
	await page.getByRole('button', { name: 'Settings' }).click();
	const select = page.getByRole('dialog', { name: 'Settings' }).getByRole('combobox', { name: 'Commit messages' });
	await expect(select).toHaveValue('Fake Claude');
	await select.selectOption('Fake Codex');
	await expect.poll(() => readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/config.toml`, 'utf8')).toContain('commit_rig = "Fake Codex"');
	await page.reload();
	await page.getByRole('button', { name: 'Settings' }).click();
	await expect(page.getByRole('dialog', { name: 'Settings' }).getByRole('combobox', { name: 'Commit messages' })).toHaveValue('Fake Codex');
});

test('Settings keeps the instructions for drafting commit messages', async ({ page }) => {
	const config = () => readFileSync(`${process.env.STAMPEDE_E2E_ROOT}/config.toml`, 'utf8');
	await page.goto('/');
	await page.getByRole('button', { name: 'Settings' }).click();
	const instructions = page.getByRole('dialog', { name: 'Settings' }).getByRole('textbox', { name: 'Instructions' });
	const initial = await instructions.inputValue();
	expect(initial).not.toBe('');
	await instructions.fill('Twelve words or fewer.\nNo body.');
	await instructions.blur();
	await expect.poll(config).toContain('commit_instructions = ');
	await page.reload();
	await page.getByRole('button', { name: 'Settings' }).click();
	await expect(instructions).toHaveValue('Twelve words or fewer.\nNo body.');
	await instructions.fill('');
	await instructions.blur();
	await expect(instructions).toHaveValue(initial);
	await expect.poll(config).not.toContain('commit_instructions');
});

test('a branch publishes to origin, then pushes and pulls by its counts', async ({ page }) => {
	const origin = `${process.env.STAMPEDE_E2E_ROOT}/origin.git`;
	execFileSync('git', ['init', '-q', '--bare', '-b', 'main', origin]);
	const { root, git } = repository('remote');
	git('remote', 'add', 'origin', origin);
	writeFileSync(`${root}/file.txt`, 'one\n');
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	const sidebar = await changesPanel(page, 'remote');
	const push = sidebar.getByRole('button', { name: 'Push' });
	const pull = sidebar.getByRole('button', { name: 'Pull' });

	await expect(sidebar.getByText('Not on origin')).toBeVisible();
	await sidebar.getByRole('button', { name: 'Publish Branch' }).click();
	await expect(sidebar.getByText('origin/main')).toBeVisible();
	await expect(push).toHaveText('0');
	await expect(push).toBeDisabled();

	writeFileSync(`${root}/file.txt`, 'two\n');
	git('commit', '-qam', 'Local');
	await expect(push).toHaveText('1');
	await push.click();
	await expect(push).toHaveText('0');
	expect(execFileSync('git', ['-C', origin, 'log', '-1', '--format=%s'], { encoding: 'utf8' }).trim()).toBe('Local');

	const other = `${process.env.STAMPEDE_E2E_ROOT}/other`;
	execFileSync('git', ['clone', '-q', origin, other]);
	writeFileSync(`${other}/remote.txt`, 'remote\n');
	for (const args of [['add', '.'], ['-c', 'user.name=other', '-c', 'user.email=other@stampede.invalid', '-c', 'commit.gpgsign=false', 'commit', '-qm', 'Remote'], ['push', '-q']]) {
		execFileSync('git', ['-C', other, ...args]);
	}
	await sidebar.getByRole('button', { name: 'Fetch' }).click();
	await expect(pull).toHaveText('1');
	await pull.click();
	await expect(pull).toHaveText('0');
	expect(readFileSync(`${root}/remote.txt`, 'utf8')).toBe('remote\n');
	await page.screenshot({ path: test.info().outputPath('remote.png') });
});

test('the sidebar branch switches to a local branch, or a fetched remote one tracked locally', async ({ page }) => {
	const origin = `${process.env.STAMPEDE_E2E_ROOT}/switch-origin.git`;
	execFileSync('git', ['init', '-q', '--bare', '-b', 'main', origin]);
	const { root, git } = repository('switch');
	git('remote', 'add', 'origin', origin);
	writeFileSync(`${root}/file.txt`, 'one\n');
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	git('push', '-q', '-u', 'origin', 'main');
	git('checkout', '-qb', 'feature');
	writeFileSync(`${root}/file.txt`, 'feature\n');
	git('commit', '-qam', 'Feature');
	git('checkout', '-q', 'main');
	await openProject(page, 'switch');
	const sidebar = await showSidebar(page);
	const dialog = page.getByRole('dialog', { name: 'Switch branch' });

	await sidebar.getByRole('button', { name: 'Branch main' }).click();
	await expect(dialog.getByRole('option', { name: 'main Current branch' })).toBeVisible();
	await expect(dialog.getByRole('option', { name: 'origin/main' })).toBeVisible();
	await page.screenshot({ path: test.info().outputPath('switch-branch.png') });
	await dialog.getByRole('combobox').fill('feat');
	await expect(dialog.getByRole('option')).toHaveCount(1);
	await dialog.getByRole('option', { name: 'feature' }).click();
	await expect(dialog).toBeHidden();
	await expect(sidebar.getByRole('button', { name: 'Branch feature' })).toBeVisible();
	expect(git('branch', '--show-current')).toBe('feature');

	const other = `${process.env.STAMPEDE_E2E_ROOT}/switch-other`;
	execFileSync('git', ['clone', '-q', origin, other]);
	execFileSync('git', ['-C', other, 'push', '-q', 'origin', 'main:fresh']);
	await sidebar.getByRole('button', { name: 'Branch feature' }).click();
	await expect(dialog.getByRole('option', { name: 'origin/main' })).toBeVisible();
	await expect(dialog.getByRole('option', { name: 'origin/fresh' })).toHaveCount(0);
	await dialog.getByRole('button', { name: 'Fetch' }).click();
	await dialog.getByRole('option', { name: 'origin/fresh' }).click();
	await expect(dialog).toBeHidden();
	await expect(sidebar.getByRole('button', { name: 'Branch fresh' })).toBeVisible();
	expect(git('rev-parse', '--abbrev-ref', 'fresh@{upstream}')).toBe('origin/fresh');

	writeFileSync(`${root}/file.txt`, 'dirty\n');
	await sidebar.getByRole('button', { name: 'Branch fresh' }).click();
	await dialog.getByRole('option', { name: 'feature' }).click();
	await expect(dialog.getByRole('alert')).toContainText('would be overwritten');
	expect(git('branch', '--show-current')).toBe('fresh');
	expect(readFileSync(`${root}/file.txt`, 'utf8')).toBe('dirty\n');
});

test('changes stash away and pop back', async ({ page }) => {
	const { root, git } = repository('stash');
	writeFileSync(`${root}/file.txt`, 'one\n');
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	writeFileSync(`${root}/file.txt`, 'two\n');
	const sidebar = await changesPanel(page, 'stash');
	const stashes = sidebar.getByRole('list', { name: 'Stashes' });

	await sidebar.getByRole('button', { name: 'More Git actions' }).click();
	await page.getByRole('menuitem', { name: 'Stash All Changes' }).click();
	await expect(stashes.getByRole('listitem')).toHaveCount(1);
	await expect(sidebar.getByText('No uncommitted changes')).toBeVisible();
	expect(readFileSync(`${root}/file.txt`, 'utf8')).toBe('one\n');
	await page.screenshot({ path: test.info().outputPath('stashes.png') });
	await stashes.getByRole('button', { name: /^Pop / }).click();
	await expect(stashes).toHaveCount(0);
	await expect(sidebar.getByRole('list', { name: 'Changes', exact: true }).getByRole('button', { name: 'file.txt', exact: true })).toBeVisible();
	expect(readFileSync(`${root}/file.txt`, 'utf8')).toBe('two\n');
});

test('a merge conflict blocks committing until it is resolved and staged', async ({ page }) => {
	const { root, git } = repository('conflict');
	writeFileSync(`${root}/clash.txt`, 'base\n');
	git('add', '.');
	git('commit', '-qm', 'Base');
	git('checkout', '-qb', 'other');
	writeFileSync(`${root}/clash.txt`, 'theirs\n');
	git('commit', '-qam', 'Theirs');
	git('checkout', '-q', 'main');
	writeFileSync(`${root}/clash.txt`, 'ours\n');
	git('commit', '-qam', 'Ours');
	expect(() => git('merge', '-q', 'other')).toThrow();
	const sidebar = await changesPanel(page, 'conflict');
	const conflicts = sidebar.getByRole('list', { name: 'Merge Changes' });
	const commit = sidebar.getByRole('button', { name: 'Commit Merge' });

	await expect(conflicts.getByRole('button', { name: 'clash.txt', exact: true })).toBeVisible();
	await expect(sidebar.getByRole('status')).toContainText('A merge is in progress');
	await sidebar.getByRole('textbox', { name: 'Commit message' }).fill('Merge other');
	await expect(commit).toBeDisabled();
	await sidebar.getByRole('button', { name: 'Stage clash.txt', exact: true }).click();
	await expect(sidebar.getByRole('alert')).toContainText('still has conflict markers');
	await page.screenshot({ path: test.info().outputPath('conflict.png') });

	writeFileSync(`${root}/clash.txt`, 'both\n');
	await sidebar.getByRole('button', { name: 'Stage clash.txt', exact: true }).click();
	await expect(conflicts).toHaveCount(0);
	await commit.click();
	await expect(sidebar.getByRole('status')).toHaveCount(0);
	expect(git('log', '-1', '--format=%s')).toBe('Merge other');
	expect(git('log', '-1', '--format=%P').split(' ')).toHaveLength(2);
});

test('History edits the message of an earlier commit', async ({ page }) => {
	const { root, git } = repository('reword');
	for (const name of ['First', 'Second']) {
		writeFileSync(`${root}/${name}.txt`, `${name}\n`);
		git('add', '.');
		git('commit', '-qm', name);
	}
	await openProject(page, 'reword');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'History' }).click();
	await sidebar.getByRole('button', { name: `Actions for ${git('rev-parse', '--short', 'HEAD~1')}` }).click();
	await page.getByRole('menuitem', { name: 'Edit Message…' }).click();
	const dialog = page.getByRole('dialog', { name: 'Edit commit message' });
	const message = dialog.getByRole('textbox', { name: 'Commit message' });
	await expect(message).toHaveValue('First');
	await page.screenshot({ path: test.info().outputPath('reword.png') });
	await message.fill('First, reworded\n\nWith a body');
	await dialog.getByRole('button', { name: 'Save' }).click();
	await expect(dialog).toBeHidden();
	await expect(sidebar.getByRole('list', { name: 'Commits' }).getByRole('button', { name: /First, reworded/ })).toBeVisible();
	expect(git('log', '-1', '--format=%B', 'HEAD~1')).toBe('First, reworded\n\nWith a body');
	expect(git('log', '-1', '--format=%s')).toBe('Second');
});

test('History copies the short and full hash of a commit', async ({ page, context }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	const { root, git } = repository('hashes');
	writeFileSync(`${root}/README.md`, '# hashes\n');
	git('add', '.');
	git('commit', '-qm', 'Initial');
	const short = git('rev-parse', '--short', 'HEAD');
	await openProject(page, 'hashes');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'History' }).click();
	await sidebar.getByRole('button', { name: `Actions for ${short}` }).click();
	await page.getByRole('menuitem', { name: 'Copy Short Hash' }).click();
	await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(short);
	await sidebar.getByRole('list', { name: 'Commits' }).getByRole('button', { name: /Initial/ }).focus();
	await page.keyboard.press('Shift+Y');
	await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(git('rev-parse', 'HEAD'));
});
