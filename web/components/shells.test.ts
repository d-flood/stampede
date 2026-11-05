import Tray from '#lib/chat/Tray.svelte';
import type { Item } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

const callbacks = () => ({ onopen: vi.fn(), onattend: vi.fn(), oncanvas: vi.fn() });

test('long-running commands retain nested ownership until their outputs finish', async () => {
	vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
	const shell: Item = { kind: 'command', command: `pnpm dev ${'long-argument-'.repeat(20)}`, status: 'running', output: 'Server ready' };
	const watcher: Item = { kind: 'command', command: 'pnpm test --watch', status: 'running', output: 'Watching files' };
	const child: Item = { kind: 'subagent', id: 'builder', name: 'Builder', task: 'Build the app', status: 'working', items: [watcher] };
	const view = await render(Tray, { items: [shell, child], ...callbacks() });
	const chip = view.getByRole('region', { name: 'Shells' }).getByRole('button');
	await expect.element(view.getByRole('region', { name: 'Shells' })).not.toBeInTheDocument();
	await vi.advanceTimersByTimeAsync(1500);
	await expect.element(chip).toHaveTextContent('2 shells');
	await chip.click();
	const shells = page.getByRole('region', { name: 'Running shells' });
	await expect.element(shells).toMatchTextContent(/Running shells · 2/);
	await expect.element(shells.getByText('Builder', { exact: true })).toBeVisible();
	await page.elementLocator(shells.element().querySelector('.shell summary')!).click();
	await expect.element(shells.getByText('Server ready', { exact: true })).toBeVisible();
	await expect.poll(() => shells.element().scrollWidth <= shells.element().clientWidth).toBe(true);
	await view.rerender({ items: [{ ...shell, status: 'ok' }, { ...child, status: 'done' }] });
	await expect.element(shells).toMatchTextContent(/Running shells · 1/);
	await expect.element(shells.getByText('pnpm test --watch', { exact: false })).toBeVisible();
	await view.rerender({ items: [{ ...shell, status: 'ok' }, { ...child, status: 'done', items: [{ ...watcher, status: 'failed' }] }] });
	await expect.element(shells).toMatchTextContent(/Shells finished/);
	await vi.advanceTimersByTimeAsync(1000);
	await expect.element(view.getByRole('region', { name: 'Shells' })).not.toBeInTheDocument();
});

test('commands that finish before the display delay never open the shell tray', async () => {
	vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
	const command: Item = { kind: 'command', command: 'true', status: 'running', output: '' };
	const view = await render(Tray, { items: [command], ...callbacks() });
	await vi.advanceTimersByTimeAsync(1000);
	await view.rerender({ items: [{ ...command, status: 'ok' }] });
	await vi.advanceTimersByTimeAsync(2000);
	expect(view.container.textContent).toBe('');
});
