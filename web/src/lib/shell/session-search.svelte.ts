import type { Coverage, Device, FoundSession, ServerMessage, Session } from '#lib/device.svelte.ts';
import fuzzysort from 'fuzzysort';
import { onMount } from 'svelte';

/** Where a finder searches: its Device's Host, for a Project, whose Sessions count under `focus`. */
export type Searching = { device: Device; projectId: string; focus: string };

export function sessionKey({ key }: Session): string {
	return `${key.agent}:${key.store}:${key.sessionId}`;
}

export function projectName(session: Session, labels?: Map<string, string>): string {
	return labels?.get(session.project.id) ?? session.project.path.split('/').filter(Boolean).pop() ?? session.project.path;
}

/** The Host's results for the latest query a finder typed. Call during component init. */
export class SessionSearch {
	found = $state.raw<FoundSession[]>([]);
	coverage = $state.raw<Coverage>();
	/** Whether the latest query still awaits its first reply. */
	pending = $state(false);
	#target: () => Searching | undefined;
	#query = '';
	#current: string | undefined;
	#timer: ReturnType<typeof setTimeout> | undefined;
	#sent = false;

	constructor(target: () => Searching | undefined) {
		this.#target = target;
		onMount(() => {
			const device = target()?.device;
			const unlisten = device?.listen({ message: (message) => this.#message(message) });
			return () => {
				clearTimeout(this.#timer);
				unlisten?.();
				const target = this.#target();
				if (this.#sent && target) target.device.send({ type: 'sessions.search', searchId: crypto.randomUUID(), projectId: target.projectId, query: '' });
			};
		});
	}

	get indexing(): boolean {
		return !!this.coverage && this.coverage.indexed < this.coverage.total;
	}

	set(query: string) {
		clearTimeout(this.#timer);
		this.#query = query.trim() ? query : '';
		this.#current = undefined;
		this.pending = !!this.#query;
		if (!this.#query) {
			this.found = [];
			this.coverage = undefined;
			return;
		}
		this.#timer = setTimeout(() => this.#run(), 250);
	}

	#run() {
		const target = this.#target();
		if (!this.#query || !target) return;
		this.#current = crypto.randomUUID();
		this.#sent = true;
		target.device.send({ type: 'sessions.search', searchId: this.#current, projectId: target.projectId, query: this.#query });
	}

	#message(message: ServerMessage) {
		if (message.type === 'hello' && this.#query) this.#run();
		if (message.type !== 'sessions.found' || message.searchId !== this.#current) return;
		this.found = message.results;
		this.coverage = message.coverage;
		this.pending = false;
	}
}

/** Splits the Host's results into the focused Project's and the rest, each followed by title matches from lists already held that the Host didn't return. */
export function grouped(found: FoundSession[], held: Session[], query: string, focus: string): { here: FoundSession[]; other: FoundSession[] } {
	const returned = new Set(found.map(sessionKey));
	const titled = fuzzysort
		.go(query, held.filter((session) => !returned.has(sessionKey(session))), { key: 'title', limit: 0, threshold: 0 })
		.map(({ obj }): FoundSession => ({ ...obj, matches: 0 }));
	const here = (session: Session) => session.project.id === focus;
	return { here: [...found.filter(here), ...titled.filter(here)], other: [...found.filter((session) => !here(session)), ...titled.filter((session) => !here(session))] };
}
