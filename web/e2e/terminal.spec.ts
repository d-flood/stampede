import { expect, test, type Page } from '@playwright/test';
import { openProject } from './shell.ts';

function screen(page: Page) {
	return page.evaluate(() => {
		const buffer = window.terminal?.buffer.active;
		if (!buffer) return '';
		const lines = Array.from({ length: buffer.length }, (_, y) => buffer.getLine(y));
		return lines.map((line) => line?.translateToString(true) ?? '').join('\n');
	});
}

test('a Terminal tab in a Project runs echo hi', async ({ page }, { project }) => {
	const fixture = project.name === 'desktop' ? 'alpha' : 'beta';
	await openProject(page, fixture);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect(page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab')).toHaveAttribute('aria-selected', 'true');
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	await page.keyboard.type('echo hi');
	await page.keyboard.press('Enter');
	await expect.poll(() => screen(page)).toMatch(/^hi$/m);
});
