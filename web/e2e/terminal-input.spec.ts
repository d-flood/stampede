import { expect, test } from './fixtures.ts';
import type { Page } from '@playwright/test';
import { openProject, screen } from './shell.ts';


async function terminal(page: Page) {
	const input: string[] = [];
	let created = '';
	page.on('websocket', (socket) => {
		socket.on('framesent', ({ payload }) => {
			if (typeof payload !== 'string' && payload[0] === 2) input.push(payload.subarray(5).toString());
		});
		socket.on('framereceived', ({ payload }) => {
			if (typeof payload !== 'string') return;
			const message = JSON.parse(payload);
			if (message.type === 'terminal.created') created = message.terminal.id;
		});
	});
	await openProject(page, test.info().project.name === 'desktop' ? 'alpha' : 'beta');
	const tabs = page.getByRole('tablist', { name: 'Tabs', exact: true }).getByRole('tab');
	const count = await tabs.count();
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: 'Terminal' }).click();
	await expect(tabs).toHaveCount(count + 1);
	await expect.poll(() => created).not.toBe('');
	await expect(page.locator(`[data-tab="${created}"]`)).toHaveAttribute('aria-selected', 'true');
	await expect(page.locator('.terminal:visible canvas')).toBeVisible();
	await expect.poll(() => screen(page), { timeout: 15_000 }).toMatch(/\S/);
	await expect.poll(() => page.evaluate(() => window.terminal?.hasBracketedPaste())).toBe(true);
	return input;
}

test('terminal paste and Shift Enter send bracketed input from either focus target and keyboard layout', { tag: '@desktop' }, async ({ page, context }) => {
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

	await page.evaluate((text) => navigator.clipboard.writeText(text), text);
	input.length = 0;
	await page.keyboard.press('Control+V');
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify(`\x1b[200~${text}\x1b[201~`));

	input.length = 0;
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

	input.length = 0;
	await page.locator('.terminal:visible [contenteditable="true"]').focus();
	await page.keyboard.press('Shift+Enter');
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify('\x1b[200~\n\x1b[201~'));
	input.length = 0;
	await page.keyboard.press('Enter');
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify('\r'));
});

test('Paste and Newline buttons preserve input, and blocked clipboard access opens a focused Composer', { tag: '@phone' }, async ({ page, context }) => {
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	const input = await terminal(page);
	const text = 'pasted first line\npasted second line';
	await page.evaluate((text) => navigator.clipboard.writeText(text), text);
	await page.getByRole('button', { name: 'Paste', exact: true }).click();
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify(`\x1b[200~${text}\x1b[201~`));
	input.length = 0;
	await page.getByRole('button', { name: 'Newline', exact: true }).click();
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify('\x1b[200~\n\x1b[201~'));

	input.length = 0;
	await page.evaluate(() => {
		navigator.clipboard.readText = async () => { throw new DOMException('denied', 'NotAllowedError'); };
	});
	await page.getByRole('button', { name: 'Paste', exact: true }).click();
	const composer = page.getByRole('textbox', { name: 'Composer', exact: true });
	await expect(composer).toBeFocused();
	await expect(page.getByRole('status')).toBeVisible();
	await composer.fill('first line\nsecond line');
	await page.getByRole('button', { name: 'Insert', exact: true }).click();
	await expect.poll(() => JSON.stringify(input.join(''))).toBe(JSON.stringify('\x1b[200~first line\nsecond line\x1b[201~'));
});

test('terminal composer submits with desktop Enter and preserves mobile newlines', async ({ page }, { project }) => {
	const input = await terminal(page);
	await page.getByRole('button', { name: 'Composer', exact: true }).click();
	const composer = page.getByRole('textbox', { name: 'Composer', exact: true });
	await composer.fill('first line');
	input.length = 0;
	await composer.press('Shift+Enter');
	await composer.press('s');
	await expect(composer).toHaveValue('first line\ns');
	expect(input.join('')).toBe('');
	await composer.press('Enter');
	if (project.name === 'pixel') {
		await expect(composer).toHaveValue('first line\ns\n');
		expect(input.join('')).toBe('');
		await page.getByRole('button', { name: 'Send', exact: true }).click();
	}
	await expect(composer).toHaveValue('');
	const text = `first line\ns${project.name === 'pixel' ? '\n' : ''}`;
	await expect.poll(() => input.join('')).toBe(`\x1b[200~${text}\x1b[201~\r`);
});
