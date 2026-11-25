import Usage from '#lib/shell/Usage.svelte';
import CodexResets from '#lib/shell/CodexResets.svelte';
import { form } from '#lib/form.ts';
import type { Reading } from '#lib/device.svelte.ts';
import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page, userEvent } from 'vitest/browser';

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

const resets = { availableCount: 2, expiresAt: now + 2 * day, creditId: 'next' };

function tooltip() {
	return page.getByRole('tooltip');
}

function text(node: Element) {
	return [...node.children].map((child) => child.textContent!.replace(/\s+/g, ' ').trim()).filter(Boolean).join(' ');
}

function inTooltip(selector: string) {
	return () => [...document.querySelectorAll(`[role="tooltip"] ${selector}`)].map((node) => node.textContent?.replace(/\s+/g, ' ').trim());
}

test('the row shows labelled bars with numbers, reset times, the Reset count, and Copilot cost', async () => {
	vi.useFakeTimers({ toFake: ['Date'] });
	vi.setSystemTime(now);
	const view = await render(Usage, { readings, codexResets: resets });
	const section = view.getByRole('region', { name: 'Usage windows' });
	await expect.poll(() => section.element().getBoundingClientRect().height).toBe(form() === 'phone' ? 79 : 67);
	expect(view.getByRole('group').elements()).toHaveLength(3);
	expect(view.getByRole('meter').elements()).toHaveLength(3);
	await expect.element(view.getByRole('meter', { name: 'Claude · 5h' })).toHaveAttribute('aria-valuenow', '100');
	await expect.element(view.getByRole('meter', { name: 'Claude · week' })).toHaveAttribute('aria-valuenow', '12');
	await expect.element(view.getByRole('meter', { name: 'Codex · week' })).toHaveAttribute('aria-valuenow', '43');
	const claude = view.getByRole('group', { name: 'Claude', exact: true }).element();
	const codex = view.getByRole('group', { name: 'Codex', exact: true }).element();
	expect(text(claude)).toBe('5h 100 3h 9m wk 12 6d 14h');
	const warning = claude.querySelector('.high .number')!;
	const probe = warning.appendChild(document.createElement('i'));
	probe.style.color = 'var(--status-waiting)';
	expect(getComputedStyle(warning).color).toBe(getComputedStyle(probe).color);
	probe.remove();
	expect(text(codex)).toBe('wk 43 4d 20h 2 resets');
	await expect.element(view.getByRole('group', { name: 'Codex', exact: true }).getByText('2 resets')).toBeVisible();
	expect(text(view.getByRole('group', { name: 'Copilot', exact: true }).element())).toBe('$12.345 this month');
	for (const [used, high] of [[0.79, false], [0.8, true]] as const) {
		await view.rerender({ readings: [{ ...readings[2], used }], codexResets: { availableCount: 1 } });
		await expect.poll(() => codex.querySelector('.meter')?.classList.contains('high')).toBe(high);
		expect(codex.querySelector('.number')?.textContent).toBe(String(Math.round(used * 100)));
		expect(codex.textContent).toContain('1 reset');
	}
	await view.rerender({ readings: [readings[2]], codexResets: { availableCount: 0 } });
	await expect.poll(() => codex.querySelector('.count')).toBeNull();
	await view.rerender({ readings: [], codexResets: null });
	await expect.poll(() => getComputedStyle(view.container.querySelector('section')!).display).toBe('none');
	vi.useRealTimers();
});

const longest: Reading[] = (['claude-code', 'codex'] as const).flatMap((agent) => [
	{ agent, window: 'five-hour' as const, used: 1, readAt: now, resetsAt: now + 4 * hour + 59 * minute },
	{ agent, window: 'weekly' as const, used: 1, readAt: now, resetsAt: now + 6 * day + 23 * hour }
]);
const antigravity: Reading[] = [
	{ agent: 'antigravity', window: 'weekly', group: { name: 'Gemini Models', short: 'gemini' }, used: 0.3, readAt: now, resetsAt: now + 6 * day + 23 * hour },
	{ agent: 'antigravity', window: 'weekly', group: { name: 'Claude and GPT models', short: 'other' }, used: 1, readAt: now, resetsAt: now + 6 * day + 23 * hour }
];

test('Resets, logos, bars, and reset times fit at every Sidebar width when every window is high', async () => {
	vi.useFakeTimers({ toFake: ['Date'] });
	vi.setSystemTime(now);
	const view = await render(Usage, { readings: [...longest, ...antigravity, readings[3]], codexResets: { availableCount: 12 } });
	const row = view.container.querySelector('.row')!;
	expect(row.querySelectorAll('[role="meter"]')).toHaveLength(5);
	await expect.element(view.getByRole('meter', { name: 'Google Antigravity · week' })).toHaveAttribute('aria-valuenow', '100');
	for (const width of [160, 240, 300, form() === 'phone' ? 430 : 395]) {
		view.container.style.width = `${width}px`;
		await expect.poll(() => {
			const box = row.getBoundingClientRect();
			return box.width === width && [...row.querySelectorAll('.brand, .period, .number, .count, .cost')].every((node) => {
				const { left, right, width } = node.getBoundingClientRect();
				return width > 0 && left >= box.left && right <= box.right;
			}) && [...row.querySelectorAll('.gauge, .remaining')].every((node) => {
				const { left, right, width } = node.getBoundingClientRect();
				return width === 0 || (left >= box.left && right <= box.right);
			});
		}).toBe(true);
	}
	vi.useRealTimers();
});

test('every Agent window and the Copilot cost line up in shared columns that span the row', async () => {
	vi.useFakeTimers({ toFake: ['Date'] });
	vi.setSystemTime(now);
	const view = await render(Usage, { readings: [...longest.slice(0, 2), readings[2], ...antigravity, readings[3]], codexResets: { availableCount: 3 } });
	const row = view.container.querySelector('.row')!;
	const edges = (selector: string, side: 'left' | 'right' = 'left') => new Set([...row.querySelectorAll(selector)].map((node) => Math.round(node.getBoundingClientRect()[side])));
	for (const width of [160, 320]) {
		view.container.style.width = `${width}px`;
		await expect.poll(() => row.getBoundingClientRect().width).toBe(width);
		expect(row.querySelectorAll('.weekly .period')).toHaveLength(3);
		for (const selector of ['.brand', '.weekly .period', '.weekly .gauge', '.weekly .remaining']) expect(edges(selector).size, selector).toBe(1);
		expect(edges('.weekly .number', 'right').size).toBe(1);
		expect(edges('.cost')).toEqual(edges('.five-hour'));
	}
	const box = row.getBoundingClientRect();
	const style = getComputedStyle(row);
	expect(row.querySelector('.count')!.getBoundingClientRect().right).toBeCloseTo(box.right - parseFloat(style.paddingRight), 0);
	expect(row.querySelector('.brand')!.getBoundingClientRect().left).toBeCloseTo(box.left + parseFloat(style.paddingLeft), 0);
	vi.useRealTimers();
});

test('high usage follows the displayed percent', async () => {
	const view = await render(Usage, { readings: [{ ...readings[2], used: 0.796 }], codexResets: null });
	await expect.element(view.getByRole('meter', { name: 'Codex · week' })).toHaveClass('high');
	expect(view.container.querySelector('.number')?.textContent).toBe('80');
	await view.getByRole('button', { name: 'Usage details' }).click();
	const popover = page.getByRole('dialog', { name: 'Usage details' });
	await expect.element(popover).toBeVisible();
	expect(popover.element().querySelector('.window.high .percent')?.textContent).toBe('80%');
});

test('hovering reveals every reading at once and keeps countdowns current', async () => {
	vi.useFakeTimers({ toFake: ['Date', 'setInterval', 'clearInterval', 'setTimeout', 'clearTimeout'] });
	vi.setSystemTime(now);
	const view = await render(Usage, { readings, codexResets: resets });
	await view.getByRole('button', { name: 'Usage details' }).hover();
	await new Promise(requestAnimationFrame);
	// Polling advances fake timers, which would run an open delay; this read has to come first.
	expect(tooltip().query()).not.toBeNull();
	await expect.element(tooltip()).toBeVisible();
	expect(inTooltip('.window')()).toEqual([
		expect.stringMatching(/^5h 100% .* in 3h 9m$/),
		expect.stringMatching(/^Weekly 12% .* in 6d 14h$/),
		expect.stringMatching(/^Weekly 43% .* in 4d 20h$/)
	]);
	expect(inTooltip('.countdown')()).toEqual(['in 3h 9m', 'in 6d 14h', 'in 4d 20h']);
	expect(inTooltip('.age')()).toEqual(['2m ago', 'just now', 'just now']);
	expect(inTooltip('.amount')()).toEqual(['$12.345']);
	expect(inTooltip('.credits')()).toEqual(['1,234.5 credits']);
	expect(inTooltip('.cycle')()).toEqual(['Oct 1 – Nov 1']);
	expect(inTooltip('.resets')()).toEqual(['2 resets · Expires in 2d']);
	await vi.advanceTimersByTimeAsync(minute);
	await expect.poll(() => inTooltip('.countdown')()[0]).toBe('in 3h 8m');
	expect(view.container.querySelector('.remaining')?.textContent).toBe('3h 8m');
	await view.rerender({ readings: [{ ...readings[0], resetsAt: now + 2 * minute }] });
	await expect.poll(() => inTooltip('.countdown')()[0]).toBe('in 1m');
	await vi.advanceTimersByTimeAsync(minute);
	await expect.poll(() => inTooltip('.countdown')()[0]).toBe('now');
	await vi.advanceTimersByTimeAsync(minute);
	await expect.poll(() => inTooltip('.countdown')()[0]).toBe('now');
	vi.useRealTimers();
});

test('every billing state reads in the tooltip without overflowing it', async () => {
	const view = await render(Usage, { readings: [readings[3]] });
	await view.getByRole('button', { name: 'Usage details' }).hover();
	await expect.element(tooltip()).toBeVisible();
	const cases: { readings: Reading[]; amount: string; credits?: string; notes: string[] }[] = [
		{ readings: [{ ...readings[3], consumption: { amount: 0, unit: 'credits' } }], amount: '$0.00', credits: '0 credits', notes: ['This billing period'] },
		{ readings: [{ ...readings[3], consumption: { amount: 75.5, unit: 'requests' } }], amount: '75.5 requests', notes: ['This billing period'] },
		{ readings: [{ agent: 'copilot', window: 'monthly', readAt: now }], amount: 'Usage unavailable', notes: ['Billing usage not reported'] },
		{ readings: [{ agent: 'copilot', window: 'session', chatId: 'first', consumption: { amount: 12.5, unit: 'credits' }, readAt: now }, { agent: 'copilot', window: 'session', chatId: 'second', consumption: { amount: 2.5, unit: 'credits' }, partial: true, readAt: now }], amount: '$0.15 reported', credits: '15 credits', notes: [] },
		{ readings: [{ agent: 'copilot', window: 'session', chatId: 'first', consumption: { amount: 9, unit: 'requests' }, readAt: now }], amount: '9 requests', notes: ['Cost not reported'] }
	];
	for (const sample of cases) {
		await view.rerender({ readings: sample.readings });
		await expect.poll(inTooltip('.amount')).toEqual([sample.amount]);
		expect(inTooltip('.credits')()).toEqual(sample.credits ? [sample.credits] : []);
		expect(inTooltip('.billing .period')()).toEqual(sample.notes);
		await expect.poll(() => {
			const box = document.querySelector('[role="tooltip"]')!.getBoundingClientRect();
			return [...document.querySelectorAll('[role="tooltip"] .billing > *')].every((node) => node.getBoundingClientRect().right <= box.right + 1);
		}).toBe(true);
	}
});

test('clicking the row opens the detail popover with Use a reset', async () => {
	vi.useFakeTimers({ toFake: ['Date'] });
	vi.setSystemTime(now);
	const view = await render(Usage, { readings, codexResets: resets });
	const trigger = view.getByRole('button', { name: 'Usage details' });
	await trigger.hover();
	await expect.element(tooltip()).toBeVisible();
	await trigger.click();
	const popover = page.getByRole('dialog', { name: 'Usage details' });
	await expect.element(popover).toBeVisible();
	await expect.element(tooltip()).not.toBeInTheDocument();
	await expect.element(popover.getByRole('button', { name: 'Use a reset' })).toBeEnabled();
	await expect.element(popover.getByText('Expires in 2d')).toBeVisible();
	await expect.element(popover.getByText('$12.345')).toBeVisible();
	expect([...popover.element().querySelectorAll('.countdown')].map((node) => node.textContent)).toHaveLength(3);
	await popover.getByRole('button', { name: 'Use a reset' }).click();
	await expect.element(popover.getByText('Use 1 reset?')).toBeVisible();
	await trigger.hover();
	await expect.element(tooltip()).not.toBeInTheDocument();
	vi.useRealTimers();
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
	await view.getByRole('button', { name: 'Use a reset' }).click();
	await view.getByRole('button', { name: 'Cancel', exact: true }).click();
	expect(attempts).toHaveLength(0);
	await view.getByRole('button', { name: 'Use a reset' }).click();
	await view.getByRole('button', { name: 'Confirm', exact: true }).click();
	await expect.element(view.getByRole('alert')).toHaveTextContent('Connection lost. Retry this reset.');
	await view.getByRole('button', { name: 'Retry', exact: true }).click();
	await expect.element(view.getByRole('status')).toHaveTextContent('Reset applied');
	expect(attempts).toHaveLength(2);
	expect(attempts[1]).toEqual(attempts[0]);
	expect(attempts[0].creditId).toBe('next');
	for (const [result, message] of [['nothingToReset', 'No eligible limit to reset'], ['noCredit', 'No resets available'], ['alreadyRedeemed', 'Reset applied']]) {
		outcome = result;
		await view.getByRole('button', { name: 'Use a reset' }).click();
		await view.getByRole('button', { name: 'Confirm', exact: true }).click();
		await expect.element(view.getByRole('status')).toHaveTextContent(message);
	}
	await view.rerender({ credits: { availableCount: 3 } });
	expect(view.container.querySelector('time')).toBeNull();
	await view.rerender({ credits: { availableCount: 0 } });
	await expect.element(view.getByRole('button', { name: 'Use a reset' })).toBeDisabled();
});

test('closing Usage details keeps a pending Reset retry', async () => {
	const attempts: { idempotencyKey: string; creditId?: string }[] = [];
	const original = globalThis.fetch;
	vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
		if (input !== '/api/usage/codex/reset') return original(input, init);
		attempts.push(JSON.parse(String(init?.body)));
		return attempts.length === 1 ? new Response('Connection lost. Retry this reset.', { status: 502 }) : Response.json('reset');
	});
	const view = await render(Usage, { readings, codexResets: resets });
	const popover = page.getByRole('dialog', { name: 'Usage details' });
	await view.getByRole('button', { name: 'Usage details' }).click();
	await popover.getByRole('button', { name: 'Use a reset' }).click();
	await popover.getByRole('button', { name: 'Confirm', exact: true }).click();
	await expect.element(popover.getByRole('alert')).toHaveTextContent('Connection lost. Retry this reset.');
	await userEvent.keyboard('{Escape}');
	await expect.element(popover).not.toBeInTheDocument();
	await view.getByRole('button', { name: 'Usage details' }).click();
	await popover.getByRole('button', { name: 'Use a reset' }).click();
	await popover.getByRole('button', { name: 'Retry', exact: true }).click();
	await expect.element(popover.getByRole('status')).toHaveTextContent('Reset applied');
	expect(attempts).toHaveLength(2);
	expect(attempts[1]).toEqual(attempts[0]);
});
