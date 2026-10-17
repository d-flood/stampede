import { expect, test } from '@playwright/test';
import { writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test('a new server build offers a reload without discarding dirty edits', async ({ page }) => {
	let updated = false;
	let restart: (() => void) | undefined;
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		restart = () => { updated = true; socket.close(); server.close(); };
		server.onMessage((data) => {
			if (typeof data !== 'string') { socket.send(data); return; }
			const message = JSON.parse(data);
			if (message.type === 'hello' && updated) {
				message.buildId = 'next-build';
			}
			socket.send(JSON.stringify(message));
		});
	});
	const project = test.info().project.name === 'desktop' ? 'alpha' : 'beta';
	writeFileSync(`${process.env.OIKONOMOS_E2E_ROOT}/projects/${project}/rebuild.txt`, 'before rebuild\n');
	await openProject(page, project);
	const update = page.getByRole('status').filter({ has: page.getByRole('button', { name: 'Reload', exact: true }) });
	await expect(update).toBeHidden();
	restart!();
	await expect(update).toBeVisible();
	await expect.poll(() => update.evaluate((element) => {
		const box = element.getBoundingClientRect();
		return box.left >= 0 && box.right <= window.innerWidth && box.top >= 0;
	})).toBe(true);
	await page.screenshot({ path: test.info().outputPath('update.png') });
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'rebuild.txt', exact: true }).click();
	const editor = page.locator('.editor:not([hidden]) .cm-content');
	await editor.click();
	await page.keyboard.press('End');
	await page.keyboard.type(' with edits');
	await expect(update).toContainText('Save your edits to reload the update.');
	await expect(update.getByRole('button', { name: 'Reload', exact: true })).toBeDisabled();
	await expect(editor).toContainText('with edits');
	await page.keyboard.press('ControlOrMeta+s');
	await expect(update.getByRole('button', { name: 'Reload', exact: true })).toBeEnabled();
	updated = false;
	await update.getByRole('button', { name: 'Reload', exact: true }).click();
	await expect(page.getByRole('status').filter({ hasText: 'Oikonomos updated.' })).toBeHidden();
	await expect(editor).toContainText('with edits');
});
