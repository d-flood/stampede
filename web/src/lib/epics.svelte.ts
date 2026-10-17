import type { Epic, TicketStatus } from '#lib/device.svelte.ts';

export class OpenEpic {
	ticket = $state<number>();
	page = $state<'brief' | 'setup' | 'edit-brief' | 'edit-ticket' | 'new-ticket'>();
	showResolved = $state(false);

	constructor(public projectId: string, public slug: string) {}

	get id() {
		return `epic:${this.projectId}:${this.slug}`;
	}

	get name() {
		return `Epic ${this.slug}`;
	}
}

export type Edit =
	| { kind: 'brief.set'; brief: string }
	| { kind: 'setup.set'; command: string }
	| { kind: 'ticket.added'; ticket: number; title: string; body: string; blockers: number[] }
	| { kind: 'ticket.body.set'; ticket: number; body: string }
	| { kind: 'blockers.set'; ticket: number; blockers: number[] }
	| { kind: 'note.added'; ticket: number; author: 'david'; text: string }
	| { kind: 'epic.archived' };

export async function record(slug: string, edit: Edit): Promise<string | undefined> {
	const response = await fetch(`/api/epics/${slug}`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(edit)
	}).catch((error: Error) => error);
	if (response instanceof Error) return response.message;
	return response.ok ? undefined : await response.text();
}

export function toggle(blockers: number[], number: number) {
	return blockers.includes(number) ? blockers.filter((other) => other !== number) : [...blockers, number].sort((a, b) => a - b);
}

export const statuses: TicketStatus[] = ['merged', 'skipped', 'merging', 'in-progress', 'not-started'];

export const tones: Record<TicketStatus, string> = {
	merged: 'var(--status-done)',
	skipped: 'var(--muted-fg)',
	merging: 'var(--ansi-5)',
	'in-progress': 'var(--status-working)',
	'not-started': 'var(--muted-fg)'
};

export function label(status: TicketStatus) {
	return status.replace('-', ' ');
}

export function resolved(status: TicketStatus) {
	return status === 'merged' || status === 'skipped';
}

export function counts(epic: Epic) {
	const counts = { merged: 0, skipped: 0, merging: 0, 'in-progress': 0, 'not-started': 0 };
	for (const ticket of epic.tickets) counts[ticket.status]++;
	return counts;
}
