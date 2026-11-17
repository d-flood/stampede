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
	await sidebar.getByRole('radio', { name: 'Git', exact: true }).click();
	await sidebar.getByRole('list', { name: 'Staged Changes' }).getByRole('button', { name: 'after.ts', exact: true }).click();
	const diff = page.locator('.diff:not([hidden])');
	await expect(diff.locator('.path')).toHaveText('before.ts → after.ts');
	await expect(diff.locator('.cm-changedLine', { hasText: 'export const four' })).toBeVisible();
	git('commit', '-qm', 'Rename file');
	await expect(diff.locator('.cm-changedLine')).toHaveCount(0);
	await expect(diff.locator('.path')).toHaveText('after.ts');
});

test('an image Diff shows both versions and follows disk', { tag: '@desktop' }, async ({ page }) => {
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/pictures`;
	mkdirSync(root);
	const git = (...args: string[]) => execFileSync('git', ['-C', root, '-c', 'user.name=stampede', '-c', 'user.email=e2e@stampede.invalid', '-c', 'commit.gpgsign=false', ...args]);
	const pngs = await page.evaluate(() => [40, 80, 120].map((width) => {
		const canvas = Object.assign(document.createElement('canvas'), { width, height: 30 });
		return canvas.toDataURL('image/png').split(',')[1];
	}));
	const write = (index: number) => writeFileSync(`${root}/image.png`, Buffer.from(pngs[index], 'base64'));
	git('init', '-q');
	write(0);
	git('add', '.');
	git('commit', '-qm', 'Add image');
	write(1);
	await openProject(page, 'pictures');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('radio', { name: 'Git', exact: true }).click();
	await sidebar.getByRole('list', { name: 'Changes', exact: true }).getByRole('button', { name: 'image.png', exact: true }).click();
	const images = page.locator('.diff:not([hidden]) figure img');
	const widths = () => images.evaluateAll((elements) => elements.map((element) => (element as HTMLImageElement).naturalWidth));
	await expect.poll(widths).toEqual([40, 80]);
	write(2);
	await expect.poll(widths).toEqual([40, 120]);
});
