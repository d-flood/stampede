import ContextBar from '#lib/chat/ContextBar.svelte';
import type { Context } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

test('context readings format tokens, clamp the meter, and mark high usage', async () => {
	const context: Context = { used: 512_000, window: 1_000_000 };
	const view = await render(ContextBar, { props: { context, tokens: { input: 1200, output: 150 } } });
	const meter = view.getByRole('meter', { name: 'Context' });
	await expect.element(meter).toHaveAttribute('aria-valuetext', '512k / 1M (51%)');
	await view.rerender({ context: { used: 1_200_000, window: 1_000_000 } });
	await expect.element(meter).toHaveAttribute('aria-valuenow', '1000000');
	await expect.element(meter).toHaveClass('high');
	await expect.poll(() => view.container.querySelector('.fill')?.getAttribute('stroke-dasharray')).toBe('100 100');
});

test('expired cache readings show their state and token details', async () => {
	const now = Date.now();
	vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
	vi.setSystemTime(now);
	const context: Context = { used: 512_000, window: 1_000_000, cache: { expires: now - 3 * 86_400_000, then: 'cold' } };
	const view = await render(ContextBar, { props: { context, tokens: { input: 1200, output: 150 } } });
	const meter = view.getByRole('meter', { name: 'Context' });
	await expect.element(meter).toHaveAttribute('aria-valuetext', '512k / 1M (51%), cache cold');
	await meter.hover();
	await expect.element(view.getByRole('tooltip')).toMatchTextContent('Cache cold · expired 3 days ago');
	await expect.element(view.getByRole('tooltip')).toMatchTextContent('1k in · 150 out');
});

test('cache state updates when its expiry passes', async () => {
	const now = Date.now();
	vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
	vi.setSystemTime(now);
	const view = await render(ContextBar, { props: { context: { used: 1000, window: 2000, cache: { expires: now + 60_000, then: 'probably-cold' } } } });
	const meter = view.getByRole('meter', { name: 'Context' });
	await expect.element(meter).toHaveAttribute('aria-valuetext', '1k / 2k (50%)');
	await vi.advanceTimersByTimeAsync(60_000);
	await expect.element(meter).toHaveAttribute('aria-valuetext', '1k / 2k (50%), cache probably cold');
});
