import type { Locator, Page } from '@playwright/test';
import { mkdirSync, readFileSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { closeProject, openProject } from './shell.ts';

const png = readFileSync(new URL('../static/icons/icon-192.png', import.meta.url)).toString('base64');

async function launch(page: Page, name: string, rig = 'Fake Claude') {
	mkdirSync(`${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`, { recursive: true });
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab' }).click();
	await page.getByRole('menuitem', { name: rig, exact: true }).click();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	return { panel, composer: panel.getByRole('textbox', { name: 'Composer' }) };
}

async function paste(composer: Locator, files: { name: string; type?: string; size?: number }[], text = '') {
	return composer.evaluate((element, { files, text, png }) => {
		const clipboardData = new DataTransfer();
		if (text) clipboardData.setData('text/plain', text);
		for (const file of files) {
			const data = file.size === undefined ? Uint8Array.from(atob(png), (char) => char.charCodeAt(0)) : new Uint8Array(file.size);
			clipboardData.items.add(new File([data], file.name, { type: file.type ?? 'image/png' }));
		}
		return element.dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true, cancelable: true }));
	}, { files, text, png });
}

for (const agent of ['Claude', 'Codex']) {
	test(`${agent} sends a pasted image and preserves the draft`, { tag: agent === 'Codex' ? '@desktop' : [] }, async ({ page, context }, { project }) => {
		const name = `paste-${agent}-${project.name}`;
		const { panel, composer } = await launch(page, name, `Fake ${agent}`);
		await composer.fill('Look at this');
		if (project.name === 'desktop') {
			await context.grantPermissions(['clipboard-read', 'clipboard-write']);
			await page.evaluate(async (png) => {
				const data = Uint8Array.from(atob(png), (char) => char.charCodeAt(0));
				await navigator.clipboard.write([new ClipboardItem({ 'image/png': new Blob([data], { type: 'image/png' }) })]);
			}, png);
			await composer.press('Control+V');
		} else {
			expect(await paste(composer, [{ name: 'screenshot.png' }], 'image URL')).toBe(false);
		}
		const image = panel.getByLabel('Attachments', { exact: true }).locator('img');
		await expect(image).toBeVisible();
		await expect.poll(() => image.evaluate((node: HTMLImageElement) => node.complete && node.naturalWidth > 0)).toBe(true);
		await expect(composer).toHaveValue('Look at this');
		await expect.poll(() => panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
		await panel.getByRole('button', { name: 'Send', exact: true }).click();
		await expect(panel.getByLabel('Attachments', { exact: true })).toHaveCount(0);
		await expect(panel.getByText('1 image', { exact: true })).toBeVisible();
		const sentImage = panel.locator('.transcript').getByRole('img', { name: 'Attached image 1', exact: true });
		await expect(sentImage).toBeVisible();
		await expect.poll(() => sentImage.evaluate((node: HTMLImageElement) => node.complete && node.naturalWidth > 0)).toBe(true);
		await expect(panel.getByRole('link', { name: 'Open attached image 1', exact: true })).toHaveAttribute('href', /^\/api\/media\//);
		await expect(composer).toHaveValue('');
		await page.reload();
		await expect(sentImage).toBeVisible();
		await expect.poll(() => sentImage.evaluate((node: HTMLImageElement) => node.complete && node.naturalWidth > 0)).toBe(true);
		await closeProject(page, name);
	});
}

test('image-only messages show every attachment within the transcript bounds', async ({ page }, { project }) => {
	const name = `image-only-${project.name}`;
	const { panel, composer } = await launch(page, name);
	const files = await page.evaluate(() => [[1600, 400], [400, 1600]].map(([width, height], index) => {
		const canvas = document.createElement('canvas');
		canvas.width = width;
		canvas.height = height;
		const context = canvas.getContext('2d')!;
		context.fillStyle = '#4338ca';
		context.fillRect(0, 0, width, height);
		return { name: `image-${index}.png`, data: canvas.toDataURL().split(',')[1] };
	}));
	await panel.getByLabel('Attach images', { exact: true }).setInputFiles(files.map(({ name, data }) => ({ name, mimeType: 'image/png', buffer: Buffer.from(data, 'base64') })));
	await expect(panel.getByRole('button', { name: 'Send', exact: true })).toBeEnabled();
	if (project.name === 'desktop') await composer.press('Enter');
	else await panel.getByRole('button', { name: 'Send', exact: true }).click();
	const transcript = panel.locator('.transcript');
	const images = transcript.getByRole('img');
	await expect(images).toHaveCount(2);
	await expect(transcript.getByText('2 images', { exact: true })).toBeVisible();
	await expect(transcript.locator('.user')).toHaveCount(0);
	for (const image of await images.all()) {
		await image.scrollIntoViewIfNeeded();
		await expect.poll(() => image.evaluate((node: HTMLImageElement) => node.complete && node.naturalWidth > 0)).toBe(true);
		await expect.poll(() => image.evaluate((node) => node.getBoundingClientRect().width <= node.closest('.transcript')!.clientWidth)).toBe(true);
		await expect.poll(() => image.evaluate((node) => node.getBoundingClientRect().height <= innerHeight * 0.5 + 1)).toBe(true);
		await expect.poll(() => image.evaluate((node: HTMLImageElement) => Math.abs(node.width / node.height - node.naturalWidth / node.naturalHeight))).toBeLessThan(0.02);
	}
	await expect.poll(() => panel.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
	await closeProject(page, name);
});

test('text paste stays native and pasted images can be removed', { tag: '@desktop' }, async ({ page, context }, { project }) => {
	const name = `paste-text-${project.name}`;
	const { panel, composer } = await launch(page, name);
	expect(await paste(composer, [], 'plain text')).toBe(true);
	expect(await paste(composer, [{ name: 'notes.txt', type: 'text/plain' }])).toBe(true);
	await expect(panel.getByLabel('Attachments', { exact: true })).toHaveCount(0);
	await context.grantPermissions(['clipboard-read', 'clipboard-write']);
	await page.evaluate(() => navigator.clipboard.writeText('plain text\nsecond line'));
	await composer.press('Control+V');
	await expect(composer).toHaveValue('plain text\nsecond line');
	await paste(composer, [{ name: 'remove.png' }]);
	await expect(panel.getByRole('img', { name: 'remove.png' })).toBeVisible();
	await panel.getByRole('button', { name: 'Remove remove.png', exact: true }).click();
	await expect(panel.getByLabel('Attachments', { exact: true })).toHaveCount(0);
	await closeProject(page, name);
});

test('pasted images share picker limits and reject overlapping reads', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `paste-limits-${project.name}`;
	const { panel, composer } = await launch(page, name);
	const alert = panel.getByRole('alert');
	await paste(composer, [{ name: 'vector.svg', type: 'image/svg+xml' }]);
	await expect(alert).toHaveText('Choose PNG, JPEG, GIF or WebP images.');
	await paste(composer, [{ name: 'large.png', size: 5 * 1024 * 1024 + 1 }]);
	await expect(alert).toHaveText('Each image must be 5 MB or smaller.');
	await paste(composer, Array.from({ length: 5 }, (_, index) => ({ name: `${index}.png`, size: 4 * 1024 * 1024 + 1 })));
	await expect(alert).toHaveText('Keep attachments under 20 MB per message.');
	await expect(panel.getByLabel('Attachments', { exact: true })).toHaveCount(0);
	await panel.getByLabel('Attach images', { exact: true }).setInputFiles(new URL('../static/icons/icon-192.png', import.meta.url).pathname);
	await expect(panel.getByRole('img', { name: 'icon-192.png' })).toBeVisible();
	await composer.evaluate((element, png) => {
		const clipboardData = new DataTransfer();
		const data = Uint8Array.from(atob(png), (char) => char.charCodeAt(0));
		for (let index = 0; index < 10; index++) clipboardData.items.add(new File([data], `${index}.png`, { type: 'image/png' }));
		for (let index = 0; index < 2; index++) element.dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true, cancelable: true }));
	}, png);
	const images = panel.getByLabel('Attachments', { exact: true }).locator('img');
	await expect(images).toHaveCount(11);
	await expect(panel.getByRole('button', { name: 'Attach image', exact: true })).toBeEnabled();
	await paste(composer, Array.from({ length: 10 }, (_, index) => ({ name: `extra-${index}.png` })));
	await expect(alert).toHaveText('Attach up to 20 images.');
	await expect(images).toHaveCount(11);
	await closeProject(page, name);
});

test('image paste respects pending approval and offline restrictions', { tag: '@desktop' }, async ({ page, context }, { project }) => {
	const name = `paste-blocked-${project.name}`;
	const { panel, composer } = await launch(page, name, 'Claude Approval');
	await composer.fill('Run the shell command `mkdir -p out && touch out/approved.txt` exactly as written, then reply with the single word Done.');
	await panel.getByRole('button', { name: 'Send', exact: true }).click();
	const card = panel.getByRole('region', { name: 'Approval' });
	await expect(card).toBeVisible();
	await paste(composer, [{ name: 'blocked.png' }]);
	await expect(panel.getByLabel('Attachments', { exact: true })).toHaveCount(0);
	await card.getByRole('button', { name: 'Allow', exact: true }).click();
	await expect(card).toHaveCount(0);
	await context.setOffline(true);
	await expect(panel.getByRole('button', { name: 'Attach image', exact: true })).toBeDisabled();
	await paste(composer, [{ name: 'offline.png' }]);
	await expect(panel.getByLabel('Attachments', { exact: true })).toHaveCount(0);
	await context.setOffline(false);
	await closeProject(page, name);
});

test('images dropped anywhere on a Chat attach without leaving the page', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `drop-${project.name}`;
	const { panel } = await launch(page, name);
	const allowed = await panel.locator('.transcript').first().evaluate((element, png) => {
		const dataTransfer = new DataTransfer();
		dataTransfer.items.add(new File([Uint8Array.from(atob(png), (char) => char.charCodeAt(0))], 'dropped.png', { type: 'image/png' }));
		return ['dragover', 'drop'].map((type) => element.dispatchEvent(new DragEvent(type, { dataTransfer, bubbles: true, cancelable: true })));
	}, png);
	expect(allowed).toEqual([false, false]);
	await expect(panel.getByRole('img', { name: 'dropped.png' })).toBeVisible();
	await closeProject(page, name);
});

test('mobile attachment controls leave the previews unobscured', { tag: '@phone' }, async ({ page }, { project }) => {
	const name = `attachment-controls-${project.name}`;
	const { panel, composer } = await launch(page, name);
	await paste(composer, [{ name: 'first.png' }, { name: 'second.png' }]);
	const attachments = panel.getByLabel('Attachments', { exact: true });
	await expect(attachments.locator('img')).toHaveCount(2);
	for (const width of [448, 320]) {
		await page.setViewportSize({ width, height: 998 });
		for (const name of ['first.png', 'second.png']) {
			const image = attachments.getByRole('img', { name, exact: true });
			const remove = attachments.getByRole('button', { name: `Remove ${name}`, exact: true });
			await expect.poll(async () => {
				const [preview, button] = await Promise.all([image.boundingBox(), remove.boundingBox()]);
				if (!preview || !button) return -1;
				return Math.max(0, Math.min(preview.x + preview.width, button.x + button.width) - Math.max(preview.x, button.x))
					* Math.max(0, Math.min(preview.y + preview.height, button.y + button.height) - Math.max(preview.y, button.y));
			}).toBe(0);
			await expect.poll(async () => {
				const button = await remove.boundingBox();
				return button && button.width >= 48 && button.height >= 48;
			}).toBe(true);
		}
		await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	}
	await attachments.getByRole('button', { name: 'Remove first.png', exact: true }).click();
	await expect(attachments.getByRole('img', { name: 'first.png', exact: true })).toHaveCount(0);
	await expect(attachments.getByRole('img', { name: 'second.png', exact: true })).toBeVisible();
	await closeProject(page, name);
});
