import Code from '#lib/chat/Code.svelte';
import Edit from '#lib/chat/Edit.svelte';
import RequestCard from '#lib/chat/RequestCard.svelte';
import type { Request } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

const patch = ['--- a/example.ts', '+++ b/example.ts', '@@ -1,3 +1,3 @@', '-export const value = "old";', '+export const value = "<img src=x onerror=alert(1)>";', '-/* removed comment', '+export const enabled = true;', ' const number = 42;', '@@ -10,3 +10,3 @@', ' /* shared comment', '- old text', '+ new text', '  */', ''].join('\n');

async function colored(root: Element, text: string, variable: string) {
	await expect.poll(() => {
		const token = [...root.querySelectorAll('span[class]')].find((element) => element.textContent === text && !element.classList.contains('syntax'));
		if (!token) return false;
		const probe = document.body.appendChild(document.createElement('span'));
		probe.style.color = `var(${variable})`;
		const expected = getComputedStyle(probe).color;
		probe.remove();
		return getComputedStyle(token).color === expected;
	}).toBe(true);
}

test.each([
	['pnpm -C web check', [['pnpm', '--ansi-4'], ['-C', '--ansi-3']]],
	["sed -n '1,125p' file.ts", [['sed', '--ansi-4'], ["'1,125p'", '--ansi-2']]],
	["ps -eo pid,ppid,comm | rg 'pnpm|node|zsh'", [['ps', '--ansi-4'], ['rg', '--ansi-4'], ['|', '--muted-fg']]],
	['cat file.ts | head -42; git diff', [['head', '--ansi-4'], ['git', '--ansi-4']]],
	["python3 -c 'import json; print(json.dumps(1))' | head", [['import', '--ansi-5'], ['head', '--ansi-4']]],
	["python3 -I - <<'EOF'\nfor line in open('x'):\n    print(line)\nEOF\necho done", [['for', '--ansi-5'], ['echo', '--ansi-4']]]
] as const)('highlights shell and embedded Python in %s', async (code, tokens) => {
	const view = await render(Code, { code, path: 'command.sh' });
	for (const [text, variable] of tokens) await colored(view.container, text, variable);
});

test('updates highlighted text when command props change', async () => {
	const view = await render(Code, { code: 'printf \'%s\\n\' "$HOME"', path: 'command.sh' });
	await colored(view.container, "'%s\\n'", '--ansi-2');
	await view.rerender({ code: 'printf "updated"' });
	await colored(view.container, '"updated"', '--ansi-2');
});

test('highlights JSON tool arguments', async () => {
	const view = await render(Code, { code: '{"enabled":true,"count":42}', path: 'arguments.json' });
	await colored(view.container, 'true', '--ansi-3');
});

test('diffs highlight both versions, preserve hunk context, and escape HTML', async () => {
	const ondiff = vi.fn();
	const view = await render(Edit, { item: { kind: 'edit', path: 'example.ts', diff: patch, added: 3, removed: 3 }, ondiff });
	const summary = view.container.querySelector('summary');
	if (summary) await page.elementLocator(summary).click();
	const added = view.container.querySelector('.code-line.added')!;
	await colored(view.container, 'const', '--ansi-5');
	await colored(view.container, 'true', '--ansi-3');
	await colored(view.container, '"old"', '--ansi-2');
	await colored(view.container, ' new text', '--muted-fg');
	expect(view.container.querySelector('.code-line')?.textContent).toBe('--- a/example.ts');
	expect(view.container.querySelector('img')).toBeNull();
	expect(view.container.textContent).toContain('+export const value = "<img src=x onerror=alert(1)>";');
	await expect.poll(() => getComputedStyle(added).backgroundColor).not.toBe('rgba(0, 0, 0, 0)');
	await view.getByRole('button', { name: 'Open diff' }).click();
	expect(ondiff).toHaveBeenCalledWith('example.ts');
});

test('unknown diff languages remain plain text', async () => {
	const view = await render(Code, { code: '@@ -1 +1 @@\n-<old>\n+<new>\n', path: 'notes.unknown', diff: true });
	expect(view.container.querySelector('.code-line.added')?.textContent).toBe('+<new>');
	expect(view.container.querySelectorAll('.code-line span[class]')).toHaveLength(0);
});

test.each([
	{ kind: 'edit', id: 'edit', path: 'example.ts', diff: patch },
	{ kind: 'command', id: 'command', words: [['printf', 'hello world', '$HOME']], suggestion: [], sessionPrefixes: true },
	{ kind: 'command', id: 'python', words: [['python3', '-c', "print('hi')"]], suggestion: [], sessionPrefixes: true }
] satisfies Request[])('approval cards highlight $id requests', async (request) => {
	const view = await render(RequestCard, { request, agent: 'Claude', disabled: false, onanswer: vi.fn() });
	if (request.id === 'edit') await colored(view.container, 'true', '--ansi-3');
	else if (request.id === 'command') await colored(view.container, "'hello world'", '--ansi-2');
	else {
		expect(view.container.querySelector('pre')?.textContent).toBe(`python3 -c "print('hi')"`);
		await colored(view.container, 'print', '--ansi-4');
	}
});
