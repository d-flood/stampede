import { expect, test, type Page } from '@playwright/test';
import { openProject, screen } from './shell.ts';

test.afterEach(async ({ page }) => {
	const tab = page.locator('[data-tab][aria-selected="true"]').first();
	if (!(await tab.count())) return;
	const id = await tab.getAttribute('data-tab');
	await tab.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Close', exact: true }).click();
	await expect(page.locator(`[data-tab="${id}"]`)).toHaveCount(0);
});

async function terminal(page: Page) {
	const input: string[] = [];
	page.on('websocket', (socket) => socket.on('framesent', ({ payload }) => {
		if (typeof payload !== 'string' && payload[0] === 2) input.push(payload.subarray(5).toString());
	}));
	await openProject(page, test.info().project.name === 'desktop' ? 'alpha' : 'beta');
	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab');
	const count = await tabs.count();
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect(tabs).toHaveCount(count + 1);
	await expect(tabs.last()).toHaveAttribute('aria-selected', 'true');
	await expect(page.locator('.terminal:visible canvas')).toBeVisible();
	await expect.poll(() => screen(page)).toMatch(/\S/);
	await expect.poll(() => page.evaluate(() => window.terminal?.hasBracketedPaste())).toBe(true);
	return input;
}

test('terminal paste protects multiline input from either focus target', async ({ page, context }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	const input = await terminal(page);
	const text = 'first line\nsecond line';
	for (const target of ['[contenteditable="true"]', 'textarea']) {
		input.length = 0;
		await page.locator(`.terminal:visible ${target}`).evaluate((element, text) => {
			(element as HTMLElement).focus();
			const clipboardData = new DataTransfer();
			clipboardData.setData('text/plain', text);
			element.dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true, cancelable: true }));
		}, text);
		await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify(`\x1b[200~${text}\x1b[201~`));
	}
	if (test.info().project.name !== 'desktop') return;

	await page.evaluate((text) => navigator.clipboard.writeText(text), text);
	input.length = 0;
	await page.keyboard.press('Control+V');
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify(`\x1b[200~${text}\x1b[201~`));
});

test('Shift Enter inserts a newline without submitting', async ({ page }) => {
	const input = await terminal(page);
	await page.locator('.terminal:visible [contenteditable="true"]').focus();
	await page.keyboard.press('Shift+Enter');
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify('\x1b[200~\n\x1b[201~'));
	input.length = 0;
	await page.keyboard.press('Enter');
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify('\r'));
});

test('Paste and Newline buttons preserve input without submitting', async ({ page, context }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	const input = await terminal(page);
	const text = 'pasted first line\npasted second line';
	await page.evaluate((text) => navigator.clipboard.writeText(text), text);
	await page.getByRole('button', { name: 'Paste', exact: true }).click();
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify(`\x1b[200~${text}\x1b[201~`));
	input.length = 0;
	await page.getByRole('button', { name: 'Newline', exact: true }).click();
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify('\x1b[200~\n\x1b[201~'));
});

test('blocked clipboard access opens a focused Composer for native paste', async ({ page }) => {
	const input = await terminal(page);
	await page.evaluate(() => {
		navigator.clipboard.readText = async () => { throw new DOMException('denied', 'NotAllowedError'); };
	});
	await page.getByRole('button', { name: 'Paste', exact: true }).click();
	const composer = page.getByRole('textbox', { name: 'Composer', exact: true });
	await expect(composer).toBeFocused();
	await expect(page.getByRole('status')).toHaveText('Clipboard access unavailable. Paste into Composer, then choose Insert.');
	await composer.fill('first line\nsecond line');
	await page.getByRole('button', { name: 'Insert', exact: true }).click();
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify('\x1b[200~first line\nsecond line\x1b[201~'));
});

test('paste shortcuts use the typed key across keyboard layouts', async ({ page }) => {
	const input = await terminal(page);
	const text = 'layout-safe paste';
	const allowed = await page.locator('.terminal:visible [contenteditable="true"]').evaluate((element, text) => {
		const key = new KeyboardEvent('keydown', { key: 'v', code: 'KeyD', ctrlKey: true, bubbles: true, cancelable: true });
		element.dispatchEvent(key);
		const clipboardData = new DataTransfer();
		clipboardData.setData('text/plain', text);
		element.dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true, cancelable: true }));
		return !key.defaultPrevented;
	}, text);
	expect(allowed).toBe(true);
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify(`\x1b[200~${text}\x1b[201~`));
});
