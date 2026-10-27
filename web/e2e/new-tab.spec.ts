import { expect, test } from './fixtures.ts';
import { openProject } from './shell.ts';

test.use({ starterRigs: true });

for (const reducedMotion of ['no-preference', 'reduce'] as const) {
	test(`New Tab shows session loading and resizes with ${reducedMotion} motion`, { tag: reducedMotion === 'reduce' ? ['@desktop'] : [] }, async ({ page }) => {
		await page.emulateMedia({ reducedMotion });
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
		if (reducedMotion === 'reduce') {
			await expect.poll(() => menu.getByRole('status').evaluate((element) => element.getAnimations({ subtree: true }).filter((animation) => animation.playState === 'running').length)).toBe(0);
		}
		await expect(menu.getByRole('menuitem', { name: 'Terminal', exact: true })).toBeEnabled();
		await expect.poll(() => release !== undefined).toBe(true);
		await expect.poll(() => menu.evaluate((element) => element.getAnimations().length)).toBe(0);
		const before = (await menu.boundingBox())!.width;
		await menu.evaluate((element) => {
			element.addEventListener('transitionrun', () => {
				const widths: number[] = [];
				function sample() {
					widths.push(element.getBoundingClientRect().width);
					element.setAttribute('data-width-samples', JSON.stringify(widths));
					if (element.getAnimations().some((animation) => animation.playState === 'running')) requestAnimationFrame(sample);
				}
				sample();
			}, { once: true });
		});
		release!();
		await expect(menu.getByRole('status')).toHaveCount(0);
		await expect(menu.getByRole('menuitem', { name: /Continue investigating/ })).toBeVisible();
		await expect.poll(async () => (await menu.boundingBox())!.width).toBeGreaterThan(before + 20);
		await expect.poll(() => menu.evaluate((element) => element.getAnimations().length)).toBe(0);
		const after = (await menu.boundingBox())!.width;
		expect(after).toBeGreaterThan(before + 20);
		if (reducedMotion === 'no-preference') {
			const widths: number[] = JSON.parse((await menu.getAttribute('data-width-samples')) ?? '[]');
			expect(widths.some((width) => width > before + 1 && width < after - 1)).toBe(true);
		} else {
			await expect(menu).not.toHaveAttribute('data-width-samples');
		}
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
}
