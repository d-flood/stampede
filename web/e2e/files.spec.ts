import { expect, test } from '@playwright/test';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

test('file tree offers deletion for files and directories', async ({ page }, { project }) => {
	const name = `delete-${project.name}`;
	const root = `${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`;
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
	const root = `${process.env.OIKONOMOS_E2E_ROOT}/projects/${name}`;
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
