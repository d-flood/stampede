import EmptyChat from '#lib/chat/EmptyChat.svelte';
import Notice from '#lib/chat/Notice.svelte';
import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

const choices = {
	model: [{ value: 'opus', label: 'Opus 5.5' }],
	effort: [{ value: 'xhigh', label: 'xhigh' }],
	speed: [],
	ultracode: [],
	mode: [{ value: 'default', label: 'Ask before edits' }]
};

test('an empty Chat names its Rig, what it runs, where, and the input hints', async () => {
	const view = await render(EmptyChat, { rig: 'Deep Claude', agent: 'claude-code', settings: { model: 'opus', effort: 'xhigh', mode: 'default' }, choices, project: 'stampede', branch: 'main' });
	await expect.element(view.getByRole('heading', { name: 'Deep Claude' })).toBeVisible();
	expect(view.getByRole('heading').element().querySelector('svg')).not.toBeNull();
	await expect.element(view.getByText('Opus 5.5, xhigh, Ask before edits', { exact: true })).toBeVisible();
	await expect.element(view.getByText('stampede', { exact: true })).toBeVisible();
	await expect.element(view.getByText('main', { exact: true })).toBeVisible();
	const hints = view.getByRole('list', { name: 'Input hints' }).element();
	expect([...hints.querySelectorAll('li')].map((hint) => hint.textContent)).toEqual(['/commands', '@files', '!shell', ';Clips']);
	expect([...hints.querySelectorAll('kbd')].map((key) => key.textContent)).toEqual(['/', '@', '!', ';']);
	await expect.poll(() => view.container.scrollWidth <= view.container.clientWidth).toBe(true);
});

test('an error notice has a heading naming the Agent and is announced', async () => {
	const view = await render(Notice, { item: { kind: 'notice', notice: 'error', text: 'The Agent exited with code 1.' }, agent: 'codex' });
	const alert = view.getByRole('alert');
	await expect.element(alert).toBeVisible();
	await expect.element(alert.getByText('Codex error', { exact: true })).toBeVisible();
	await expect.element(alert.getByText('The Agent exited with code 1.', { exact: true })).toBeVisible();
	expect(view.getByRole('button').elements()).toHaveLength(0);
	const probe = view.container.appendChild(document.createElement('span'));
	probe.style.color = 'color-mix(in oklab, var(--ansi-1) 35%, var(--border))';
	expect(getComputedStyle(alert.element()).borderTopColor).toBe(getComputedStyle(probe).color);
});

test('a limit notice offers a reset only when Codex has one, and always a hand-off', async () => {
	const onhandoff = vi.fn();
	const item = { kind: 'notice', notice: 'limit', text: 'You hit your usage limit.' } as const;
	const view = await render(Notice, { item, agent: 'codex', resets: { availableCount: 2 }, onhandoff });
	await expect.element(view.getByText('Codex usage limit reached', { exact: true })).toBeVisible();
	await expect.element(view.getByText('You hit your usage limit.', { exact: true })).toBeVisible();
	await view.getByRole('button', { name: 'Use a reset', exact: true }).click();
	await expect.element(view.getByText('Use 1 reset?', { exact: true })).toBeVisible();
	await expect.element(view.getByRole('button', { name: 'Confirm', exact: true })).toBeVisible();
	await view.getByRole('button', { name: 'Continue with another Rig', exact: true }).click();
	expect(onhandoff).toHaveBeenCalledOnce();

	for (const props of [{ agent: 'codex', resets: { availableCount: 0 } }, { agent: 'codex', resets: null }, { agent: 'claude-code', resets: { availableCount: 2 } }] as const) {
		await view.rerender({ item, onhandoff, ...props });
		expect(view.getByRole('button', { name: 'Use a reset' }).elements()).toHaveLength(0);
		await expect.element(view.getByRole('button', { name: 'Continue with another Rig', exact: true })).toBeEnabled();
	}
});
