import { expect, test, type Page } from '@playwright/test';

function screen(page: Page) {
	return page.evaluate(() => {
		const buffer = window.terminal!.buffer.active;
		const lines = Array.from({ length: buffer.length }, (_, y) => buffer.getLine(y));
		return lines.map((line) => line?.translateToString(true) ?? '').join('\n');
	});
}

test('a new Terminal runs echo hi', async ({ page }) => {
	await page.goto('/');
	await page.getByRole('button', { name: 'New Terminal' }).click();
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	await page.keyboard.type('echo hi');
	await page.keyboard.press('Enter');
	await expect.poll(() => screen(page)).toMatch(/^hi$/m);
});
