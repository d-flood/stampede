import Tray from '#lib/chat/Tray.svelte';
import type { Item, Subagent } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

const agent = (id: string, rest: Partial<Subagent> = {}): Subagent => ({ kind: 'subagent', id, task: `Task ${id}`, status: 'working', items: [], ...rest });
const callbacks = () => ({ onopen: vi.fn(), onattend: vi.fn(), oncanvas: vi.fn() });

function token(name: string) {
	const probe = document.body.appendChild(document.createElement('span'));
	probe.style.color = `var(${name})`;
	const color = getComputedStyle(probe).color;
	probe.remove();
	return color;
}

function oneRow(tray: Element) {
	const boxes = [...tray.children].map((chip) => chip.getBoundingClientRect());
	return boxes.length > 0 && Math.max(...boxes.map(({ top }) => top)) < Math.min(...boxes.map(({ bottom }) => bottom));
}

test('the Tray stays one row with three Subagent dots, +N, and a visible waiting count', async () => {
	vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
	const items: Item[] = [
		agent('a'),
		agent('b', { status: 'waiting' }),
		agent('c', { status: 'done', outcome: 'completed' }),
		agent('d', { status: 'done', outcome: 'completed' }),
		agent('e', { items: [agent('nested')] }),
		{ kind: 'command', command: 'pnpm dev', status: 'running', output: '' }
	];
	const { onattend, ...rest } = callbacks();
	const view = await render(Tray, { items, canvases: ['/canvases/report.html'], onattend, ...rest });
	await vi.advanceTimersByTimeAsync(1500);
	const tray = view.getByRole('region', { name: 'Tray' });
	const subagents = tray.getByRole('region', { name: 'Subagents' });
	await expect.element(subagents.getByRole('button', { name: 'Subagents: 3 running, 2 done' })).toBeVisible();
	expect(subagents.element().querySelectorAll('.dot')).toHaveLength(3);
	await expect.element(subagents.getByText('+3', { exact: true })).toBeVisible();
	const waiting = subagents.getByRole('button', { name: '1 waiting', exact: true });
	await expect.element(waiting).toHaveStyle({ color: token('--status-waiting') });
	await expect.element(tray.getByRole('region', { name: 'Shells' }).getByRole('button', { name: '1 shell' })).toBeVisible();
	await expect.element(tray.getByRole('region', { name: 'Canvases' }).getByRole('button', { name: /report/ })).toBeVisible();
	expect(oneRow(tray.element())).toBe(true);
	await waiting.click();
	expect(onattend).toHaveBeenCalledWith('b');
});

test('five Subagents show three dots then +2, and the chip opens them grouped Running then Finished', async () => {
	const items: Item[] = [agent('a'), agent('b'), agent('c', { status: 'done', outcome: 'completed' }), agent('d'), agent('e', { status: 'done', outcome: 'completed' })];
	const { onopen, ...rest } = callbacks();
	const view = await render(Tray, { items, onopen, ...rest });
	const subagents = view.getByRole('region', { name: 'Subagents' });
	const chip = subagents.getByRole('button', { name: 'Subagents: 3 running, 2 done' });
	await expect.element(chip).toMatchTextContent(/^\+2\s*3 running, 2 done$/);
	const dots = [...subagents.element().querySelectorAll<HTMLElement>('.dot')].map((dot) => dot.style.backgroundColor);
	expect(dots).toEqual([1, 2, 4].map((index) => `var(--subagent-${index})`));
	await expect.element(subagents.getByRole('button', { name: /waiting/ })).not.toBeInTheDocument();
	expect(oneRow(view.getByRole('region', { name: 'Tray' }).element())).toBe(true);
	await chip.click();
	const phone = document.body.classList.contains('phone');
	await expect.poll(() => [...document.querySelectorAll('[role="menu"] .heading, [role="dialog"] h3')].map((heading) => heading.textContent!.trim())).toEqual(['Running', 'Finished']);
	await (phone ? page.getByRole('dialog').getByRole('button', { name: /^Task d/ }) : page.getByRole('menuitem', { name: /^Task d/ })).click();
	expect(onopen).toHaveBeenCalledWith(['d']);
});

test('the Tray renders nothing without Subagents, shells or canvases', async () => {
	const view = await render(Tray, { items: [{ kind: 'text', text: 'Hi', phase: 'final', streaming: false }], ...callbacks() });
	await expect.element(view.getByRole('region', { name: 'Tray' })).not.toBeInTheDocument();
});
