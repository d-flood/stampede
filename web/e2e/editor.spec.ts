import { expect, test } from './fixtures.ts';
import { mkdirSync, readFileSync, unlinkSync, writeFileSync } from 'node:fs';
import { openProject, showSidebar } from './shell.ts';

test.use({ reducedMotion: 'reduce' });

test('Markdown previews preserve edits and undo, save, and refresh from disk', async ({ page }, { project }) => {
	const name = `markdown-${project.name}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(root);
	const original = '# Notes\n\nA **bold** paragraph.\n\n- First\n- Second\n';
	const edited = `${original}\n## Unsaved\n\n\`\`\`ts\n${'const example = true; '.repeat(40)}\n\`\`\`\n\n| Name | Value |\n| --- | --- |\n| Test | 42 |\n\n<script>window.markdownExecuted = true</script>\n`;
	writeFileSync(`${root}/notes.MD`, original);
	writeFileSync(`${root}/prose.markdown`, '# Prose\n');
	writeFileSync(`${root}/plain.txt`, 'Plain text');
	await openProject(page, name);
	let sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'notes.MD', exact: true }).click();
	const editor = page.getByRole('tabpanel');
	const source = editor.locator('.cm-content');
	const preview = editor.getByRole('region', { name: 'Markdown preview' });
	const editButton = editor.getByRole('button', { name: 'Edit', exact: true });
	const previewButton = editor.getByRole('button', { name: 'Preview', exact: true });
	await expect(editButton).toHaveAttribute('aria-pressed', 'true');
	await expect(source).toBeVisible();
	await source.focus();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.insertText(edited);
	await previewButton.click();
	await expect(previewButton).toHaveAttribute('aria-pressed', 'true');
	await expect(source).toBeHidden();
	await expect(preview.getByRole('heading', { name: 'Notes', exact: true })).toBeVisible();
	await expect(preview.locator('strong')).toHaveText('bold');
	await expect(preview.getByRole('listitem')).toHaveCount(2);
	await expect(preview.getByRole('heading', { name: 'Unsaved' })).toBeVisible();
	await expect(preview.locator('pre code')).toContainText('const example = true;');
	await expect(preview.getByRole('cell', { name: '42', exact: true })).toBeVisible();
	expect(readFileSync(`${root}/notes.MD`, 'utf8')).toBe(original);
	expect(await page.evaluate(() => 'markdownExecuted' in window)).toBe(false);
	await expect.poll(() => editor.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
	await editButton.click();
	await expect(source).toContainText('## Unsaved');
	await source.focus();
	await page.keyboard.press('ControlOrMeta+z');
	await expect(source).not.toContainText('Unsaved');
	await previewButton.click();
	await expect(preview.getByRole('heading', { name: 'Unsaved' })).toHaveCount(0);
	writeFileSync(`${root}/notes.MD`, '# Updated on disk\n');
	await expect(preview.getByRole('heading', { name: 'Updated on disk' })).toBeVisible();
	await editButton.click();
	await source.focus();
	await page.keyboard.press('ControlOrMeta+a');
	await page.keyboard.insertText('# Saved from preview\n');
	await previewButton.click();
	await editor.getByRole('button', { name: 'Save', exact: true }).click();
	await expect.poll(() => readFileSync(`${root}/notes.MD`, 'utf8')).toBe('# Saved from preview\n');
	sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'prose.markdown', exact: true }).click();
	await expect(editButton).toHaveAttribute('aria-pressed', 'true');
	await previewButton.click();
	await expect(preview.getByRole('heading', { name: 'Prose' })).toBeVisible();
	sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'plain.txt', exact: true }).click();
	await expect(previewButton).toHaveCount(0);
	sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'notes.MD', exact: true }).click();
	await expect(previewButton).toHaveAttribute('aria-pressed', 'true');
	await expect(preview.getByRole('heading', { name: 'Saved from preview' })).toBeVisible();
});

test('multiline selections keep editor text visible', { tag: '@desktop' }, async ({ page }) => {
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/selection`;
	mkdirSync(root);
	writeFileSync(`${root}/selection.txt`, 'Selected text must remain visible.\nAcross multiple lines.\nIncluding this final line.\n');
	await openProject(page, 'selection');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'selection.txt', exact: true }).click();
	const editor = page.locator('.editor:not([hidden])');
	const line = editor.locator('.cm-line').first();
	const foreground = await line.evaluate((element) => getComputedStyle(element).color);
	const paintedText = async () => {
		const screenshot = await line.screenshot();
		return page.evaluate(async ({ png, foreground }) => {
			const image = new Image();
			image.src = `data:image/png;base64,${png}`;
			await image.decode();
			const canvas = document.createElement('canvas');
			canvas.width = image.width;
			canvas.height = image.height;
			const context = canvas.getContext('2d')!;
			context.drawImage(image, 0, 0);
			const pixels = context.getImageData(0, 0, image.width, image.height).data;
			const rgb = foreground.match(/[\d.]+/g)!.slice(0, 3).map(Number);
			let count = 0;
			for (let offset = 0; offset < pixels.length; offset += 4) {
				if (rgb.every((value, channel) => Math.abs(pixels[offset + channel] - value) < 20)) count++;
			}
			return count;
		}, { png: screenshot.toString('base64'), foreground });
	};
	const unselected = await paintedText();
	expect(unselected).toBeGreaterThan(20);
	await editor.locator('.cm-content').focus();
	await page.keyboard.press('ControlOrMeta+a');
	await expect(editor.locator('.cm-selectionBackground')).not.toHaveCount(0);
	await expect.poll(paintedText).toBeGreaterThan(unselected * 0.7);
	await expect.poll(paintedText).toBeLessThan(unselected * 1.3);
});

test('line wrapping defaults to Markdown and toggles per editor without losing edits', { tag: '@desktop' }, async ({ page }) => {
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/wrapping`;
	const content = 'A long line of prose that should wrap inside the editor. '.repeat(80);
	mkdirSync(root);
	for (const file of ['notes.MD', 'prose.markdown', 'plain.txt']) writeFileSync(`${root}/${file}`, content);
	await openProject(page, 'wrapping');
	let sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'notes.MD', exact: true }).click();
	const editor = page.locator('.editor:not([hidden])');
	const toggle = editor.getByRole('button', { name: 'Line wrapping', exact: true });
	const line = editor.locator('.cm-line').first();
	const wrapped = () => line.evaluate((element) => {
		const height = element.getBoundingClientRect().height;
		const lineHeight = parseFloat(getComputedStyle(element).lineHeight);
		return height > lineHeight * 2;
	});
	await expect(toggle).toHaveAttribute('aria-pressed', 'true');
	await expect.poll(wrapped).toBe(true);
	await toggle.click();
	await expect(toggle).toHaveAttribute('aria-pressed', 'false');
	await expect.poll(wrapped).toBe(false);
	await editor.locator('.cm-content').focus();
	await page.keyboard.press('ControlOrMeta+End');
	await page.keyboard.type(' unsaved edit');
	await toggle.click();
	await expect.poll(wrapped).toBe(true);
	await expect(editor.locator('.cm-content')).toContainText('unsaved edit');
	await editor.locator('.cm-content').focus();
	await page.keyboard.press('ControlOrMeta+z');
	await expect(editor.locator('.cm-content')).not.toContainText('unsaved edit');
	await toggle.click();
	sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'plain.txt', exact: true }).click();
	await expect(toggle).toHaveAttribute('aria-pressed', 'false');
	await expect.poll(wrapped).toBe(false);
	await toggle.click();
	await expect.poll(wrapped).toBe(true);
	sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'notes.MD', exact: true }).click();
	await expect(toggle).toHaveAttribute('aria-pressed', 'false');
	await expect.poll(wrapped).toBe(false);
	sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: 'prose.markdown', exact: true }).click();
	await expect(toggle).toHaveAttribute('aria-pressed', 'true');
	await expect.poll(wrapped).toBe(true);
});

test('Editor markers and Agent file changes preserve explicit edits', async ({ page }, { project }) => {
	const file = `${project.name}.txt`;
	await openProject(page, 'alpha');
	const sidebar = await showSidebar(page);
	await sidebar.getByRole('button', { name: file, exact: true }).click();

	const line = page.locator('.cm-line', { hasText: 'two' });
	await line.click();
	await page.keyboard.press('End');
	await page.keyboard.type(' edited');
	const tab = page.getByRole('tablist', { name: 'Tabs' }).getByRole('tab', { name: file });
	await expect(tab.getByTitle('Unsaved changes')).toBeVisible();
	if (project.name === 'desktop') await page.keyboard.press('ControlOrMeta+s');
	else await page.getByRole('button', { name: 'Save' }).click();
	await expect(tab.getByTitle('Unsaved changes')).toHaveCount(0);

	const marker = page.locator('.cm-changeGutter .cm-change-modified');
	await expect(marker).toHaveCount(1);
	await expect
		.poll(async () => {
			const [mark, text] = await Promise.all([marker.boundingBox(), line.boundingBox()]);
			return mark && text && Math.abs(mark.y + mark.height / 2 - (text.y + text.height / 2));
		})
		.toBeLessThan(2);

	const files = await showSidebar(page);
	await files.getByRole('button', { name: `empty-${project.name}.ts`, exact: true }).click();
	await page.locator('.editor:not([hidden]) .cm-content').click();
	await page.keyboard.type('export const added = true;');
	await expect(page.locator('.editor:not([hidden]) .cm-changeGutter .cm-gutterElement[class*="cm-change-"]')).toHaveCount(1);

	const tree = await showSidebar(page);
	await tree.getByRole('button', { name: 'Show ignored files' }).click();
	await tree.getByRole('button', { name: 'ignored', exact: true }).click();
	await tree.getByTitle(`ignored/${project.name}.txt`, { exact: true }).click();
	const current = page.locator('.editor:not([hidden])');
	await expect(current.locator('.cm-content')).toContainText('initial ignored contents');
	const ignored = `${process.env.STAMPEDE_E2E_ROOT}/projects/alpha/ignored/${project.name}.txt`;
	writeFileSync(ignored, 'changed by Agent\n');
	await expect(current.locator('.cm-content')).toContainText('changed by Agent');
	await current.locator('.cm-content').click();
	await page.keyboard.press('End');
	await page.keyboard.type(' with unsaved edits');
	writeFileSync(ignored, 'changed again by Agent\n');
	await expect(current.getByRole('button', { name: 'Keep mine' })).toBeVisible();
	await current.getByRole('button', { name: 'Keep mine' }).click();
	await expect(current.locator('.cm-content')).toContainText('with unsaved edits');
	unlinkSync(ignored);
	await expect(current.getByRole('button', { name: 'Keep mine' })).toBeVisible();
	await current.getByRole('button', { name: 'Keep mine' }).click();
	if (project.name === 'desktop') {
		await current.locator('.cm-content').click();
		await page.keyboard.press('ControlOrMeta+s');
	}
	else await current.getByRole('button', { name: 'Save', exact: true }).click();
	await expect.poll(() => {
		try { return readFileSync(ignored, 'utf8'); } catch { return ''; }
	}).toContain('with unsaved edits');
});
