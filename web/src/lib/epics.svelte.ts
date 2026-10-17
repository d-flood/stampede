import type { Epic, TicketStatus } from '#lib/device.svelte.ts';

export class OpenEpic {
	ticket = $state<number>();
	page = $state<'brief' | 'setup'>();
	showResolved = $state(false);

	constructor(public projectId: string, public slug: string) {}

	get id() {
		return `epic:${this.projectId}:${this.slug}`;
	}

	get name() {
		return `Epic ${this.slug}`;
	}
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
