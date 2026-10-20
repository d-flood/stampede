import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { expect, test } from './fixtures.ts';
import { closeProject, openProject } from './shell.ts';

test('@ references a tracked file by its plain path', async ({ page }, { project }) => {
	const sent: string[] = [];
	page.on('websocket', (socket) => socket.on('framesent', ({ payload }) => {
		if (typeof payload !== 'string') return;
		const message = JSON.parse(payload);
		if (message.type === 'chat.send') sent.push(message.text);
	}));
	const name = `chat-files-${project.name}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	for (const dir of ['src/lib', 'dist']) mkdirSync(`${root}/${dir}`, { recursive: true });
	writeFileSync(`${root}/.gitignore`, 'dist/\n');
	writeFileSync(`${root}/src/lib/viewer.ts`, 'export const secret = "contents stay on disk";\n');
	writeFileSync(`${root}/dist/viewer.js`, '');
	writeFileSync(`${root}/README.md`, '');
	execFileSync('git', ['init', '-q', root]);
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	await expect(page.getByRole('tab', { name: /^Fake Claude/ }).getByTitle('done')).toBeVisible();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	const menu = panel.getByRole('region', { name: 'Files' });

	await composer.fill('Compare @vie');
	await expect(menu.getByRole('button')).toHaveText(['src/lib/viewer.ts']);
	if (project.name === 'desktop') await composer.press('Enter');
	else await menu.getByRole('button', { name: 'src/lib/viewer.ts' }).click();
	await expect(composer).toHaveValue('Compare src/lib/viewer.ts ');
	await expect(menu).toBeHidden();

	if (project.name === 'desktop') {
		await composer.press('@');
		await expect(menu).toHaveText('Type to find a file');
		await composer.press('Escape');
		await expect(menu).toBeHidden();
		await composer.press('Backspace');
	} else {
		await panel.getByRole('button', { name: 'Reference file', exact: true }).click();
		await expect(composer).toBeFocused();
		await expect(composer).toHaveValue('Compare src/lib/viewer.ts @');
		await expect(menu).toHaveText('Type to find a file');
	}
	await composer.pressSequentially(project.name === 'desktop' ? 'with @readme' : 'readme');
	await expect(menu.getByRole('button')).toHaveText(['README.md']);
	await composer.press('Home');
	await composer.press('End');
	await composer.press('ArrowLeft');
	await composer.press('ArrowLeft');
	await expect(menu.getByRole('button')).toHaveText(['README.md']);
	await menu.getByRole('button', { name: 'README.md' }).click();
	const expected = project.name === 'desktop' ? 'Compare src/lib/viewer.ts with README.md ' : 'Compare src/lib/viewer.ts README.md ';
	await expect(composer).toHaveValue(expected);
	await expect.poll(() => composer.evaluate((node: HTMLTextAreaElement) => node.selectionStart)).toBe(expected.length);

	await panel.getByRole('button', { name: 'Send', exact: true }).click();
	await expect.poll(() => sent).toEqual([expected]);
	await closeProject(page, name);
});

test('@ shows progress while the file list loads and recovers from a failed load', async ({ page }, { project }) => {
	const name = `chat-files-loading-${project.name}`;
	const root = `${process.env.STAMPEDE_E2E_ROOT}/projects/${name}`;
	mkdirSync(root, { recursive: true });
	writeFileSync(`${root}/README.md`, '');
	let fail = true;
	let release!: () => void;
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/api/projects/*/files', async (route) => {
		if (fail) return route.fulfill({ status: 500, body: 'walk failed' });
		await held;
		await route.continue();
	});
	await openProject(page, name);
	await page.getByRole('button', { name: 'New Tab', exact: true }).click();
	await page.getByRole('menuitem', { name: 'Fake Claude', exact: true }).click();
	const panel = page.getByRole('tabpanel').filter({ has: page.getByRole('textbox', { name: 'Composer' }) });
	const composer = panel.getByRole('textbox', { name: 'Composer' });
	const menu = panel.getByRole('region', { name: 'Files' });

	await composer.fill('@re');
	await expect(menu.getByRole('alert')).toHaveText('Could not read files');
	await composer.press('Escape');
	await expect(menu).toBeHidden();
	fail = false;
	await composer.fill('@');
	await expect(menu.getByRole('status')).toHaveText('Reading files…');
	await composer.pressSequentially('read');
	await expect(menu.getByRole('status')).toHaveText('Reading files…');
	release();
	await expect(menu.getByRole('button')).toHaveText(['README.md']);
	await expect(menu.getByRole('status')).toHaveCount(0);
	await closeProject(page, name);
});
