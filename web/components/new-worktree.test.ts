import NewWorktree from '#lib/shell/NewWorktree.svelte';
import { expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

function answer(cli: string) {
	const original = globalThis.fetch;
	vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
		const url = new URL(String(input), location.href);
		if (url.pathname === '/api/cloud') return Response.json({ cli, claudeToken: false });
		if (url.pathname === '/api/projects/p/git/refs') return Response.json({ local: ['main', 'bare'], remote: [] });
		if (url.pathname === '/api/projects/p/cloud-setup') {
			const from = url.searchParams.get('from');
			return Response.json(from === 'main' ? { present: true } : { present: false, error: `${from} has no Cloud Setup. Type /cloud-setup in a Chat to draft one.` });
		}
		return original(input, init);
	});
}

test('Cloud Host is offered only when usable, and says why not', async () => {
	const props = { project: { id: 'p', path: '/repos/alpha' }, current: 'main', oncreate: vi.fn(), oncancel: vi.fn() };
	answer('missing');
	const missing = await render(NewWorktree, props);
	const cloud = missing.getByRole('radio', { name: 'Cloud Host' });
	await expect.element(missing.getByText('Install the sprite CLI and sign in to use Cloud Hosts.')).toBeVisible();
	await expect.element(cloud).toBeDisabled();
	missing.unmount();
	vi.restoreAllMocks();

	answer('ready');
	const view = await render(NewWorktree, props);
	const ready = view.getByRole('radio', { name: 'Cloud Host' });
	await expect.element(ready).toBeEnabled();
	await expect.element(view.getByRole('radio', { name: 'Stampede' })).toHaveAttribute('aria-checked', 'true');
	await ready.click();
	await expect.element(view.getByText('Created for this worktree and destroyed with it. Runs the Cloud Setup from main.')).toBeVisible();
	await userEvent.selectOptions(view.getByRole('combobox', { name: 'From' }), 'bare');
	await expect.element(view.getByText('bare has no Cloud Setup. Type /cloud-setup in a Chat to draft one.')).toBeVisible();
	await expect.element(ready).toBeDisabled();
	await expect.element(view.getByRole('radio', { name: 'Stampede' })).toHaveAttribute('aria-checked', 'true');
});
