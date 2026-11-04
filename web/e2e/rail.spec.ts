import { expect, test } from './fixtures.ts';
import { openProject, screen } from './shell.ts';

test('Terminal dimensions follow project rail and sidebar toggles', { tag: '@desktop' }, async ({ page }) => {
	let resized: { cols: number; rows: number } | undefined;
	page.on('websocket', (socket) => socket.on('framereceived', ({ payload }) => {
		if (typeof payload !== 'string') return;
		const message = JSON.parse(payload);
		if (message.type === 'terminal.resized') resized = { cols: message.cols, rows: message.rows };
	}));
	await openProject(page, 'alpha');
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	const terminal = page.getByRole('tabpanel').locator('.terminal').filter({ visible: true });
	await expect(terminal).not.toHaveClass(/resizing/);
	for (const name of ['Expand project rail', 'Collapse project rail', 'Sidebar', 'Sidebar']) {
		const before = await page.evaluate(() => window.terminal!.cols);
		await page.getByRole('button', { name, exact: true }).click();
		await expect.poll(() => page.evaluate(() => window.terminal!.cols)).not.toBe(before);
		await expect(terminal).not.toHaveClass(/resizing/);
		await expect.poll(async () => {
			const size = await page.evaluate(() => ({ cols: window.terminal!.cols, rows: window.terminal!.rows }));
			return size.cols > 0 && size.rows > 0 && size.cols === resized?.cols && size.rows === resized?.rows;
		}).toBe(true);
	}
});
