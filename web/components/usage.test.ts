import Usage from '#lib/shell/Usage.svelte';
import CodexResets from '#lib/shell/CodexResets.svelte';
import type { Reading } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

const now = Date.parse('2026-10-17T14:00:00Z');
const minute = 60_000;
const hour = 60 * minute;
const day = 24 * hour;
const readings: Reading[] = [
	{ agent: 'claude-code', window: 'five-hour', used: 1, readAt: now - 2 * minute, resetsAt: now + 3 * hour + 9 * minute },
	{ agent: 'claude-code', window: 'weekly', used: 0.12, readAt: now - 2 * minute, resetsAt: now + 6 * day + 14 * hour + 59 * minute },
	{ agent: 'codex', window: 'weekly', used: 0.43, readAt: now, resetsAt: now + 4 * day + 20 * hour + 15 * minute },
	{ agent: 'copilot', window: 'monthly', consumption: { amount: 1234.5, unit: 'credits' }, readAt: now, startsAt: Date.parse('2026-10-01T00:00:00Z'), resetsAt: Date.parse('2026-11-01T00:00:00Z') }
];

test('usage groups agents, formats billing, and advances reset countdowns', async () => {
	vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval'] });
	vi.setSystemTime(now);
	const view = await render(Usage, { readings });
	expect(view.getByRole('group').elements()).toHaveLength(3);
	expect(view.getByRole('meter').elements()).toHaveLength(3);
	await expect.element(view.getByRole('meter', { name: 'Claude · 5h' })).toHaveAttribute('aria-valuenow', '100');
	expect([...view.container.querySelectorAll('.countdown')].map((node) => node.textContent)).toEqual(['in 3h 9m', 'in 6d 14h', 'in 4d 20h']);
	expect(view.container.querySelector('.amount')?.textContent?.trim()).toBe('$12.345');
	expect(view.container.querySelector('.credits')?.textContent).toBe('1,234.5 credits');
	expect(view.container.querySelector('.cycle')?.textContent?.replace(/\s+/g, ' ').trim()).toBe('Oct 1 – Nov 1');
	expect(view.getByRole('group', { name: 'Claude', exact: true }).element().querySelector('.age')?.textContent).toBe('2m ago');
	await vi.advanceTimersByTimeAsync(minute);
	await expect.poll(() => view.container.querySelector('.countdown')?.textContent).toBe('in 3h 8m');
	await view.rerender({ readings: [{ ...readings[0], resetsAt: now + 2 * minute }] });
	await expect.poll(() => view.container.querySelector('.countdown')?.textContent).toBe('in 1m');
	await vi.advanceTimersByTimeAsync(minute);
	await expect.poll(() => view.container.querySelector('.countdown')?.textContent).toBe('now');
	await vi.advanceTimersByTimeAsync(minute);
	await expect.poll(() => view.container.querySelector('.countdown')?.textContent).toBe('now');
	await view.rerender({ readings: [] });
	await expect.poll(() => getComputedStyle(view.container.querySelector('section')!).display).toBe('none');
});

test.each(matchMedia('(pointer: coarse) and (max-width: 767px)').matches ? [384, 320] : [384, 320, 256, 208, 160])('billing states fit a %i-pixel container', async (width) => {
	const view = await render(Usage, { readings: [readings[3]] });
	view.container.style.width = `${width}px`;
	const cases: { readings: Reading[]; amount: string; credits?: string }[] = [
		{ readings: [{ ...readings[3], consumption: { amount: 0, unit: 'credits' } }], amount: '$0.00', credits: '0 credits' },
		{ readings: [{ ...readings[3], consumption: { amount: 75.5, unit: 'requests' } }], amount: '75.5 requests' },
		{ readings: [{ agent: 'copilot', window: 'monthly', readAt: now }], amount: 'Usage unavailable' },
		{ readings: [{ agent: 'copilot', window: 'session', chatId: 'first', consumption: { amount: 12.5, unit: 'credits' }, readAt: now }, { agent: 'copilot', window: 'session', chatId: 'second', consumption: { amount: 2.5, unit: 'credits' }, partial: true, readAt: now }], amount: '$0.15 reported', credits: '15 credits' },
		{ readings: [{ agent: 'copilot', window: 'session', chatId: 'first', consumption: { amount: 9, unit: 'requests' }, readAt: now }], amount: '9 requests' }
	];
	for (const sample of cases) {
		await view.rerender({ readings: sample.readings });
		await expect.poll(() => view.container.querySelector('.amount')?.textContent?.replace(/\s+/g, ' ').trim()).toBe(sample.amount);
		if (sample.credits) expect(view.container.querySelector('.credits')?.textContent).toBe(sample.credits);
		await expect.poll(() => {
			const billing = view.container.querySelector('.billing')!;
			const bounds = billing.getBoundingClientRect();
			return [...billing.children].filter((node) => node.getClientRects().length).every((node) => node.scrollWidth <= node.clientWidth && node.getBoundingClientRect().right <= bounds.right + 1);
		}).toBe(true);
	}
});

test('reset confirmation retries the same request and presents every outcome', async () => {
	const attempts: { idempotencyKey: string; creditId?: string }[] = [];
	let outcome = 'reset';
	const original = globalThis.fetch;
	vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
		if (input !== '/api/usage/codex/reset') return original(input, init);
		attempts.push(JSON.parse(String(init?.body)));
		return attempts.length === 1 ? new Response('Connection lost. Retry this reset.', { status: 502 }) : Response.json(outcome);
	});
	const view = await render(CodexResets, { credits: { availableCount: 2, expiresAt: now + 2 * day, creditId: 'next' }, now });
	await expect.element(view.getByText('Expires in 2d')).toBeVisible();
	await view.getByRole('button', { name: 'Use a Codex reset' }).click();
	await view.getByRole('button', { name: 'Cancel', exact: true }).click();
	expect(attempts).toHaveLength(0);
	await view.getByRole('button', { name: 'Use a Codex reset' }).click();
	await view.getByRole('button', { name: 'Confirm', exact: true }).click();
	await expect.element(view.getByRole('alert')).toHaveTextContent('Connection lost. Retry this reset.');
	await view.getByRole('button', { name: 'Retry', exact: true }).click();
	await expect.element(view.getByRole('status')).toHaveTextContent('Reset applied');
	expect(attempts).toHaveLength(2);
	expect(attempts[1]).toEqual(attempts[0]);
	expect(attempts[0].creditId).toBe('next');
	for (const [result, message] of [['nothingToReset', 'No eligible limit to reset'], ['noCredit', 'No resets available'], ['alreadyRedeemed', 'Reset applied']]) {
		outcome = result;
		await view.getByRole('button', { name: 'Use a Codex reset' }).click();
		await view.getByRole('button', { name: 'Confirm', exact: true }).click();
		await expect.element(view.getByRole('status')).toHaveTextContent(message);
	}
	await view.rerender({ credits: { availableCount: 3 } });
	expect(view.container.querySelector('time')).toBeNull();
	await view.rerender({ credits: { availableCount: 0 } });
	await expect.element(view.getByRole('button', { name: 'Use a Codex reset' })).toBeDisabled();
});
