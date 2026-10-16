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

export async function head(projectId: string, path: string): Promise<string> {
	const response = await fetch(`/api/projects/${projectId}/git/head?${new URLSearchParams({ path })}`);
	if (!response.ok) throw new RequestError(response.status, await response.text());
	return response.text();
}
