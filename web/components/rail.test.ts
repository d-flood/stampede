import ProjectRow from '#lib/shell/ProjectRow.svelte';
import type { Project } from '#lib/device.svelte.ts';
import { labels, worktrees } from '#lib/projects.ts';
import { expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

type Cloud = NonNullable<Project['cloud']>;

const parent: Project = { id: 'p', path: '/home/david/repos/stampede' };
const cloud = (rest: Partial<Cloud>): Cloud => ({ setup: 'ready', setupTerminal: null, state: 'awake', reason: null, tip: null, inSync: true, ...rest });
const rows: { branch: string; cloud: Cloud; text?: string; hollow: boolean }[] = [
	{ branch: 'gone', cloud: cloud({ state: 'gone', setup: 'failed', inSync: false }), text: 'gone', hollow: true },
	{ branch: 'unreachable', cloud: cloud({ state: 'unreachable', setup: 'failed' }), text: 'unreachable', hollow: true },
	{ branch: 'failed', cloud: cloud({ setup: 'failed', state: 'asleep', inSync: false }), text: 'setup failed', hollow: false },
	{ branch: 'creating', cloud: cloud({ setup: 'creating', inSync: false }), text: 'setting up', hollow: false },
	{ branch: 'running', cloud: cloud({ setup: 'running', state: 'asleep' }), text: 'setting up', hollow: false },
	{ branch: 'diverged', cloud: cloud({ inSync: false, state: 'asleep' }), text: 'out of sync', hollow: false },
	{ branch: 'asleep', cloud: cloud({ state: 'asleep' }), text: 'asleep', hollow: true },
	{ branch: 'awake', cloud: cloud({}), hollow: false }
];

test('a Cloud Host worktree tile under its parent shows the cloud mark and its state', async () => {
	const projects: Project[] = [
		parent,
		{ id: 'local', path: '/w/local', worktree: { parent: 'p', branch: 'local' } },
		...rows.map(({ branch, cloud }) => ({ id: branch, host: branch, path: `/w/${branch}`, worktree: { parent: 'p', branch }, cloud }))
	];
	const tiles = worktrees(projects, {}, [], () => 'done', new Map()).get('p')!;
	const view = await render(ProjectRow, {
		project: parent,
		label: 'stampede',
		labels: labels(projects),
		tiles,
		epics: [],
		root: true,
		onfocus: vi.fn(),
		onclose: vi.fn(),
		ondelete: vi.fn(),
		onsetup: vi.fn(),
		onworktree: vi.fn(),
		onmenu: vi.fn(),
		onicon: vi.fn()
	});
	const group = view.getByRole('group', { name: 'Worktrees of stampede' });
	const local = group.getByRole('button', { name: 'local', exact: true });
	await expect.element(local).toBeVisible();
	expect(local.element().querySelector('.cloud, .state')).toBeNull();
	for (const { branch, text, hollow } of rows) {
		const tile = group.getByRole('button', { name: [branch, 'on a Cloud Host', text].filter(Boolean).join(', '), exact: true });
		if (text) await expect.element(tile.getByText(`· ${text}`, { exact: true })).toBeVisible();
		else expect(tile.element().querySelector('.state')).toBeNull();
		const mark = tile.element().querySelector<HTMLElement>('.cloud');
		await expect.element(mark!).toBeVisible();
		expect(mark!.classList.contains('hollow'), branch).toBe(hollow);
	}
});
