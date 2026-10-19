import { expect, test } from './fixtures.ts';
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

test('context menus create files and directories in the selected location', async ({ page }, { project }) => {
	const name = `create-${project.name}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(root);
	await openProject(page, name);
	let sidebar = await showSidebar(page);
	const tree = sidebar.getByRole('group', { name: 'File tree', exact: true });
	await tree.click({ button: 'right', position: { x: 10, y: 10 } });
	await page.getByRole('menuitem', { name: 'New File', exact: true }).click();
	const fileDialog = page.getByRole('dialog', { name: 'New File', exact: true });
	await fileDialog.getByLabel('Name', { exact: true }).fill('root.txt');
	await fileDialog.getByRole('button', { name: 'Cancel', exact: true }).click();
	expect(existsSync(`${root}/root.txt`)).toBe(false);
	await tree.click({ button: 'right', position: { x: 10, y: 10 } });
	await page.getByRole('menuitem', { name: 'New File', exact: true }).click();
	await fileDialog.getByLabel('Name', { exact: true }).fill('root.txt');
	await fileDialog.getByRole('button', { name: 'Create', exact: true }).click();
	await expect(fileDialog).toBeHidden();
	await expect(page.locator('.editor:not([hidden]) .cm-content')).toBeVisible();
	expect(readFileSync(`${root}/root.txt`, 'utf8')).toBe('');
	sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'root.txt', exact: true }).click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'New Directory', exact: true }).click();
	const directoryDialog = page.getByRole('dialog', { name: 'New Directory', exact: true });
	await directoryDialog.getByLabel('Name', { exact: true }).fill('docs');
	await directoryDialog.getByLabel('Name', { exact: true }).press('Enter');
	await expect(sidebar.getByRole('button', { name: 'docs', exact: true })).toBeVisible();
	expect(statSync(`${root}/docs`).isDirectory()).toBe(true);
	await sidebar.getByRole('button', { name: 'docs', exact: true }).click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'New File', exact: true }).click();
	await expect(fileDialog).toContainText('Create in docs');
	await fileDialog.getByLabel('Name', { exact: true }).fill('notes.md');
	await expect.poll(async () => {
		const box = await fileDialog.boundingBox();
		const viewport = page.viewportSize();
		return box && viewport && box.x >= 0 && box.x + box.width <= viewport.width;
	}).toBe(true);
	await page.screenshot({ path: test.info().outputPath('new-file-dialog.png') });
	await fileDialog.getByRole('button', { name: 'Create', exact: true }).click();
	await expect(fileDialog).toBeHidden();
	expect(readFileSync(`${root}/docs/notes.md`, 'utf8')).toBe('');
	sidebar = await showSidebar(page);
	await expect(sidebar.getByRole('button', { name: 'notes.md', exact: true })).toBeVisible();
	writeFileSync(`${root}/docs/notes.md`, 'keep these contents');
	await sidebar.getByRole('button', { name: 'notes.md', exact: true }).click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'New File', exact: true }).click();
	await expect(fileDialog).toContainText('Create in docs');
	await fileDialog.getByLabel('Name', { exact: true }).fill('notes.md');
	await fileDialog.getByRole('button', { name: 'Create', exact: true }).click();
	await expect(fileDialog.getByRole('alert')).toHaveText('A file or directory with that name already exists.');
	expect(readFileSync(`${root}/docs/notes.md`, 'utf8')).toBe('keep these contents');
	await fileDialog.getByRole('button', { name: 'Cancel', exact: true }).click();
});

test('file tree offers deletion for files and directories', async ({ page }, { project }) => {
	const name = `delete-${project.name}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(`${root}/folder/nested`, { recursive: true });
	writeFileSync(`${root}/remove.txt`, 'remove me\n');
	writeFileSync(`${root}/folder/nested/child.txt`, 'child\n');
	await openProject(page, name);
	const sidebar = await showSidebar(page);
	const file = sidebar.getByRole('button', { name: 'remove.txt', exact: true });
	await expect(file).toHaveAttribute('tabindex', '0');
	await page.evaluate(() => document.addEventListener('contextmenu', (event) => {
		document.documentElement.dataset.contextPrevented = String(event.defaultPrevented);
	}));
	if (project.name === 'pixel') {
		const actions = sidebar.getByRole('button', { name: 'Actions for remove.txt', exact: true });
		await expect.poll(async () => {
			const box = await actions.boundingBox();
			return box && box.width >= 48 && box.height >= 48;
		}).toBe(true);
		await actions.tap();
	} else {
		await file.click({ button: 'right' });
		await expect(page.locator('html')).toHaveAttribute('data-context-prevented', 'true');
	}
	await expect(page.getByRole('menuitem', { name: 'Delete', exact: true })).toBeVisible();
	await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
	const dialog = page.getByRole('dialog', { name: 'Delete file?' });
	await expect(dialog).toContainText('remove.txt');
	await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
	expect(existsSync(`${root}/remove.txt`)).toBe(true);
	await file.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
	await dialog.getByRole('button', { name: 'Delete', exact: true }).click();
	await expect(file).toHaveCount(0);
	expect(existsSync(`${root}/remove.txt`)).toBe(false);
	await sidebar.getByRole('button', { name: 'folder', exact: true }).click();
	await sidebar.getByRole('button', { name: 'nested', exact: true }).click();
	await expect(sidebar.getByRole('button', { name: 'child.txt', exact: true })).toBeVisible();
	await page.screenshot({ path: test.info().outputPath('file-tree-actions.png') });
	await sidebar.getByRole('button', { name: 'folder', exact: true }).click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
	const directory = page.getByRole('dialog', { name: 'Delete directory?' });
	await expect(directory).toContainText('folder');
	await directory.getByRole('button', { name: 'Delete', exact: true }).click();
	await expect(sidebar.getByRole('button', { name: 'folder', exact: true })).toHaveCount(0);
	expect(existsSync(`${root}/folder`)).toBe(false);
});

test('failed deletion preserves the entry and can be retried', async ({ page }, { project }) => {
	const name = `delete-error-${project.name}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(root);
	writeFileSync(`${root}/keep.txt`, 'keep me\n');
	await openProject(page, name);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'Actions for keep.txt', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Delete', exact: true }).click();
	await page.route('**/api/projects/*/file?*', async (route) => {
		if (route.request().method() === 'DELETE') await route.fulfill({ status: 403, body: 'Permission denied' });
		else await route.continue();
	});
	const dialog = page.getByRole('dialog', { name: 'Delete file?' });
	await dialog.getByRole('button', { name: 'Delete', exact: true }).click();
	await expect(dialog.getByRole('alert')).toHaveText('Permission denied');
	expect(existsSync(`${root}/keep.txt`)).toBe(true);
	await page.unroute('**/api/projects/*/file?*');
	await dialog.getByRole('button', { name: 'Delete', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect(sidebar.getByRole('button', { name: 'keep.txt', exact: true })).toHaveCount(0);
	expect(existsSync(`${root}/keep.txt`)).toBe(false);
});
