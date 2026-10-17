import type { Reading, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from '@playwright/test';
import { showSidebar } from './shell.ts';

test('usage groups agents into rows, fits narrow screens, and updates reset countdowns', async ({ page }, { project }) => {
	const now = new Date('2026-10-17T09:00:00-05:00').getTime();
	const minute = 60_000;
	const hour = 60 * minute;
	const day = 24 * hour;
	const readings: Reading[] = [
		{ agent: 'claude-code', window: 'five-hour', used: 1, readAt: now - 2 * minute, resetsAt: now + 3 * hour + 9 * minute },
		{ agent: 'claude-code', window: 'weekly', used: 0.12, readAt: now - 2 * minute, resetsAt: now + 6 * day + 14 * hour + 59 * minute },
		{ agent: 'codex', window: 'weekly', used: 0.43, readAt: now, resetsAt: now + 4 * day + 20 * hour + 15 * minute }
	];
	let update: (readings: Reading[]) => void = () => {};
	await page.clock.install({ time: now });
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.routeWebSocket('**/ws', (socket) => {
		update = (readings) => socket.send(JSON.stringify({ type: 'usage.changed', readings } satisfies ServerMessage));
		const server = socket.connectToServer();
		server.onMessage((data) => {
			if (typeof data !== 'string') return socket.send(data);
			const message: ServerMessage = JSON.parse(data);
			if (message.type === 'usage.changed') update(readings);
			else socket.send(data);
		});
	});
	await page.goto('/');
	let usage = page.getByRole('region', { name: 'Usage windows' });
	if (project.name === 'pixel') {
		await page.getByRole('button', { name: 'Previous screen' }).click();
		await expect.poll(() => page.locator('.screens').evaluate((element) => Math.abs(element.scrollLeft) < 1 && !element.children[0].hasAttribute('inert'))).toBe(true);
	} else {
		await expect(page.getByRole('button', { name: 'Sidebar' })).toBeVisible();
		usage = (await showSidebar(page)).getByRole('region', { name: 'Usage windows' });
	}
	await expect(usage).toBeInViewport();
	await page.clock.pauseAt(now + 5_000);
	await expect(usage.getByRole('group')).toHaveCount(2);
	await expect(usage.getByRole('meter')).toHaveCount(4);
	await expect(usage.locator('.countdown')).toHaveText(['(in 3h 9m)', '(in 6d 14h)', '(in 4d 20h)']);
	await expect(usage.getByRole('meter', { name: 'Codex · 5h' })).toHaveText('—');
	await expect(usage.getByText('Reset unavailable')).toHaveCount(1);
	await expect(usage.getByRole('group', { name: 'Claude', exact: true }).locator('.age')).toHaveText('2m ago');
	await expect(usage.getByRole('group', { name: 'Codex', exact: true }).locator('.age')).toHaveText('just now');
	for (const colorScheme of ['dark', 'light'] as const) {
		await page.emulateMedia({ colorScheme });
		const codexRing = usage.getByRole('meter', { name: 'Codex · week', exact: true });
		await expect.poll(async () => {
			const screenshot = await codexRing.screenshot();
			return page.evaluate(async (base64) => {
				const image = new Image();
				image.src = `data:image/png;base64,${base64}`;
				await image.decode();
				const canvas = document.createElement('canvas');
				canvas.width = image.width;
				canvas.height = image.height;
				const context = canvas.getContext('2d')!;
				context.drawImage(image, 0, 0);
				const arc = context.getImageData(Math.floor(image.width * 0.925), Math.floor(image.height / 2), 1, 1).data;
				const track = context.getImageData(Math.floor(image.width * 0.075), Math.floor(image.height / 2), 1, 1).data;
				return Math.hypot(...[0, 1, 2].map((channel) => arc[channel] - track[channel]));
			}, screenshot.toString('base64'));
		}).toBeGreaterThan(50);
		for (const name of ['Claude', 'Codex']) {
			const agent = usage.getByRole('group', { name, exact: true });
			await expect(agent.locator('.brand svg')).toBeVisible();
			await expect.poll(() => agent.evaluate((element) => {
				const logo = getComputedStyle(element.querySelector('.brand svg')!).color;
				const fill = getComputedStyle(element.querySelectorAll('.fill')[1]).stroke;
				return logo === fill;
			})).toBe(true);
		}
	}
	await expect.poll(() => usage.locator('.agent').last().evaluate((element) => getComputedStyle(element).borderTopWidth)).toBe('1px');
	const viewport = page.viewportSize()!;
	for (const width of project.name === 'pixel' ? [320, 360, 448, 640] : [viewport.width]) {
		await page.setViewportSize({ ...viewport, width });
		await expect.poll(() => usage.evaluate((element) => {
			const bounds = element.getBoundingClientRect();
			return element.scrollWidth <= element.clientWidth && [...element.querySelectorAll('.identity, .ring, .label, time, .countdown')].every((node) => {
				const rect = node.getBoundingClientRect();
				return rect.left >= bounds.left && rect.right <= bounds.right && rect.top >= bounds.top && rect.bottom <= bounds.bottom;
			}) && [...element.querySelectorAll('.agent')].every((agent) => {
				const [first, second] = agent.querySelectorAll('.ring');
				return Math.abs(first.getBoundingClientRect().top - second.getBoundingClientRect().top) < 1;
			});
		})).toBe(true);
	}
	await page.setViewportSize(viewport);
	await page.emulateMedia({ colorScheme: 'dark' });
	await usage.screenshot({ path: test.info().outputPath('usage.png') });
	await page.clock.fastForward(minute);
	await expect(usage.locator('.countdown').first()).toHaveText('(in 3h 8m)');
	update([{ ...readings[0], resetsAt: now + 2 * minute }]);
	await expect(usage.locator('.countdown')).toHaveText('(in 1m)');
	await page.clock.fastForward(minute);
	await expect(usage.locator('.countdown')).toHaveText('(now)');
	await page.clock.fastForward(minute);
	await expect(usage.locator('.countdown')).toHaveText('(now)');
	update([]);
	await expect(usage.getByText('—', { exact: true })).toHaveCount(4);
	await expect(usage.locator('.countdown')).toHaveCount(0);
	await expect(usage.getByText('Reset unavailable')).toHaveCount(4);
});
