import Transcript from '#lib/chat/Transcript.svelte';
import type { Item } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

const summaries = (container: HTMLElement) => [...container.querySelectorAll<HTMLButtonElement>('.calls > .summary')];

test('messages stay visible while Activity groups update and retain their open state', async () => {
	const items: Item[] = [
		{ kind: 'text', text: 'I’ll read the files.', phase: 'commentary', streaming: false },
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'notes.txt' }] }
	];
	const view = await render(Transcript, { items, onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });
	await expect.element(view.getByText('I’ll read the files.', { exact: true })).toBeVisible();
	expect(summaries(view.container).map((element) => element.textContent)).toEqual(['Read 1 file']);
	expect(getComputedStyle(summaries(view.container)[0]).fontFamily).toContain('Fira Sans');
	expect(view.container.querySelector('.rail')).toBeNull();
	items[1] = { kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'notes.txt' }, { kind: 'search', name: 'Grep', detail: 'needle' }] };
	items.push({ kind: 'text', text: 'I found the match.', phase: 'commentary', streaming: false }, { kind: 'command', command: 'wc -l notes.txt', status: 'running', output: '' });
	await view.rerender({ items: [...items], activity: 'Running wc' });
	expect(summaries(view.container).map((element) => element.textContent)).toEqual(['Read 1 file, searched 1 time', 'Ran 1 command · Running wc']);
	await expect.element(view.getByText('I found the match.', { exact: true })).toBeVisible();
	await page.elementLocator(summaries(view.container)[1]).click();
	items[3] = { kind: 'command', command: 'wc -l notes.txt', status: 'ok', output: '3 notes.txt' };
	items.push({ kind: 'text', text: 'The file has three lines.', phase: 'final', streaming: false });
	await view.rerender({ items: [...items], activity: null });
	expect(summaries(view.container).map((element) => element.getAttribute('aria-expanded'))).toEqual(['false', 'true']);
	await page.elementLocator(view.container.querySelectorAll('.command')[0].closest('summary')!).click();
	await expect.element(view.getByText('3 notes.txt', { exact: true })).toBeVisible();
	await expect.element(view.getByText('The file has three lines.', { exact: true })).toBeVisible();
	await page.elementLocator(summaries(view.container)[1]).click();
	await expect.poll(() => summaries(view.container)[0].getBoundingClientRect().height).toBeGreaterThanOrEqual(matchMedia('(pointer: coarse)').matches ? 48 : 26);
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
});

test('Activity summaries count every action in order and keep failures visible while collapsed', async () => {
	const ondiff = vi.fn();
	const onexpand = vi.fn();
	const items: Item[] = [
		{ kind: 'text', text: 'Fixing the parser.', phase: 'commentary', streaming: false },
		{ kind: 'thinking', seconds: 2 },
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'src/parse.ts' }, { kind: 'read', name: 'Read', detail: 'src/lex.ts' }] },
		{ kind: 'edit', path: 'src/parse.ts', diff: '', added: 3, removed: 1 },
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'README.md' }] },
		{ kind: 'edit', path: 'src/parse.ts', diff: '', added: 2, removed: 0 },
		{ kind: 'command', command: 'pnpm lint', status: 'ok', output: 'clean' },
		{ kind: 'command', command: 'pnpm test parse', status: 'failed', output: '', elided: true }
	];
	const view = await render(Transcript, { items, onexpand, onopen: vi.fn(), onattend: vi.fn(), ondiff, oncanvas: vi.fn() });
	const [summary] = summaries(view.container);
	expect(summary.textContent).toBe('Edited 1 file, ran 2 commands, read 3 files, 1 failed');
	const probe = view.container.appendChild(document.createElement('span'));
	probe.style.color = 'var(--ansi-1)';
	expect(getComputedStyle(summary.querySelector('.failed')!).color).toBe(getComputedStyle(probe).color);
	probe.remove();

	const chips = view.container.querySelectorAll('.chip');
	expect([...chips].map((chip) => chip.textContent)).toEqual(['parse.ts+5−1']);
	await page.elementLocator(chips[0]).click();
	expect(ondiff).toHaveBeenCalledWith('src/parse.ts');

	expect(view.container.querySelector('.rail')).toBeNull();
	const failure = page.elementLocator(view.container.querySelector('.failure')!);
	await expect.element(failure).toBeVisible();
	await expect.element(failure).toHaveTextContent('pnpm test parse failed Show output');
	await view.getByRole('button', { name: 'Show output' }).click();
	expect(summary.getAttribute('aria-expanded')).toBe('true');
	expect(view.container.querySelectorAll('.rail > .step')).toHaveLength(8);
	expect(view.container.querySelectorAll('.rail details')).toHaveLength(4);
	expect(view.container.querySelector('.rail details.failed')!.hasAttribute('open')).toBe(true);
	await expect.poll(() => onexpand.mock.calls).toEqual([[7]]);
	await expect.element(failure).toBeVisible();
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
});

test('opening an elided command requests its original item index', async () => {
	const onexpand = vi.fn();
	const items: Item[] = [{ kind: 'text', text: 'Output follows.', phase: 'final', streaming: false }, { kind: 'command', command: 'cat notes.txt', status: 'ok', output: '', elided: true }];
	const view = await render(Transcript, { items, onexpand, onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });
	await page.elementLocator(summaries(view.container)[0]).click();
	await page.elementLocator(view.container.querySelector('.command')!.closest('summary')!).click();
	await expect.poll(() => onexpand.mock.calls).toEqual([[1]]);
	await view.rerender({ items: [items[0], { kind: 'command', command: 'cat notes.txt', status: 'ok', output: 'Full command output' }] });
	await expect.element(view.getByText('Full command output', { exact: true })).toBeVisible();
});
