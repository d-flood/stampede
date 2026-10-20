import { expect, test } from './fixtures.ts';
import type { WebSocketRoute } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { openProject } from './shell.ts';

test('a socket that died while the page was hidden is replaced as soon as the page shows', async ({ page }, { project }) => {
	let connections = 0;
	let silenced = false;
	await page.routeWebSocket('**/ws', (socket) => {
		const first = ++connections === 1;
		const server = socket.connectToServer();
		socket.onMessage((data) => !(first && silenced) && server.send(data));
		server.onMessage((data) => !(first && silenced) && socket.send(data));
	});
	const name = `reconnect-silent-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	silenced = true;
	await page.evaluate(() => document.dispatchEvent(new Event('visibilitychange')));
	await expect.poll(() => connections, { timeout: 3_000 }).toBe(2);
	await expect(page.getByRole('status').filter({ hasText: 'Reconnecting…' })).toBeHidden();
});

test('a reconnect the server never answers is retried instead of waited on', async ({ page }, { project }) => {
	let connections = 0;
	let route: WebSocketRoute | undefined;
	await page.routeWebSocket('**/ws', (socket) => {
		if (++connections === 2) return;
		route = socket;
		socket.connectToServer();
	});
	const name = `reconnect-unanswered-${project.name}`;
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`);
	await openProject(page, name);
	const reconnecting = page.getByRole('status').filter({ hasText: 'Reconnecting…' });
	await route!.close();
	await expect(reconnecting).toBeVisible();
	await expect.poll(() => connections).toBe(2);
	await expect(reconnecting).toBeHidden({ timeout: 6_000 });
	expect(connections).toBe(3);
});
