import { expect, test } from './fixtures.ts';
import { mkdirSync, unlinkSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import type { Locator, Page } from '@playwright/test';
import { closeProject, openProject } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="400" height="200"><rect width="400" height="200" fill="#369"/></svg>';

async function navigation(page: Page) {
	if (test.info().project.name === 'pixel') {
		const back = page.getByRole('button', { name: 'Previous screen' });
		while (!(await back.isDisabled())) await back.click();
		await expect.poll(() => page.locator('.screens').evaluate((element) => element.scrollLeft)).toBe(0);
	}
	return page.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'alpha', exact: true });
}

async function picker(page: Page, button: Locator) {
	await button.click({ button: 'right' });
	await page.getByRole('menuitem', { name: 'Change project icon…' }).click();
	const dialog = page.getByRole('dialog', { name: 'Change project icon', exact: true });
	await expect(dialog).toBeVisible();
	return dialog;
}

async function fits(element: Locator) {
	await expect.poll(() => element.evaluate((element) => {
		const bounds = element.getBoundingClientRect();
		const root = element.getRootNode();
		const container = root instanceof ShadowRoot ? root.host.getBoundingClientRect() : { left: 0, right: innerWidth, top: 0, bottom: innerHeight };
		return bounds.left >= 0 && bounds.right <= innerWidth && bounds.top >= 0 && bounds.bottom <= innerHeight
			&& bounds.left >= container.left && bounds.right <= container.right && bounds.top >= container.top && bounds.bottom <= container.bottom
			&& element.scrollWidth <= element.clientWidth;
	})).toBe(true);
}

test('emoji load on demand locally, support search and categories, and persist as project images', async ({ page, baseURL }) => {
	const requests: string[] = [];
	page.on('request', (request) => requests.push(request.url()));
	await openProject(page, 'alpha');
	let button = await navigation(page);
	let dialog = await picker(page, button);
	expect(requests.some((url) => /\/data[.-][^/]+\.json$/.test(url))).toBe(false);
	await dialog.getByRole('button', { name: 'Choose emoji…' }).click();
	const emojis = dialog.locator('emoji-picker');
	const catalog = await emojis.getAttribute('data-source');
	expect(catalog).toMatch(/\.json$/);
	await expect.poll(() => requests.includes(new URL(catalog!, baseURL).href)).toBe(true);
	expect(requests.every((url) => url.startsWith(baseURL!))).toBe(true);
	await emojis.getByRole('tab', { name: 'Animals and nature' }).click();
	await expect(emojis.getByRole('tab', { name: 'Animals and nature' })).toHaveAttribute('aria-selected', 'true');
	await fits(emojis);
	await fits(dialog);
	if (test.info().project.name === 'pixel') {
		for (const width of [320, 375, 448]) {
			await page.setViewportSize({ width, height: 998 });
			await fits(emojis);
			await fits(dialog);
			await fits(emojis.getByRole('tab', { name: 'Flags', exact: true }).locator('.nav-emoji'));
		}
	}
	await emojis.getByRole('combobox', { name: 'Search', exact: true }).fill('rocket');
	await emojis.getByRole('option', { name: /🚀.*rocket/ }).click();
	await expect(emojis).toHaveCount(0);
	const preview = dialog.getByRole('img', { name: 'Selected project icon' });
	await expect.poll(() => preview.evaluate((image: HTMLImageElement) => image.naturalWidth)).toBe(256);
	const pixels = await preview.evaluate((image: HTMLImageElement) => {
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = 256;
		const context = canvas.getContext('2d')!;
		context.drawImage(image, 0, 0);
		const data = context.getImageData(0, 0, 256, 256).data;
		let colored = 0;
		let edge = 0;
		for (let y = 0; y < 256; y++) for (let x = 0; x < 256; x++) {
			const offset = (y * 256 + x) * 4;
			if (!data[offset + 3]) continue;
			if (data[offset] !== data[offset + 1] || data[offset + 1] !== data[offset + 2]) colored++;
			if (x === 0 || y === 0 || x === 255 || y === 255) edge++;
		}
		return { colored, edge };
	});
	expect(pixels.colored).toBeGreaterThan(1000);
	expect(pixels.edge).toBe(0);
	await dialog.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect.poll(() => button.locator('img').evaluate((image: HTMLImageElement) => image.naturalWidth)).toBe(256);
	const source = await button.locator('img').getAttribute('src');
	await page.reload();
	button = await navigation(page);
	await expect(button.locator('img')).toHaveAttribute('src', source!);
	dialog = await picker(page, button);
	await dialog.getByRole('button', { name: 'Choose emoji…' }).click();
	const search = dialog.locator('emoji-picker').getByRole('combobox', { name: 'Search', exact: true });
	await search.fill('books');
	await expect(dialog.getByRole('option', { name: /📚.*books/ })).toBeVisible();
	await expect(search).toBeFocused();
	await search.press('Shift+Tab');
	await expect(dialog.getByRole('button', { name: 'Upload image…' })).toBeFocused();
	await dialog.getByRole('button', { name: 'Upload image…' }).press('Tab');
	await expect(search).toBeFocused();
	await search.press('ArrowDown');
	await expect(dialog.getByRole('option', { name: /📚.*books/ })).toHaveAttribute('aria-selected', 'true');
	await search.press('Enter');
	await expect(dialog.getByRole('img', { name: 'Selected project icon' })).toBeVisible();
	await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(button.locator('img')).toHaveAttribute('src', source!);
	dialog = await picker(page, button);
	await dialog.getByRole('button', { name: 'Use initials (Al)' }).click();
	await expect(button.locator('img')).toHaveCount(0);
	await expect(button).toContainText('Al');
});

test('a failed emoji catalog leaves image choices available', async ({ page }) => {
	await page.route(/\/data[.-][^/]+\.json$/, (route) => route.abort());
	await openProject(page, 'alpha');
	const button = await navigation(page);
	const dialog = await picker(page, button);
	await dialog.getByRole('button', { name: 'Choose emoji…' }).click();
	await expect(dialog.getByRole('alert')).toContainText('Could not load emoji');
	await expect(dialog.getByRole('button', { name: 'Save', exact: true })).toBeDisabled();
	await dialog.getByRole('button', { name: 'Choose from project…' }).click();
	await expect(dialog.locator('emoji-picker')).toHaveCount(0);
	await expect(dialog.getByRole('region', { name: 'Project images' })).toBeVisible();
	await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(button).toContainText('Al');
});

test('project images are previewed, copied, restored after reopening, and removable', async ({ page }) => {
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/alpha`;
	mkdirSync(`${root}/assets`);
	writeFileSync(`${root}/assets/brand & mark.svg`, svg);
	writeFileSync(`${root}/assets/broken.png`, 'broken');
	await openProject(page, 'alpha');
	let button = await navigation(page);
	await expect(button).toContainText('Al');
	let dialog = await picker(page, button);
	await dialog.getByRole('button', { name: 'Choose from project…' }).click();
	await dialog.getByRole('button', { name: 'assets', exact: true }).click();
	await dialog.getByRole('button', { name: 'broken.png', exact: true }).click();
	await expect(dialog.getByRole('alert')).toContainText('Unable to display');
	await expect(dialog.getByRole('button', { name: 'Save', exact: true })).toBeDisabled();
	await dialog.getByRole('button', { name: 'brand & mark.svg', exact: true }).click();
	const preview = dialog.getByRole('img', { name: 'Selected project icon' });
	await expect.poll(() => preview.evaluate((image: HTMLImageElement) => image.naturalWidth)).toBe(256);
	await expect.poll(() => preview.evaluate((image: HTMLImageElement) => image.naturalHeight)).toBe(128);
	await fits(dialog);
	await dialog.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect.poll(() => button.locator('img').evaluate((image: HTMLImageElement) => image.naturalWidth)).toBe(256);
	await expect(button.locator('img')).toHaveCSS('object-fit', 'contain');
	await fits(button);
	const source = await button.locator('img').getAttribute('src');
	unlinkSync(`${root}/assets/brand & mark.svg`);
	await closeProject(page, 'alpha');
	await openProject(page, 'alpha');
	button = await navigation(page);
	await expect(button.locator('img')).toHaveAttribute('src', source!);
	await expect.poll(() => button.locator('img').evaluate((image: HTMLImageElement) => image.naturalWidth)).toBe(256);
	dialog = await picker(page, button);
	await dialog.getByRole('button', { name: 'Use initials (Al)' }).click();
	await expect(dialog).toBeHidden();
	await expect(button.locator('img')).toHaveCount(0);
	await expect(button).toContainText('Al');
});

test('website URL paths discover icons and failed requests preserve the saved icon', { tag: '@desktop' }, async ({ page }) => {
	const requested: string[] = [];
	const png = Buffer.from(await page.evaluate(() => {
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = 32;
		canvas.getContext('2d')!.fillRect(0, 0, 32, 32);
		return canvas.toDataURL('image/png').split(',')[1];
	}), 'base64');
	const header = Buffer.alloc(22);
	header.writeUInt16LE(1, 2);
	header.writeUInt16LE(1, 4);
	header[6] = header[7] = 32;
	header.writeUInt16LE(1, 10);
	header.writeUInt16LE(32, 12);
	header.writeUInt32LE(png.length, 14);
	header.writeUInt32LE(22, 18);
	const ico = Buffer.concat([header, png]);
	const site = createServer((request, response) => {
		requested.push(request.url!);
		if (request.url === '/project/?version=2') {
			response.setHeader('Content-Type', 'text/html');
			response.end('<link rel="icon" href="assets/brand.svg">');
		} else if (request.url === '/project/assets/brand.svg') {
			response.setHeader('Content-Type', 'image/svg+xml');
			response.end(svg);
		} else if (request.url === '/ico/') {
			response.end('<title>ICO fallback</title>');
		} else if (request.url === '/ico/favicon.ico') {
			response.setHeader('Content-Type', 'image/x-icon');
			response.end(ico);
		} else { response.writeHead(404); response.end(); }
	});
	await new Promise<void>((resolve) => site.listen(0, '127.0.0.1', resolve));
	try {
		const address = site.address() as { port: number };
		await openProject(page, 'alpha');
		const button = await navigation(page);
		let dialog = await picker(page, button);
		await dialog.getByLabel('Website URL').fill(`http://127.0.0.1:${address.port}/project/?version=2`);
		await dialog.getByRole('button', { name: 'Fetch icon' }).click();
		await expect(dialog.getByRole('img', { name: 'Selected project icon' })).toBeVisible();
		expect(requested).toEqual(['/project/?version=2', '/project/assets/brand.svg']);
		await dialog.getByRole('button', { name: 'Save', exact: true }).click();
		await expect(dialog).toBeHidden();
		dialog = await picker(page, button);
		await dialog.getByLabel('Website URL').fill(`http://127.0.0.1:${address.port}/ico/`);
		await dialog.getByRole('button', { name: 'Fetch icon' }).click();
		await expect.poll(() => dialog.getByRole('img', { name: 'Selected project icon' }).evaluate((image: HTMLImageElement) => image.naturalWidth)).toBe(32);
		await dialog.getByRole('button', { name: 'Save', exact: true }).click();
		await expect(dialog).toBeHidden();
		const source = await button.locator('img').getAttribute('src');
		dialog = await picker(page, button);
		await dialog.getByLabel('Website URL').fill(`http://127.0.0.1:${address.port}/missing/`);
		await dialog.getByRole('button', { name: 'Fetch icon' }).click();
		await expect(dialog.getByRole('alert')).toBeVisible();
		await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
		await expect(button.locator('img')).toHaveAttribute('src', source!);
		await page.route(`**${source}`, (route) => route.fulfill({ status: 404 }));
		await page.reload();
		await navigation(page);
		await expect(button.locator('img')).toHaveCount(0);
		await expect(button).toContainText('Al');
	} finally { await new Promise<void>((resolve, reject) => site.close((error) => error ? reject(error) : resolve())); }
});

test('device uploads can be previewed and cancelled without changing the icon', { tag: '@desktop' }, async ({ page }) => {
	await openProject(page, 'alpha');
	const button = await navigation(page);
	const dialog = await picker(page, button);
	const choosing = page.waitForEvent('filechooser');
	await dialog.getByRole('button', { name: 'Upload image…' }).click();
	await (await choosing).setFiles({ name: 'brand.svg', mimeType: 'image/svg+xml', buffer: Buffer.from(svg) });
	await expect(dialog.getByRole('img', { name: 'Selected project icon' })).toBeVisible();
	await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect(button.locator('img')).toHaveCount(0);
	await expect(button).toContainText('Al');
});
