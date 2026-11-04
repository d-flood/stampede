import { expect, test } from './fixtures.ts';
import { openProject } from './shell.ts';

test.use({ starterRigs: true });

test('New Tab shows session loading and fits the loaded sessions', async ({ page }) => {
	let release: (() => void) | undefined;
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((message) => {
			if (typeof message === 'string') {
				const data = JSON.parse(message);
				if (data.type === 'sessions.listed') {
					release = () => socket.send(JSON.stringify({ ...data, sessions: [{ agent: 'codex', sessionId: 'past-session', title: 'Continue investigating the manuscript transcription workflow', updatedAt: Date.now() }] }));
					return;
				}
			}
			socket.send(message);
		});
	});
	await openProject(page, 'alpha');
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	const menu = page.getByRole('menu');
	await expect(menu.getByRole('status')).toHaveText('Reading sessions…');
	await expect(menu.getByRole('menuitem', { name: 'Terminal', exact: true })).toBeEnabled();
	await expect.poll(() => release !== undefined).toBe(true);
	release!();
	await expect(menu.getByRole('status')).toHaveCount(0);
	await expect(menu.getByRole('menuitem', { name: /Continue investigating/ })).toBeVisible();
	await expect.poll(() => menu.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		return bounds.left >= 0 && bounds.right <= innerWidth;
	})).toBe(true);
	await page.keyboard.press('Escape');
	release = undefined;
	await page.keyboard.press('Control+T');
	const picker = page.getByRole('dialog', { name: 'New Tab', exact: true });
	await expect(picker.getByRole('status')).toHaveText('Reading sessions…');
	await expect.poll(() => release !== undefined).toBe(true);
	release!();
	await expect(picker.getByRole('status')).toHaveCount(0);
	await expect(picker.getByRole('combobox')).toBeFocused();
});
