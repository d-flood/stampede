import { expect, test } from './fixtures.ts';
import { mkdirSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

test('image files open as previews, fit the viewport, and refresh after disk changes', async ({ page }, { project }) => {
	const name = `images-${project.name}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(root);
	const images = await page.evaluate(() => {
		const canvas = document.createElement('canvas');
		const context = canvas.getContext('2d')!;
		return ['image/jpeg', 'image/webp', 'image/png'].map((mime, index) => {
			canvas.width = index === 1 ? 1600 : 2400;
			canvas.height = index === 1 ? 2400 : 1600;
			context.fillStyle = '#369';
			context.fillRect(0, 0, canvas.width, canvas.height);
			return canvas.toDataURL(mime).split(',')[1];
		});
	});
	const files = ['photo.JPEG', 'space & hash #.webp', 'image.png'];
	files.forEach((file, index) => writeFileSync(`${root}/${file}`, Buffer.from(images[index], 'base64')));
	writeFileSync(`${root}/broken.png`, 'broken image');
	await openProject(page, name);
	for (const file of files) {
		const sidebar = await showSidebar(page);
		await sidebar.getByRole('button', { name: file, exact: true }).click();
		const preview = page.getByRole('tabpanel').getByRole('img', { name: file, exact: true });
		await expect(preview).toBeVisible();
		await expect.poll(() => preview.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBe(file.endsWith('webp') ? 1600 : 2400);
		await expect(page.locator('.cm-editor:visible')).toHaveCount(0);
		await expect.poll(() => preview.evaluate((element) => {
			const box = element.getBoundingClientRect();
			const parent = element.parentElement!.getBoundingClientRect();
			return box.width <= parent.width && box.height <= parent.height
				&& box.x >= 0 && box.right <= innerWidth && box.bottom <= innerHeight;
		})).toBe(true);
	}
	const png = page.getByRole('tabpanel').getByRole('img', { name: 'image.png', exact: true });
	const original = await png.getAttribute('src');
	writeFileSync(`${root}/image.png`, Buffer.from(images[2], 'base64'));
	await expect(png).not.toHaveAttribute('src', original!);
	await expect.poll(() => png.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBe(2400);
	unlinkSync(`${root}/image.png`);
	await expect(page.getByRole('tabpanel').getByRole('alert')).toBeVisible();
	await expect(page.getByRole('tabpanel')).toContainText('deleted on disk');
	writeFileSync(`${root}/image.png`, Buffer.from(images[2], 'base64'));
	await expect(png).toBeVisible();
	await expect(page.getByRole('tabpanel')).not.toContainText('deleted on disk');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'broken.png', exact: true }).click();
	await expect(page.getByRole('tabpanel').getByRole('alert')).toHaveText('Unable to display image');
});

test('SVG toggles between rendered and editable source without losing edits or running scripts', { tag: '@desktop' }, async ({ page }, { project }) => {
	const name = `svg-${project.name}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(root);
	const svg = (width: number) => `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="100"><rect width="100%" height="100%" fill="#369"/><script>window.parent.svgExecuted = true</script></svg>`;
	writeFileSync(`${root}/drawing.SVG`, svg(200));
	await openProject(page, name);
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'drawing.SVG', exact: true }).click();
	const panel = page.getByRole('tabpanel');
	const preview = panel.getByRole('img', { name: 'drawing.SVG', exact: true });
	await expect(preview).toBeVisible();
	await expect(panel.getByRole('button', { name: 'Rendered', exact: true })).toHaveAttribute('aria-pressed', 'true');
	await expect.poll(() => preview.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBe(200);
	await panel.getByRole('button', { name: 'Source', exact: true }).click();
	const source = panel.locator('.cm-content');
	await expect(source).toBeVisible();
	await expect(source).toContainText('width="200"');
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await source.focus();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.insertText(svg(300));
	await panel.getByRole('button', { name: 'Rendered', exact: true }).click();
	await expect(source).toBeHidden();
	await expect.poll(() => preview.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBe(300);
	expect(readFileSync(`${root}/drawing.SVG`, 'utf8')).toBe(svg(200));
	await panel.getByRole('button', { name: 'Source', exact: true }).click();
	await expect(source).toContainText('width="300"');
	await source.focus();
	await page.keyboard.press('ControlOrMeta+z');
	await expect(source).toContainText('width="200"');
	await panel.getByRole('button', { name: 'Rendered', exact: true }).click();
	await expect.poll(() => preview.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBe(200);
	writeFileSync(`${root}/drawing.SVG`, svg(400));
	await expect.poll(() => preview.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBe(400);
	await panel.getByRole('button', { name: 'Source', exact: true }).click();
	await expect(source).toContainText('width="400"');
	await source.focus();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.insertText('invalid SVG');
	await panel.getByRole('button', { name: 'Rendered', exact: true }).click();
	await expect(panel.getByRole('alert')).toHaveText('Unable to display image');
	await panel.getByRole('button', { name: 'Source', exact: true }).click();
	await source.focus();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.insertText(svg(500));
	await panel.getByRole('button', { name: 'Rendered', exact: true }).click();
	await expect.poll(() => preview.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBe(500);
	await panel.getByRole('button', { name: 'Save', exact: true }).click();
	await expect.poll(() => readFileSync(`${root}/drawing.SVG`, 'utf8')).toBe(svg(500));
	expect(await page.evaluate(() => 'svgExecuted' in window)).toBe(false);
	await expect.poll(() => panel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
});
