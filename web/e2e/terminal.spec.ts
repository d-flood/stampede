import { expect, test } from './fixtures.ts';
import { openProject, screen } from './shell.ts';

test('hidden scrolled Terminals stop painting and redraw when shown', async ({ page }) => {
	await page.addInitScript(() => {
		const paints = new WeakMap<object, number>();
		Object.assign(window, { terminalPaints: paints });
		const fillText = CanvasRenderingContext2D.prototype.fillText;
		CanvasRenderingContext2D.prototype.fillText = function (...args) {
			paints.set(this.canvas, (paints.get(this.canvas) ?? 0) + 1);
			fillText.apply(this, args);
		};
	});
	await openProject(page, 'alpha');
	for (let count = 0; count < 2; count++) {
		await page.getByRole('button', { name: 'New Tab', exact: true }).press('Enter');
		await page.getByRole('menuitem', { name: 'Terminal', exact: true }).click();
		await expect.poll(() => screen(page)).toMatch(/\S/);
		await page.locator('.terminal:visible [contenteditable="true"]').focus();
		await expect.poll(() => page.evaluate(() => !!window.terminal?.element?.getClientRects().length && window.terminal.hasBracketedPaste())).toBe(true);
		await page.keyboard.type(count === 0 ? 'seq 1 100; sleep 1; printf "hidden-output\\n"' : 'seq 1 100');
		await page.keyboard.press('Enter');
		await expect.poll(async () => /^100$/m.test(await screen(page))).toBe(true);
		await page.evaluate(() => window.terminal?.scrollLines(-50));
		await expect.poll(() => page.evaluate(() => window.terminal?.getViewportY())).toBeGreaterThan(0);
	}
	const hidden = page.locator('.terminal canvas').filter({ visible: false }).first();
	await expect(hidden).toHaveCount(1);
	await expect.poll(() => hidden.evaluate(async (canvas) => {
		const paints = (window as unknown as { terminalPaints: WeakMap<object, number> }).terminalPaints;
		const before = paints.get(canvas) ?? 0;
		await new Promise((resolve) => setTimeout(resolve, 100));
		return (paints.get(canvas) ?? 0) - before;
	}), { timeout: 2_000 }).toBe(0);
	const before = await hidden.evaluate((canvas) => (window as unknown as { terminalPaints: WeakMap<object, number> }).terminalPaints.get(canvas) ?? 0);
	await page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab').first().click();
	const visible = page.locator('.terminal:visible canvas');
	await expect.poll(() => visible.evaluate((canvas) => (window as unknown as { terminalPaints: WeakMap<object, number> }).terminalPaints.get(canvas) ?? 0)).toBeGreaterThan(before);
	await expect.poll(async () => /^hidden-output$/m.test(await screen(page))).toBe(true);
});

test('a Terminal runs echo hi and a deep link reveals its Tab', async ({ page }, { project }) => {
	let target: { id: string; owner: { project: string } } | undefined;
	page.on('websocket', (socket) => socket.on('framereceived', ({ payload }) => {
		if (typeof payload !== 'string') return;
		const message = JSON.parse(payload);
		if (message.type === 'terminal.created') target = message.terminal;
	}));
	const fixture = project.name === 'desktop' ? 'alpha' : 'beta';
	await openProject(page, fixture);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect(page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab')).toHaveAttribute('aria-selected', 'true');
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	await page.keyboard.type('echo hi');
	await page.keyboard.press('Enter');
	await expect.poll(() => screen(page)).toMatch(/^hi$/m);
	if (project.name === 'pixel') await page.getByRole('button', { name: 'Drawer', exact: true }).click();
	else await page.keyboard.press('Control+Backquote');
	const drawer = page.getByRole('region', { name: 'Drawer', includeHidden: true });
	await expect.poll(() => drawer.evaluate((element) => getComputedStyle(element).transform)).toBe('none');
	expect(target).toBeDefined();
	await page.goto(`/?project=${target!.owner.project}&tab=${target!.id}`);
	await expect(drawer).toHaveAttribute('inert', '');
	await expect.poll(() => drawer.evaluate((element) => element.getBoundingClientRect().bottom)).toBeLessThanOrEqual(0);
	await expect(page.locator(`[data-tab="${target!.id}"]`)).toHaveAttribute('aria-selected', 'true');
});
