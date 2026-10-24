import { mkdirSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { closeProject, openProject } from './shell.ts';

test('canceling a queued message returns text and images alongside the current draft', async ({ page }, { project }) => {
	const name = `unqueue-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude (steer)', exact: true }).click();
	const tab = page.getByRole('tab', { name: /^Fake Claude \(steer\)/ });
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	const queued = panel.getByRole('list', { name: 'Queued' }).getByRole('listitem');
	const attach = panel.getByLabel('Attach images', { exact: true });
	const image = new URL('../static/icons/icon-192.png', import.meta.url).pathname;
	const text = 'Return this queued message to the composer '.repeat(4).trim();
	await composer.fill('Count from 1 to 30, one number per line. Use no tools.');
	await panel.getByRole('button', { name: 'Send', exact: true }).click();
	await expect(tab.getByTitle('working')).toBeVisible();
	await composer.fill(text);
	await attach.setInputFiles(image);
	await panel.getByRole('button', { name: 'Queue', exact: true }).click();
	await expect(queued).toHaveText([text]);
	await composer.fill('Keep this queued');
	await panel.getByRole('button', { name: 'Queue', exact: true }).click();
	await expect(queued).toHaveText([text, 'Keep this queued']);
	await composer.fill('Current draft');
	await attach.setInputFiles(image);
	const cancel = queued.first().getByRole('button', { name: 'Cancel queued message', exact: true });
	await expect.poll(() => cancel.evaluate((node) => {
		const button = node.getBoundingClientRect();
		const item = node.closest('li')!.getBoundingClientRect();
		const minimum = parseFloat(getComputedStyle(node).getPropertyValue('--target-min'));
		return button.width >= minimum && button.height >= minimum && button.left >= item.left && button.right <= item.right && button.bottom <= item.bottom;
	})).toBe(true);
	await cancel.click();
	await expect(queued).toHaveText(['Keep this queued']);
	await expect(composer).toHaveValue(`${text}\n\nCurrent draft`);
	await expect(composer).toBeFocused();
	const attachments = panel.getByLabel('Attachments', { exact: true }).locator('img');
	await expect(attachments).toHaveCount(2);
	for (const attachment of await attachments.all()) {
		await expect.poll(() => attachment.evaluate((node: HTMLImageElement) => node.complete && node.naturalWidth > 0)).toBe(true);
	}
	await expect(tab.getByTitle('working')).toBeVisible();
	await expect(panel.getByText('Interrupted', { exact: true })).toHaveCount(0);
	await expect(panel.locator('.user', { hasText: text })).toHaveCount(0);
	await expect.poll(() => panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
	await closeProject(page, name);
});
