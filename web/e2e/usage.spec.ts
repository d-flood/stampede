import type { Reading, ServerMessage } from '#lib/device.svelte.ts';
import { expect, test } from './fixtures.ts';
import { showSidebar } from './shell.ts';

test('usage groups agents into rows, fits narrow screens, and updates reset countdowns', async ({ page }, { project }) => {
	const now = new Date('2026-10-17T09:00:00-05:00').getTime();
	const minute = 60_000;
	const hour = 60 * minute;
	const day = 24 * hour;
	const readings: Reading[] = [
		{ agent: 'claude-code', window: 'five-hour', used: 1, readAt: now - 2 * minute, resetsAt: now + 3 * hour + 9 * minute },
		{ agent: 'claude-code', window: 'weekly', used: 0.12, readAt: now - 2 * minute, resetsAt: now + 6 * day + 14 * hour + 59 * minute },
		{ agent: 'codex', window: 'weekly', used: 0.43, readAt: now, resetsAt: now + 4 * day + 20 * hour + 15 * minute },
		{ agent: 'copilot', window: 'monthly', consumption: { amount: 1234.5, unit: 'credits' }, readAt: now, startsAt: Date.parse('2026-10-01T00:00:00Z'), resetsAt: Date.parse('2026-11-01T00:00:00Z') }
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
	if (project.name === 'desktop') {
		await page.getByRole('separator', { name: 'Resize sidebar' }).focus();
		await page.keyboard.press('End');
	}
	await page.clock.pauseAt(now + 5_000);
	const meter = usage.getByRole('meter', { name: 'Claude · 5h' });
	const tooltip = page.getByRole('tooltip');
	await expect(usage.locator('.reset').first()).toBeVisible();
	await meter.hover();
	await meter.focus();
	await page.clock.runFor(500);
	await expect(tooltip).toBeHidden();
	if (project.name === 'desktop') {
		const reset = `${await usage.locator('time').first().textContent()} ${await usage.locator('.countdown').first().textContent()}`;
		const resizer = page.getByRole('separator', { name: 'Resize sidebar' });
		await resizer.focus();
		await page.mouse.move(900, 400);
		await page.keyboard.press('Home');
		await expect(usage.locator('.reset').first()).toBeHidden();
		await meter.hover();
		await expect(tooltip).toHaveText(reset);
		await resizer.focus();
		await page.keyboard.press('End');
		await expect(usage.locator('.reset').first()).toBeVisible();
		await page.clock.runFor(100);
		await expect(tooltip).toBeHidden();
		await page.mouse.move(900, 400);
		await meter.focus();
		await page.clock.runFor(500);
		await expect(tooltip).toBeHidden();
	}
	await expect(usage.getByRole('group')).toHaveCount(3);
	await expect(usage.getByRole('meter')).toHaveCount(3);
	const copilot = usage.getByRole('group', { name: 'Copilot', exact: true });
	const consumption = copilot.locator('.amount');
	async function checkNarrowBilling(name: string) {
		const viewport = page.viewportSize()!;
		const height = (await copilot.boundingBox())!.height;
		const resizer = page.getByRole('separator', { name: 'Resize sidebar' });
		for (const width of project.name === 'desktop' ? [640, 384, 320, 256, 208, 160] : [448, 360, 320]) {
			if (project.name === 'desktop') {
				await resizer.focus();
				while (Number(await resizer.getAttribute('aria-valuenow')) > width) await page.keyboard.press('ArrowLeft');
			} else await page.setViewportSize({ ...viewport, width });
			await expect.poll(() => copilot.evaluate((element, height) => {
				const billing = element.querySelector('.billing')!;
				const bounds = billing.getBoundingClientRect();
				const lines = [...billing.children].filter((child) => child.getClientRects().length);
				return {
					grows: element.getBoundingClientRect().height > height + 1,
					overflows: lines.some((child) => child.scrollWidth > child.clientWidth || child.getBoundingClientRect().right > bounds.right + 1),
					wraps: getComputedStyle(element).getPropertyValue('--compact').trim() === '1' && (lines.length > 2 || lines.some((child) => child.getBoundingClientRect().height > parseFloat(getComputedStyle(child).lineHeight) + 1))
				};
			}, height)).toEqual({ grows: false, overflows: false, wraps: false });
		}
		if (name === 'cost') {
			await copilot.locator('.billing').focus();
			await page.clock.runFor(500);
			await expect(tooltip).toHaveText('Reported cost; some usage may be missing');
			await page.keyboard.press('Escape');
			await page.clock.runFor(100);
			await expect(tooltip).toBeHidden();
		}
		await copilot.screenshot({ path: test.info().outputPath(`copilot-${name}-narrow.png`) });
		if (project.name === 'desktop') {
			await resizer.focus();
			await page.keyboard.press('End');
		} else await page.setViewportSize(viewport);
	}
	await expect(consumption).toHaveText('$12.345');
	await expect(copilot.locator('.credits')).toHaveText('1,234.5 credits');
	await expect(copilot.getByText('This billing period', { exact: true })).toBeVisible();
	await expect(copilot.locator('.cycle')).toHaveText('Oct 1 – Nov 1');
	await expect(copilot.getByRole('meter')).toHaveCount(0);
	await expect(copilot.locator('.brand svg')).toBeVisible();
	await expect.poll(() => copilot.evaluate((element) => element.getBoundingClientRect().top >= element.previousElementSibling!.getBoundingClientRect().bottom)).toBe(true);
	await expect(usage.locator('.countdown')).toHaveText(['(in 3h 9m)', '(in 6d 14h)', '(in 4d 20h)']);
	await expect(usage.getByRole('meter', { name: 'Codex · 5h' })).toHaveCount(0);
	await expect(usage.getByText('Reset unavailable')).toHaveCount(0);
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
				const fill = getComputedStyle(element.querySelector('.meter:last-child .fill')!).stroke;
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
			const outside = [...element.querySelectorAll('.identity, .ring, .label, time, .countdown, .amount, .credits, .period, .cycle')].filter((node) => {
				if (!node.getClientRects().length) return false;
				const rect = node.getBoundingClientRect();
				return rect.left < bounds.left || rect.right > bounds.right || rect.top < bounds.top || rect.bottom > bounds.bottom;
			}).map((node) => `${node.className}: ${node.textContent}`);
			const misaligned = [...element.querySelectorAll('.agent')].filter((agent) => {
				const rings = [...agent.querySelectorAll('.ring')];
				return rings.some((ring) => Math.abs(ring.getBoundingClientRect().top - rings[0].getBoundingClientRect().top) >= 1);
			}).map((agent) => agent.getAttribute('aria-label'));
			const overlapping = [...element.querySelectorAll('.agent')].filter((agent) => {
				const meter = agent.querySelector('.ring, .billing')!.getBoundingClientRect();
				return [...agent.querySelectorAll('.name, .age')].some((node) => node.getBoundingClientRect().right >= meter.left);
			}).map((agent) => agent.getAttribute('aria-label'));
			return { overflow: element.scrollWidth > element.clientWidth, outside, misaligned, overlapping };
		})).toEqual({ overflow: false, outside: [], misaligned: [], overlapping: [] });
	}
	await page.setViewportSize(viewport);
	await page.emulateMedia({ colorScheme: 'dark' });
	await usage.screenshot({ path: test.info().outputPath('usage.png') });
	await checkNarrowBilling('monthly');
	update([...readings.slice(0, 3), { ...readings[3], consumption: { amount: 0, unit: 'credits' } }]);
	await expect(consumption).toHaveText('$0.00');
	await expect(copilot.locator('.credits')).toHaveText('0 credits');
	update([...readings.slice(0, 3), { ...readings[3], consumption: { amount: 75.5, unit: 'requests' } }]);
	await expect(consumption).toHaveText('75.5 requests');
	update([...readings.slice(0, 3), { agent: 'copilot', window: 'monthly', readAt: now }]);
	await expect(consumption).toHaveText('Usage unavailable');
	await expect(copilot.getByText('Billing usage not reported', { exact: true })).toBeVisible();
	await checkNarrowBilling('unavailable');
	update([...readings.slice(0, 3), { agent: 'copilot', window: 'monthly', readAt: now },
		{ agent: 'copilot', window: 'session', chatId: 'first', consumption: { amount: 12.5, unit: 'credits' }, readAt: now },
		{ agent: 'copilot', window: 'session', chatId: 'second', consumption: { amount: 2.5, unit: 'credits' }, partial: true, readAt: now }
	]);
	await expect(consumption).toHaveText('$0.15 reported');
	await expect(copilot.locator('.credits')).toHaveText('15 credits');
	await expect(copilot.locator('.period')).toHaveCount(0);
	await expect(copilot.locator('.cycle')).toHaveCount(0);
	await copilot.screenshot({ path: test.info().outputPath('copilot-cost.png') });
	await checkNarrowBilling('cost');
	update([...readings.slice(0, 3), { agent: 'copilot', window: 'session', chatId: 'first', consumption: { amount: 9, unit: 'requests' }, readAt: now }]);
	await expect(consumption).toHaveText('9 requests');
	await expect(copilot.locator('.period')).toHaveCount(0);
	await expect(copilot.getByText('Cost not reported', { exact: true })).toBeVisible();
	await checkNarrowBilling('requests');
	await page.clock.fastForward(minute);
	await expect(usage.locator('.countdown').first()).toHaveText('(in 3h 8m)');
	update([{ ...readings[0], resetsAt: now + 2 * minute }]);
	await expect(usage.locator('.countdown')).toHaveText('(in 1m)');
	await page.clock.fastForward(minute);
	await expect(usage.locator('.countdown')).toHaveText('(now)');
	await page.clock.fastForward(minute);
	await expect(usage.locator('.countdown')).toHaveText('(now)');
	update([]);
	await expect(usage).toBeHidden();
	await expect(usage.getByRole('group')).toHaveCount(0);
});
