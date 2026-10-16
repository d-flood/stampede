import { json, RequestError } from '#lib/files.svelte.ts';

export type Kind = 'added' | 'modified' | 'deleted' | 'renamed' | 'untracked';

export type Change = { path: string; kind: Kind; from?: string };

export type Git = { isRepo: boolean; changes: Change[] };

export const kinds: { kind: Kind; letter: string; label: string }[] = [
	{ kind: 'modified', letter: 'M', label: 'Modified' },
	{ kind: 'renamed', letter: 'R', label: 'Renamed' },
	{ kind: 'added', letter: 'A', label: 'Added' },
	{ kind: 'untracked', letter: 'U', label: 'Untracked' },
	{ kind: 'deleted', letter: 'D', label: 'Deleted' }
];

export const letters = Object.fromEntries(kinds.map(({ kind, letter }) => [kind, letter])) as Record<Kind, string>;

export async function status(projectId: string): Promise<Git> {
	return json(await fetch(`/api/projects/${projectId}/git`));
}

export async function baseline(projectId: string, path: string, git?: Git): Promise<string | null> {
	const change = git?.changes.find((change) => change.path === path);
	if (!git?.isRepo || change?.kind === 'untracked') return null;
	const response = await fetch(`/api/projects/${projectId}/git/head?${new URLSearchParams({ path: change?.from ?? path })}`);
	if (!response.ok) throw new RequestError(response.status, await response.text());
	const content = await response.text();
	// HEAD is also empty for ignored files, so an empty HEAD counts only when git lists the path as changed.
	return content || change ? content : null;
}
