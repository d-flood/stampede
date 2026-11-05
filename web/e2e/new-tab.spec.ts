import { expect, test } from './fixtures.ts';
import { closeTab, expectCurrent, openProject, openSwitcher } from './shell.ts';

test.use({ starterRigs: true });

test('New Tab shows session loading and fits the loaded sessions', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	const item = phone ? 'button' : 'menuitem';
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
	if (!phone) await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	const menu = phone ? await openSwitcher(page) : page.getByRole('menu');
	await expect(menu.getByRole('status')).toHaveText('Reading sessions…');
	await expect(menu.getByRole(item, { name: 'Terminal', exact: true })).toBeEnabled();
	await expect.poll(() => release !== undefined).toBe(true);
	release!();
	await expect(menu.getByRole('status')).toHaveCount(0);
	await expect(menu.getByRole(item, { name: /Continue investigating/ })).toBeVisible();
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

test('the Launchpad offers the New Tab items and comes back focused when the last Tab closes', async ({ page }, { project }) => {
	const phone = project.name === 'pixel';
	await page.routeWebSocket('**/ws', (socket) => {
		const server = socket.connectToServer();
		server.onMessage((message) => {
			if (typeof message === 'string' && JSON.parse(message).type === 'sessions.listed') {
				const data = JSON.parse(message);
				socket.send(JSON.stringify({ ...data, sessions: [{ agent: 'codex', sessionId: 'past-session', title: 'Continue the collation', updatedAt: Date.now() }] }));
				return;
			}
			socket.send(message);
		});
	});
	await openProject(page, 'alpha');
	const launchpad = page.getByRole('region', { name: 'alpha', exact: true });
	await expect(launchpad.getByText(/, main, \d+ changed files?$/)).toBeVisible();
	const offered = ['Claude Code', 'Codex', 'Terminal', 'Browser', 'Open file', 'Continue the collation', 'Search sessions…'];
	await expect(launchpad.getByRole('button', { name: 'Continue the collation', exact: true })).toBeVisible();
	await expect(launchpad.getByRole('button')).toHaveCount(offered.length);
	for (const [index, label] of offered.entries()) await expect(launchpad.getByRole('button').nth(index)).toHaveAccessibleName(label);
	await expect(launchpad.getByRole('button', { name: 'Claude Code', exact: true })).toHaveAccessibleDescription('Default model');

	if (!phone) await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	const items = phone ? (await openSwitcher(page)).locator('section').getByRole('button') : page.getByRole('menu').getByRole('menuitem');
	await expect(items).toHaveCount(offered.length);
	for (const [index, label] of offered.entries()) await expect(items.nth(index)).toHaveAccessibleName(label);
	await page.keyboard.press('Escape');

	const primary = launchpad.getByRole('button', { name: 'Claude Code', exact: true });
	await primary.click();
	await expectCurrent(page, 'Claude Code');
	await expect(launchpad).toHaveCount(0);
	await closeTab(page, 'Claude Code');
	await expect(page.getByRole('tab', { name: /^Claude Code/ })).toHaveCount(0);
	if (phone) {
		await page.keyboard.press('Escape');
		await primary.click();
	} else {
		await expect(primary).toBeFocused();
		await page.keyboard.press('Enter');
	}
	await expectCurrent(page, 'Claude Code');
	await expect(launchpad).toHaveCount(0);
});

test('a Launchpad without Rigs offers Create a Rig, which opens Rigs settings', async ({ page }) => {
	await page.route('**/api/rigs', (route) => route.request().method() === 'GET' ? route.fulfill({ json: [] }) : route.fallback());
	await openProject(page, 'alpha');
	const launchpad = page.getByRole('region', { name: 'alpha', exact: true });
	await expect(launchpad.getByRole('button', { name: /^Terminal/ })).toBeVisible();
	await expect(launchpad.getByRole('button', { name: /^Browser/ })).toBeVisible();
	await expect(launchpad.getByRole('button', { name: /^Open file/ })).toBeVisible();
	await launchpad.getByRole('button', { name: /^Create a Rig/ }).click();
	const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
	await expect(settings.getByRole('heading', { name: 'Rigs', exact: true })).toBeVisible();
});
