import { json, RequestError } from '#lib/files.svelte.ts';

export type Kind = 'added' | 'modified' | 'deleted' | 'renamed' | 'untracked';

export type Change = { path: string; kind: Kind; from?: string };

export type Git = { isRepo: boolean; changes: Change[] };

export type Commit = { sha: string; short: string; subject: string; author: string; date: string };

export type Log = { commits: Commit[]; cursor?: string };

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
	return text(await fetch(`/api/projects/${projectId}/git/head?${new URLSearchParams({ path })}`));
}

export async function log(projectId: string, cursor?: string): Promise<Log> {
	const query = cursor ? `?${new URLSearchParams({ cursor })}` : '';
	return json(await fetch(`/api/projects/${projectId}/git/log${query}`));
}

export async function commitFiles(projectId: string, sha: string): Promise<Change[]> {
	return json(await fetch(`/api/projects/${projectId}/git/commit/${sha}`));
}

export async function show(projectId: string, sha: string, path: string): Promise<string> {
	return text(await fetch(`/api/projects/${projectId}/git/show?${new URLSearchParams({ sha, path })}`));
}

export function split(path: string) {
	const slash = path.lastIndexOf('/');
	return { name: path.slice(slash + 1), dir: path.slice(0, slash + 1) };
}

async function text(response: Response): Promise<string> {
	if (!response.ok) throw new RequestError(response.status, await response.text());
	return response.text();
}

export async function baseline(projectId: string, path: string, git?: Git): Promise<string | null> {
	if (!git?.isRepo) return null;
	const change = git.changes.find((change) => change.path === path);
	return head(projectId, change?.from ?? path);
}
