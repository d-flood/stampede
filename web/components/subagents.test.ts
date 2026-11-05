import { subagents } from '#lib/chat/agents.ts';
import SubagentHeader from '#lib/chat/SubagentHeader.svelte';
import SubagentMenu from '#lib/chat/SubagentMenu.svelte';
import SubagentRow from '#lib/chat/SubagentRow.svelte';
import Transcript from '#lib/chat/Transcript.svelte';
import type { Item, Subagent } from '#lib/device.svelte.ts';
import { createRawSnippet } from 'svelte';
import { expect, test, vi } from 'vitest';
import { page, type Locator } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

const agent = (id: string, task: string, rest: Partial<Subagent> = {}): Subagent => ({ kind: 'subagent', id, task, status: 'done', outcome: 'completed', items: [], ...rest });
const dot = (element: Element) => element.querySelector<HTMLElement>('.dot')!.style.backgroundColor;
const words = (locator: Locator) => locator.element().textContent!.replace(/\s+/g, ' ').trim();
const callbacks = () => ({ onopen: vi.fn(), onattend: vi.fn(), ondiff: vi.fn(), oncanvas: vi.fn() });

function muted(token = '--muted-fg') {
	const probe = document.body.appendChild(document.createElement('span'));
	probe.style.color = `var(${token})`;
	const color = getComputedStyle(probe).color;
	probe.remove();
	return color;
}

test('a delegated Subagent shows its Rig and Agent logo without an origin badge', async () => {
	const subagent: Subagent = { kind: 'subagent', id: 'delegated', origin: 'stampede', name: 'Reviewer', agent: 'claude-code', task: 'Review the diff', status: 'working', items: [] };
	const view = await render(SubagentRow, { props: { subagent, color: 'var(--subagent-1)' } });
	await expect.element(view.getByRole('img', { name: 'Claude Code' })).toBeVisible();
	await expect.element(view.getByText('Reviewer', { exact: true })).toBeVisible();
	await expect.element(view.getByText(/Stampede delegate|Harness/)).not.toBeInTheDocument();
});

test('a Codex Subagent shows its nickname followed by its muted role', async () => {
	const subagent: Subagent = { kind: 'subagent', id: 'codex', origin: 'harness', name: 'Heisenberg', role: 'explorer', task: 'find_needle', status: 'working', items: [] };
	const view = await render(SubagentRow, { props: { subagent, color: 'var(--subagent-1)' } });
	const name = view.getByText('Heisenberg', { exact: true });
	const role = view.getByText('explorer', { exact: true });
	await expect.element(name).toBeVisible();
	await expect.element(role).toHaveStyle({ color: muted() });
	expect(name.element().compareDocumentPosition(role.element()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
	await expect.element(view.getByRole('img')).not.toBeInTheDocument();
	await expect.element(view.getByText(/Stampede delegate|Harness/)).not.toBeInTheDocument();
});

test('a Subagent keeps its spawn-order color on its transcript line, menu row and breadcrumb', async () => {
	const nested = agent('nested', 'Search deeper', { name: 'Explore' });
	const parent = agent('parent', 'Find the file', { items: [{ kind: 'text', text: 'Looking.', phase: 'commentary', streaming: false }, nested] });
	const main: Item[] = [parent, agent('sibling', 'Review the diff', { status: 'working', outcome: undefined })];

	const transcript = await render(Transcript, { items: parent.items, main, path: ['parent'], ...callbacks() });
	expect(dot(transcript.getByRole('button', { name: /Search deeper/ }).element())).toBe('var(--subagent-2)');
	const top = await render(Transcript, { items: main, ...callbacks() });
	expect(dot(top.getByRole('button', { name: /Find the file/ }).element())).toBe('var(--subagent-1)');
	expect(dot(top.getByRole('button', { name: /Review the diff/ }).element())).toBe('var(--subagent-3)');

	const trigger = createRawSnippet(() => ({ render: () => '<button>Subagents</button>' }));
	await render(SubagentMenu, { heading: 'Subagents', entries: subagents(main), main, open: true, trigger, onselect: vi.fn() });
	expect(dot(page.getByRole('menuitem', { name: /Search deeper/ }).element())).toBe('var(--subagent-2)');

	const header = await render(SubagentHeader, { subagent: nested, path: ['parent', 'nested'], trail: subagents(main).slice(0, 1), main, onopen: vi.fn() });
	const breadcrumb = header.getByRole('navigation', { name: 'Breadcrumb' });
	expect(dot(breadcrumb.getByRole('button', { name: 'Find the file' }).element())).toBe('var(--subagent-1)');
	expect(dot(breadcrumb.getByText('Search deeper').element().closest('[aria-current]')!)).toBe('var(--subagent-2)');
});

test('seven Subagents cycle back to the first color', async () => {
	const main = Array.from({ length: 7 }, (_, index) => agent(`agent-${index}`, `Task ${index}`));
	const view = await render(Transcript, { items: main, ...callbacks() });
	const lines = view.getByRole('button', { name: /^Task \d/ }).elements();
	expect(lines.map(dot)).toEqual([1, 2, 3, 4, 5, 6, 1].map((index) => `var(--subagent-${index})`));
});

test('a transcript line states its Subagent outcome and a waiting line attends its request', async () => {
	const { onopen, onattend, ...rest } = callbacks();
	const items: Item[] = [
		agent('done', 'Old task', { name: 'Explore', totals: { durationMs: 48_000, tools: 11 } }),
		agent('failed', 'Broken task', { outcome: 'failed' }),
		agent('fresh', 'New task', { status: 'working', outcome: undefined }),
		agent('waiting', 'Review the file', { name: 'Reviewer', role: 'reviewer', agent: 'codex', status: 'waiting', outcome: undefined })
	];
	const view = await render(Transcript, { items, onopen, onattend, ...rest });
	const line = (task: string) => view.getByRole('button', { name: new RegExp(task) });
	expect(words(line('Old task'))).toBe('Explore Old task finished in 48s, 11 tools');
	expect(words(line('Broken task'))).toBe('Broken task failed');
	expect(words(line('New task'))).toBe('New task started');
	expect(words(line('Review the file'))).toBe('Reviewer reviewer Review the file waiting for you');
	await expect.element(line('Review the file').getByText('waiting for you')).toHaveStyle({ color: muted('--status-waiting') });
	await expect.element(line('Review the file').getByRole('img', { name: 'Codex' })).toBeVisible();
	await line('Review the file').click();
	expect(onattend).toHaveBeenCalledWith('waiting');
	expect(onopen).not.toHaveBeenCalled();
	await line('Old task').click();
	expect(onopen).toHaveBeenCalledWith(['done']);
});
