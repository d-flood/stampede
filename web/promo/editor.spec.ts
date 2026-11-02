import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { expect, test } from '../e2e/fixtures.ts';
import { Director } from './record.ts';
import { configure, project } from './stage.ts';

const format = `const minute = 60_000;
const hour = 60 * minute;
const day = 24 * hour;

export function ago(updated: number, now = Date.now()) {
	const elapsed = now - updated;
	if (elapsed < minute) return 'just now';
	if (elapsed < hour) return \`\${Math.floor(elapsed / minute)}m ago\`;
	if (elapsed < day) return \`\${Math.floor(elapsed / hour)}h ago\`;
	return new Date(updated).toLocaleDateString();
}
`;

// Shares the Rust tests' download cache, so the server installs TypeScript without the network once it has run.
process.env.STAMPEDE_LSP_CACHE ??= join(dirname(dirname(process.env.STAMPEDE_E2E_BINARY!)), 'tmp/lsp-cache');

test('the editor and its language server', async ({ browser, baseURL }) => {
	test.setTimeout(180_000);
	const root = process.env.STAMPEDE_E2E_ROOT!;
	const path = project(root, 'tidepool');
	writeFileSync(join(path, 'src/lib/format.ts'), format);
	execFileSync('git', ['-C', path, 'add', 'src/lib/format.ts']);
	execFileSync('git', ['-C', path, 'commit', '-q', '-m', 'Show when notes changed']);
	mkdirSync(join(path, '.svelte-kit'));
	writeFileSync(join(path, '.svelte-kit/tsconfig.json'), JSON.stringify({ compilerOptions: { target: 'esnext', module: 'esnext', lib: ['esnext', 'dom'], noEmit: true }, include: ['../src/**/*.ts'] }));

	const desktop = await (await browser.newContext({ baseURL, viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2, colorScheme: 'dark' })).newPage();
	await configure(desktop, root, []);
	await desktop.goto('/');
	await desktop.getByRole('navigation', { name: 'Projects' }).getByRole('button', { name: 'Open Project' }).click();
	await desktop.getByRole('option', { name: /\/tidepool$/ }).click();
	await expect(desktop.getByRole('dialog')).toBeHidden();
	const sidebar = desktop.getByRole('complementary', { name: 'Sidebar' });
	if (!(await sidebar.isVisible())) await desktop.getByRole('button', { name: 'Sidebar' }).click();
	for (const name of ['src', 'lib']) await sidebar.getByRole('button', { name, exact: true }).click();
	await sidebar.getByRole('button', { name: 'format.ts', exact: true }).click();
	const tab = desktop.getByRole('tablist', { name: 'Tabs' }).getByRole('tab', { name: 'format.ts' });
	await expect(tab).toHaveAttribute('aria-selected', 'true');
	const editor = desktop.locator('.editor:not([hidden])');
	const content = editor.locator('.cm-content');
	const toggle = editor.getByRole('switch', { name: 'Language server' });
	await expect(toggle).toHaveAttribute('aria-checked', 'false');
	await desktop.mouse.move(1000, 600);

	const director = new Director('editor');
	await director.record('desktop', desktop);
	await director.hold(1);
	director.mark('desktop', 'start');
	const [gutter, last, right] = await Promise.all([editor.locator('.cm-gutters'), editor.locator('.cm-line').last(), toggle].map(async (locator) => (await locator.boundingBox())!));
	director.events.push({ t: Date.now() / 1000, device: 'desktop', type: 'focus', name: 'code', box: { x: gutter.x, y: right.y - 6, width: right.x + right.width - gutter.x, height: last.y + last.height - right.y + 6 } });
	const anchor = editor.locator('.cm-line', { hasText: 'elapsed < day' });
	await director.click('desktop', anchor);
	await desktop.keyboard.press('End');
	await desktop.keyboard.press('Enter');
	await desktop.keyboard.press('Shift+Home');
	await desktop.keyboard.insertText('\t');
	await director.type('desktop', content, "if (elapsd < 2 * day) return 'yesterday';");
	await desktop.keyboard.press('Escape');
	const added = editor.locator('.cm-line', { hasText: 'yesterday' });
	await expect(editor.locator('.cm-changeGutter .cm-gutterElement[class*="cm-change-"]')).not.toHaveCount(0);
	await expect(tab.getByTitle('Unsaved changes')).toBeVisible();
	await director.focus('desktop', 'edit', editor.locator('.cm-scroller'));
	await director.hold(1.4);
	director.mark('desktop', 'typed');

	await director.click('desktop', toggle);
	await director.focus('desktop', 'switch', toggle);
	await expect(toggle).toHaveText(/LSP on/, { timeout: 60_000 });
	director.mark('desktop', 'on');
	const error = editor.locator('.cm-lintRange-error');
	await expect(error).toHaveCount(1, { timeout: 30_000 });
	await director.focus('desktop', 'error', added);
	director.mark('desktop', 'diagnosed');
	await director.hold(0.8);
	await director.point('desktop', error);
	await error.hover();
	const tooltip = desktop.locator('.cm-tooltip-lint, .cm-tooltip-hover').first();
	await expect(tooltip).toBeVisible();
	await director.focus('desktop', 'tooltip', tooltip);
	await director.hold(2);

	await director.point('desktop', error, 0.4);
	director.events.push({ t: Date.now() / 1000, device: 'desktop', type: 'click' });
	await error.dblclick();
	await director.type('desktop', content, 'ela');
	const completion = desktop.locator('.cm-tooltip-autocomplete');
	await expect(completion.getByRole('option', { name: /^elapsed/ })).toBeVisible();
	await director.focus('desktop', 'completion', completion);
	await director.hold(1);
	director.keys('desktop', 'Enter');
	await desktop.keyboard.press('Enter');
	await expect(added).toContainText("if (elapsed < 2 * day) return 'yesterday';");
	await expect(error).toHaveCount(0);
	director.mark('desktop', 'fixed');
	await director.hold(0.8);
	director.keys('desktop', 'Ctrl + S');
	await desktop.keyboard.press('Control+s');
	await expect(tab.getByTitle('Unsaved changes')).toHaveCount(0);
	await director.rest('desktop', desktop, 1000, 600);
	await director.hold(1.6);
	director.mark('desktop', 'end');
	await director.finish();
});
