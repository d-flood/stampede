import type { Reading, ResetCredits, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import { showScreen } from './shell.ts';

test('Codex resets show in the Usage row and retry a redemption safely from its popover', async ({ page }, { project }) => {
	const now = Date.now();
	const day = 86_400_000;
	let credits: ResetCredits | null = { availableCount: 2, expiresAt: now + 2 * day, creditId: 'next' };
	let readings: Reading[] = [{ agent: 'codex', window: 'five-hour', used: 1, readAt: now }];
	let update = () => {};
	const attempts: { idempotencyKey: string; creditId?: string }[] = [];
	await page.clock.install({ time: now });
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.routeWebSocket('**/ws', (socket) => {
		update = () => socket.send(JSON.stringify({ type: 'usage.changed', readings, codexResets: credits } satisfies ServerMessage));
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data === 'string' && JSON.parse(data).type === 'usage.changed') update();
			else socket.send(data);
		});
	});
	await page.route('**/api/usage/codex/reset', async (route) => {
		attempts.push(route.request().postDataJSON());
		if (attempts.length === 1) return route.fulfill({ status: 502, body: 'Connection lost. Retry this reset.' });
		credits = { availableCount: 1, expiresAt: now + 17 * day, creditId: 'later' };
		readings = [{ ...readings[0], used: 0 }];
		update();
		await route.fulfill({ json: 'reset' });
	});
	await page.goto('/');
	let usage = page.getByRole('region', { name: 'Usage windows' });
	if (project.name === 'pixel') {
		await showScreen(page, 'projects');
		await expect.poll(() => page.locator('.screens').evaluate((element) => Math.abs(element.scrollLeft) < 1 && !element.children[0].hasAttribute('inert'))).toBe(true);
	} else {
		const sidebar = page.getByRole('complementary', { name: 'Sidebar' });
		await expect(sidebar).toBeVisible();
		usage = sidebar.getByRole('region', { name: 'Usage windows' });
	}
	const codex = usage.getByRole('group', { name: 'Codex', exact: true });
	await expect.poll(async () => (await usage.boundingBox())?.height).toBe(project.name === 'pixel' ? 58 : 50);
	await expect(codex).toHaveText('100 5h 2 resets');
	await usage.getByRole('button', { name: 'Usage details' }).click();
	const details = page.getByRole('dialog', { name: 'Usage details' });
	const row = details.locator('.resets');
	await expect(row).toContainText('2 resets');
	await expect(row.locator('time')).toHaveText('Expires in 2d');
	await row.getByRole('button', { name: 'Use a reset' }).click();
	await expect(row.getByText('Use 1 reset?')).toBeVisible();
	expect(attempts).toHaveLength(0);
	await row.getByRole('button', { name: 'Cancel', exact: true }).click();
	await row.getByRole('button', { name: 'Use a reset' }).click();
	await row.getByRole('button', { name: 'Confirm', exact: true }).click();
	await expect(row.getByRole('alert')).toHaveText('Connection lost. Retry this reset.');
	await row.getByRole('button', { name: 'Retry', exact: true }).click();
	await expect(row.getByRole('status')).toHaveText('Reset applied');
	expect(attempts).toHaveLength(2);
	expect(attempts[1]).toEqual(attempts[0]);
	expect(attempts[0].creditId).toBe('next');
	await expect(row).toContainText('1 reset');
	await expect(row.locator('time')).toHaveText('Expires in 17d');
	await expect(usage.getByRole('meter', { name: 'Codex · 5h' })).toHaveAttribute('aria-valuenow', '0');
	await expect(codex).toHaveText('0 5h 1 reset');
});
