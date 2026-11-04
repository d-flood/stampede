import { expect, test } from './fixtures.ts';
import type { Locator, Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
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

async function comparePanel(page: Page) {
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Compare', exact: true }).click();
	return sidebar.getByRole('region', { name: 'Compare' });
}

async function pick(page: Page, compare: Locator, side: 'Target' | 'Source', name: string) {
	await compare.getByRole('button', { name: new RegExp(`^${side}`) }).click();
	const dialog = page.getByRole('dialog', { name: `Choose the ${side}` });
	await dialog.getByRole('option', { name, exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect(compare.getByRole('button', { name: new RegExp(`^${side}`) })).toContainText(name);
}

test('Compare walks what the Source changed since it split off the Target in one Diff tab', async ({ page }, { project }) => {
	const { root, git } = repository('compare');
	mkdirSync(`${root}/src`);
	writeFileSync(`${root}/src/a.ts`, 'export const a = 1;\n');
	writeFileSync(`${root}/src/b.ts`, 'export const b = 1;\n');
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	git('branch', 'older');
	git('switch', '-qc', 'feature');
	writeFileSync(`${root}/src/a.ts`, 'export const a = 2;\n');
	rmSync(`${root}/src/b.ts`);
	writeFileSync(`${root}/src/new.ts`, 'export const created = true;\n');
	git('add', '-A');
	git('commit', '-qm', 'Feature');
	git('switch', '-q', 'main');
	writeFileSync(`${root}/main.ts`, 'export const main = true;\n');
	git('add', '.');
	git('commit', '-qm', 'Main moves on');
	git('branch', 'same');
	git('switch', '-q', 'feature');

	await openProject(page, 'compare');
	let compare = await comparePanel(page);
	await expect(compare.getByRole('button', { name: /^Target/ })).toContainText('main');
	await expect(compare.getByRole('button', { name: /^Source/ })).toContainText('feature');
	const files = compare.getByRole('list', { name: 'Compared files' });
	await expect(files.getByRole('button', { name: /\.ts$/ })).toHaveCount(3);
	await expect(compare).toContainText('3 files');
	await expect(files.getByRole('button', { name: 'src', exact: true })).toHaveAttribute('aria-expanded', 'true');
	for (const [name, letter] of [['a.ts', 'M'], ['b.ts', 'D'], ['new.ts', 'A']]) {
		await expect(files.getByRole('button', { name, exact: true }).locator('.letter')).toHaveText(letter);
	}
	await expect(files.getByRole('button', { name: 'main.ts', exact: true })).toHaveCount(0);

	await pick(page, compare, 'Source', 'same');
	await expect(compare.getByRole('status')).toHaveText('No changes');
	await pick(page, compare, 'Source', 'feature');
	await pick(page, compare, 'Target', 'older');
	await expect(files.getByRole('button', { name: /\.ts$/ })).toHaveCount(3);

	const tabs = page.getByRole('tab');
	const count = await tabs.count();
	await files.getByRole('button', { name: 'a.ts', exact: true }).click();
	await expect(tabs).toHaveCount(count + 1);
	const diff = page.locator('.diff:not([hidden])');
	await expect(diff.locator('.path')).toHaveText('src/a.ts');
	await expect(page.getByRole('tab', { selected: true })).toContainText('older');
	if (project.name === 'desktop') await expect(diff.locator('.sides')).toHaveText('older ↔ feature');
	await expect(diff.locator('.cm-changedLine', { hasText: 'a = 2' })).toBeVisible();
	await diff.locator('.cm-content').last().click();
	await page.keyboard.type('zzz');
	await expect(diff.locator('.cm-content').last()).not.toContainText('zzz');

	const previous = diff.getByRole('button', { name: 'Previous file' });
	const next = diff.getByRole('button', { name: 'Next file' });
	await expect(previous).toBeDisabled();
	await next.click();
	await expect(diff.locator('.path')).toHaveText('src/b.ts');
	await expect(tabs).toHaveCount(count + 1);
	await page.keyboard.press('Alt+ArrowUp');
	await expect(diff.locator('.path')).toHaveText('src/a.ts');
	await page.keyboard.press('Alt+ArrowDown');
	await expect(diff.locator('.path')).toHaveText('src/b.ts');
	await page.keyboard.press('Alt+ArrowDown');
	await expect(diff.locator('.path')).toHaveText('src/new.ts');
	await expect(diff.locator('.cm-changedLine', { hasText: 'created' })).toBeVisible();
	await expect(next).toBeDisabled();
	await page.keyboard.press('Alt+ArrowDown');
	await expect(diff.locator('.path')).toHaveText('src/new.ts');
	await previous.click();
	await expect(diff.locator('.path')).toHaveText('src/b.ts');
	await previous.click();
	await expect(diff.locator('.path')).toHaveText('src/a.ts');
	await expect(tabs).toHaveCount(count + 1);
	await expect(page.getByRole('tab', { selected: true })).toContainText('a.ts');

	writeFileSync(`${root}/src/a.ts`, 'export const a = 3;\n');
	writeFileSync(`${root}/src/later.ts`, 'export const later = true;\n');
	git('add', '.');
	git('commit', '-qm', 'More');
	await expect(diff.locator('.cm-changedLine', { hasText: 'a = 3' })).toBeVisible();
	compare = await comparePanel(page);
	await expect(files.getByRole('button', { name: 'later.ts', exact: true })).toBeVisible();
	await expect(compare).toContainText('4 files');

	await page.reload();
	await expect(page.locator('.diff:not([hidden]) .path')).toHaveText('src/a.ts');
	await expect(tabs).toHaveCount(count + 1);
	compare = await comparePanel(page);
	await expect(compare.getByRole('button', { name: /^Target/ })).toContainText('older');
	await expect(compare.getByRole('button', { name: /^Source/ })).toContainText('feature');
	await expect(files.getByRole('button', { name: /\.ts$/ })).toHaveCount(4);
});

test('An open Comparison Diff follows its file when it becomes a rename', async ({ page }) => {
	const { root, git } = repository('compare-rename');
	const lines = Array.from({ length: 30 }, (_, n) => `export const line${n} = ${n};\n`).join('');
	writeFileSync(`${root}/old.ts`, lines);
	git('add', '.');
	git('commit', '-qm', 'Initial commit');
	git('switch', '-qc', 'feature');
	writeFileSync(`${root}/new.ts`, 'export const unrelated = true;\n');
	git('add', '.');
	git('commit', '-qm', 'Add new');

	await openProject(page, 'compare-rename');
	const compare = await comparePanel(page);
	await compare.getByRole('list', { name: 'Compared files' }).getByRole('button', { name: 'new.ts', exact: true }).click();
	const diff = page.locator('.diff:not([hidden])');
	await expect(diff.locator('.cm-changedLine', { hasText: 'unrelated' })).toBeVisible();

	rmSync(`${root}/old.ts`);
	writeFileSync(`${root}/new.ts`, lines.replace('line5 = 5;', 'line5 = 50;'));
	git('add', '-A');
	git('commit', '-qm', 'Rename');
	await expect(diff.locator('.path')).toHaveText('old.ts → new.ts');
	await expect(diff.locator('.cm-changedLine', { hasText: 'line5 = 50' })).toBeVisible();
	await expect(diff.locator('.cm-changedLine', { hasText: 'line6 = 6' })).toHaveCount(0);
});

test('Compare names a branch that no longer exists and keeps its pickers usable', async ({ page }) => {
	const { git } = repository('vanished');
	git('commit', '-q', '--allow-empty', '-m', 'Initial commit');
	git('branch', 'gone');
	git('branch', 'kept');
	await openProject(page, 'vanished');
	const compare = await comparePanel(page);
	await pick(page, compare, 'Source', 'gone');
	await expect(compare.getByRole('status')).toHaveText('No changes');
	git('branch', '-D', 'gone');
	await compare.getByRole('button', { name: /^Target/ }).click();
	await page.getByRole('dialog', { name: 'Choose the Target' }).getByRole('option', { name: 'kept', exact: true }).click();
	await expect(compare.getByRole('alert')).toHaveText('gone does not exist');
	await pick(page, compare, 'Source', 'main');
	await expect(compare.getByRole('status')).toHaveText('No changes');
});
