import SubagentRow from '#lib/chat/SubagentRow.svelte';
import type { Subagent } from '#lib/device.svelte.ts';
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';

function muted() {
	const probe = document.body.appendChild(document.createElement('span'));
	probe.style.color = 'var(--muted-fg)';
	const color = getComputedStyle(probe).color;
	probe.remove();
	return color;
}

test('a delegated Subagent shows its Rig and Agent logo without an origin badge', async () => {
	const subagent: Subagent = { kind: 'subagent', id: 'delegated', origin: 'stampede', name: 'Reviewer', agent: 'claude-code', task: 'Review the diff', status: 'working', items: [] };
	const view = await render(SubagentRow, { props: { subagent } });
	await expect.element(view.getByRole('img', { name: 'Claude Code' })).toBeVisible();
	await expect.element(view.getByText('Reviewer', { exact: true })).toBeVisible();
	await expect.element(view.getByText(/Stampede delegate|Harness/)).not.toBeInTheDocument();
});

test('a Codex Subagent shows its nickname followed by its muted role', async () => {
	const subagent: Subagent = { kind: 'subagent', id: 'codex', origin: 'harness', name: 'Heisenberg', role: 'explorer', task: 'find_needle', status: 'working', items: [] };
	const view = await render(SubagentRow, { props: { subagent } });
	const name = view.getByText('Heisenberg', { exact: true });
	const role = view.getByText('explorer', { exact: true });
	await expect.element(name).toBeVisible();
	await expect.element(role).toHaveStyle({ color: muted() });
	expect(name.element().compareDocumentPosition(role.element()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
	await expect.element(view.getByRole('img')).not.toBeInTheDocument();
	await expect.element(view.getByText(/Stampede delegate|Harness/)).not.toBeInTheDocument();
});
