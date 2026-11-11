import { expect, test } from './fixtures.ts';
import type { WebSocketRoute } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

test('late Search replies stay with the Project that requested them', { tag: '@desktop' }, async ({ page }) => {
	await openProject(page, 'beta');
	const held: (string | Buffer)[] = [];
	let route: WebSocketRoute;
	let holding = false;
	await page.routeWebSocket('**/ws', (socket) => {
		route = socket;
		const server = socket.connectToServer();
		server.onMessage((message) => {
			if (typeof message === 'string' && JSON.parse(message).type === 'search.started') holding = true;
			if (holding) held.push(message);
			else socket.send(message);
		});
	});
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('textbox', { name: 'Search in files' }).fill('alpha');
	await expect.poll(() => held.some((message) => typeof message === 'string' && JSON.parse(message).type === 'search.done')).toBe(true);
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'beta', exact: true }).click();
	await expect(sidebar.getByRole('textbox', { name: 'Search in files' })).toHaveValue('');
	holding = false;
	for (const message of held) route!.send(message);
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await expect(page.getByRole('option', { name: /\/alpha$/ })).toBeVisible();
	await expect(sidebar.locator('.results')).toBeEmpty();
});

test('Search results replace the file tree until the search is cleared', async ({ page }, { project }) => {
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/alpha`;
	mkdirSync(`${root}/docs`, { recursive: true });
	writeFileSync(`${root}/docs/${project.name}-guide.txt`, 'a needle here\n');
	writeFileSync(`${root}/ignored/${project.name}-secret.txt`, 'a needle there\n');
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	const tree = sidebar.getByRole('group', { name: 'File tree' });
	const box = sidebar.getByRole('textbox', { name: 'Search in files' });
	const status = sidebar.getByRole('region', { name: 'Files' }).getByRole('status');
	const guide = tree.getByRole('button', { name: `${project.name}-guide.txt`, exact: true });

	await tree.getByRole('button', { name: 'docs', exact: true }).click();
	await expect(guide).toBeVisible();
	await expect(box).not.toBeFocused();

	await box.fill('needle');
	await expect(tree).toHaveCount(0);
	await expect(status).toHaveText('1 result in 1 file');
	await expect(sidebar.locator('.results')).toContainText(`${project.name}-guide.txt`);

	await sidebar.getByRole('button', { name: 'Show ignored files' }).click();
	await expect(status).toHaveText('2 results in 2 files');
	await expect(sidebar.locator('.results')).toContainText(`${project.name}-secret.txt`);

	const filters = sidebar.getByRole('button', { name: 'Include and exclude files' });
	await filters.click();
	await sidebar.getByRole('textbox', { name: 'Files to include' }).fill('docs/**');
	await expect(status).toHaveText('1 result in 1 file');
	await expect(filters).toBeDisabled();

	await sidebar.getByRole('radio', { name: 'Git', exact: true }).click();
	await expect(box).toBeHidden();
	await sidebar.getByRole('radio', { name: 'Files', exact: true }).click();
	await expect(box).toHaveValue('needle');
	await expect(status).toHaveText('1 result in 1 file');

	await sidebar.getByRole('button', { name: 'Clear search' }).click();
	await expect(box).toHaveValue('');
	await expect(box).toBeFocused();
	await expect(guide).toBeVisible();
	await expect(tree.getByRole('button', { name: 'ignored', exact: true })).toBeVisible();
	await sidebar.getByRole('button', { name: 'Hide ignored files' }).click();
	await expect(tree.getByRole('button', { name: 'ignored', exact: true })).toHaveCount(0);
});

test('Ctrl+Shift+F opens the Sidebar on Search with its query selected', { tag: '@desktop' }, async ({ page }) => {
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	const box = sidebar.getByRole('textbox', { name: 'Search in files' });
	await box.fill('needle');
	await sidebar.getByRole('radiogroup', { name: 'Sections' }).getByRole('radio', { name: 'Git', exact: true }).click();
	await page.getByRole('button', { name: 'Sidebar' }).click();
	await expect(sidebar).toBeHidden();
	await page.keyboard.press('Control+Shift+F');
	await expect(sidebar).toBeVisible();
	await expect(box).toBeFocused();
	await expect.poll(() => box.evaluate((input: HTMLInputElement) => [input.selectionStart, input.selectionEnd])).toEqual([0, 6]);
});
