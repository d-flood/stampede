import { json, RequestError } from '#lib/files.svelte.ts';
import type { Change } from '#lib/git.ts';

export type Pr = { number: number; title: string; author: string; isDraft: boolean; requested: boolean; updatedAt: string };

export type Availability = { available: true } | { available: false; hidden: false; reason: string } | { available: false; hidden: true };

export type PrReview = {
	number: number;
	title: string;
	url: string;
	author: string;
	mine: boolean;
	state: 'open' | 'merged' | 'closed';
	target: { branch: string; commit: string };
	source: { commit: string };
	mergeBase: string;
	viewed: Record<string, { commit: string; blob: string }>;
	drafts: unknown[];
};

export async function prs(projectId: string): Promise<Availability & { prs: Pr[] }> {
	return json(await fetch(`/api/projects/${projectId}/prs`));
}

export async function openReview(projectId: string, pr: number | string): Promise<PrReview> {
	const body = JSON.stringify({ pr });
	return json(await fetch(`/api/projects/${projectId}/pr-reviews`, { method: 'POST', headers: { 'content-type': 'application/json' }, body }));
}

export async function refreshReview(projectId: string, pr: number): Promise<PrReview> {
	return json(await fetch(`/api/projects/${projectId}/pr-reviews/${pr}/refresh`, { method: 'POST' }));
}

export async function discardReview(projectId: string, pr: number): Promise<void> {
	const response = await fetch(`/api/projects/${projectId}/pr-reviews/${pr}`, { method: 'DELETE' });
	if (!response.ok) throw new RequestError(response.status, await response.text());
}

/** Whether `change` is Viewed or changed since it was, in `review`; neither when never Viewed. */
export function viewedState(review: PrReview, change: Change): 'viewed' | 'changed' | undefined {
	const viewed = review.viewed[change.path];
	if (!viewed) return;
	return viewed.blob === (change.blob ?? '') ? 'viewed' : 'changed';
}

export async function setViewed(projectId: string, pr: number, path: string, viewed: boolean): Promise<PrReview> {
	const body = JSON.stringify({ path, viewed });
	return json(await fetch(`/api/projects/${projectId}/pr-reviews/${pr}/viewed`, { method: 'PUT', headers: { 'content-type': 'application/json' }, body }));
}

export async function openWorktree(projectId: string, pr: number): Promise<string> {
	return (await json<{ projectId: string }>(await fetch(`/api/projects/${projectId}/pr-reviews/${pr}/worktree`, { method: 'POST' }))).projectId;
}
