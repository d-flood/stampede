import { expect, test } from './fixtures.ts';
import type { WebSocketRoute } from '@playwright/test';
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
	await sidebar.getByRole('button', { name: 'Search', exact: true }).click();
	await sidebar.getByRole('textbox', { name: 'Search', exact: true }).fill('alpha');
	await expect.poll(() => held.some((message) => typeof message === 'string' && JSON.parse(message).type === 'search.done')).toBe(true);
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'beta', exact: true }).click();
	await expect(sidebar.getByRole('textbox', { name: 'Search', exact: true })).toHaveValue('');
	holding = false;
	for (const message of held) route!.send(message);
	await page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await expect(page.getByRole('option', { name: /\/alpha$/ })).toBeVisible();
	await expect(sidebar.locator('.results')).toBeEmpty();
});
