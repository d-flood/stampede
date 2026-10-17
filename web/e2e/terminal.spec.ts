import { expect, test } from '@playwright/test';
import { openProject, screen } from './shell.ts';

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
