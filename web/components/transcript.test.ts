import Transcript from '#lib/chat/Transcript.svelte';
import type { Item } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

test('messages stay visible while tool groups update and retain their open state', async () => {
	const items: Item[] = [
		{ kind: 'text', text: 'I’ll read the files.', phase: 'commentary', streaming: false },
		{ kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'notes.txt' }] }
	];
	const view = await render(Transcript, { items, onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });
	const summaries = () => [...view.container.querySelectorAll('.calls > summary')];
	await expect.element(view.getByText('I’ll read the files.', { exact: true })).toBeVisible();
	expect(summaries().map((element) => element.textContent)).toEqual(['1 tool callRead notes.txt']);
	await expect.element(view.getByText('Read 1 file', { exact: true })).not.toBeVisible();
	items[1] = { kind: 'tools', calls: [{ kind: 'read', name: 'Read', detail: 'notes.txt' }, { kind: 'search', name: 'Grep', detail: 'needle' }] };
	items.push({ kind: 'text', text: 'I found the match.', phase: 'commentary', streaming: false }, { kind: 'command', command: 'wc -l notes.txt', status: 'running', output: '' });
	await view.rerender({ items: [...items] });
	expect(summaries().map((element) => element.textContent)).toEqual(['2 tool callsGrep needle', '1 tool call$ wc -l notes.txt']);
	await expect.element(view.getByText('I found the match.', { exact: true })).toBeVisible();
	await page.elementLocator(summaries()[1]).click();
	items[3] = { kind: 'command', command: 'wc -l notes.txt', status: 'ok', output: '3 notes.txt' };
	items.push({ kind: 'text', text: 'The file has three lines.', phase: 'final', streaming: false });
	await view.rerender({ items: [...items] });
	expect(view.container.querySelectorAll('.calls[open]')).toHaveLength(1);
	await page.elementLocator(view.container.querySelectorAll('.command')[0].closest('summary')!).click();
	await expect.element(view.getByText('3 notes.txt', { exact: true })).toBeVisible();
	await expect.element(view.getByText('The file has three lines.', { exact: true })).toBeVisible();
	await page.elementLocator(summaries()[1]).click();
	await expect.poll(() => summaries()[0].getBoundingClientRect().height).toBeGreaterThanOrEqual(matchMedia('(pointer: coarse)').matches ? 48 : 26);
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
});

test('opening an elided command requests its original item index', async () => {
	const onexpand = vi.fn();
	const items: Item[] = [{ kind: 'text', text: 'Output follows.', phase: 'final', streaming: false }, { kind: 'command', command: 'cat notes.txt', status: 'ok', output: '', elided: true }];
	const view = await render(Transcript, { items, onexpand, onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });
	await page.elementLocator(view.container.querySelector('.calls > summary')!).click();
	await page.elementLocator(view.container.querySelector('.command')!.closest('summary')!).click();
	await expect.poll(() => onexpand.mock.calls).toEqual([[1]]);
	await view.rerender({ items: [items[0], { kind: 'command', command: 'cat notes.txt', status: 'ok', output: 'Full command output' }] });
	await expect.element(view.getByText('Full command output', { exact: true })).toBeVisible();
});
