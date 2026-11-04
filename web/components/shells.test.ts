import Shells from '#lib/chat/Shells.svelte';
import type { Item } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

test('long-running commands retain nested ownership until their outputs finish', async () => {
	vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
	const shell: Item = { kind: 'command', command: `pnpm dev ${'long-argument-'.repeat(20)}`, status: 'running', output: 'Server ready' };
	const watcher: Item = { kind: 'command', command: 'pnpm test --watch', status: 'running', output: 'Watching files' };
	const child: Item = { kind: 'subagent', id: 'builder', name: 'Builder', task: 'Build the app', status: 'working', items: [watcher] };
	const view = await render(Shells, { items: [shell, child] });
	expect(view.container.textContent).toBe('');
	await vi.advanceTimersByTimeAsync(1500);
	await expect.element(view.getByRole('region', { name: 'Running shells' })).toMatchTextContent(/Running shells · 2/);
	const group = view.container.querySelector('section > details > summary');
	if (group) {
		await expect.element(view.getByText('Builder', { exact: true })).not.toBeVisible();
		await page.elementLocator(group).click();
	}
	await expect.element(view.getByText('Builder', { exact: true })).toBeVisible();
	await page.elementLocator(view.container.querySelector('.shell summary')!).click();
	await expect.element(view.getByText('Server ready', { exact: true })).toBeVisible();
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
	await view.rerender({ items: [{ ...shell, status: 'ok' }, { ...child, status: 'done' }] });
	await expect.element(view.getByRole('region', { name: 'Running shells' })).toMatchTextContent(/Running shells · 1/);
	await expect.element(view.getByText('pnpm test --watch', { exact: false })).toBeVisible();
	await view.rerender({ items: [{ ...shell, status: 'ok' }, { ...child, status: 'done', items: [{ ...watcher, status: 'failed' }] }] });
	await expect.element(view.getByRole('region', { name: 'Running shells' })).toMatchTextContent(/Shells finished/);
	await vi.advanceTimersByTimeAsync(1000);
	await expect.element(view.getByRole('region', { name: 'Running shells' })).not.toBeInTheDocument();
});

test('commands that finish before the display delay never open the shell tray', async () => {
	vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
	const command: Item = { kind: 'command', command: 'true', status: 'running', output: '' };
	const view = await render(Shells, { items: [command] });
	await vi.advanceTimersByTimeAsync(1000);
	await view.rerender({ items: [{ ...command, status: 'ok' }] });
	await vi.advanceTimersByTimeAsync(2000);
	expect(view.container.textContent).toBe('');
});
