import { agentNames } from '#lib/config.svelte.ts';
import type { Coverage, Device, FoundSession, ServerMessage, Session } from '#lib/device.svelte.ts';
import fuzzysort from 'fuzzysort';
import { onMount } from 'svelte';

/** Where a finder searches: its Device's Host, for a Project, whose Sessions count under `focus`. */
export type Searching = { device: Device; projectId: string; focus: string };

/** The narrowest finder, in CSS pixels, with room for a preview pane beside its results: the Launchpad has it, the Chat list popover doesn't. */
export const PANE_MIN = 600;

export function sessionKey({ key }: Session): string {
	return `${key.agent}:${key.store}:${key.sessionId}`;
}

export function projectName(session: Session, labels?: Map<string, string>): string {
	return labels?.get(session.project.id) ?? session.project.path.split('/').filter(Boolean).pop() ?? session.project.path;
}

/** Names the Agents whose Sessions the Host couldn't list or index. */
export function outOfDate({ stale }: Coverage): string | undefined {
	return stale.length ? `${new Intl.ListFormat('en').format(stale.map((agent) => agentNames[agent]))} Sessions may be out of date` : undefined;
}

/** Each Device's finders that have searched, least recent first; the Host runs only a Device's newest search. */
const senders = new Map<Device, SessionSearch[]>();

/** The Host's results for the latest query a finder typed. Call during component init. */
export class SessionSearch {
	found = $state.raw<FoundSession[]>([]);
	coverage = $state.raw<Coverage>();
	/** Whether the latest query still awaits its first reply. */
	pending = $state(false);
	#target: () => Searching | undefined;
	#onfound: (() => void) | undefined;
	#query = '';
	#current: string | undefined;
	#timer: ReturnType<typeof setTimeout> | undefined;
	#sentTo: Searching | undefined;

	/** `onfound` runs when a new query's first results arrive. */
	constructor(target: () => Searching | undefined, onfound?: () => void) {
		this.#target = target;
		this.#onfound = onfound;
		onMount(() => {
			const device = target()?.device;
			const unlisten = device?.listen({ message: (message) => this.#message(message) });
			return () => {
				clearTimeout(this.#timer);
				unlisten?.();
				this.#release();
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
		this.#sentTo = target;
		senders.set(target.device, [...(senders.get(target.device) ?? []).filter((search) => search !== this), this]);
		target.device.send({ type: 'sessions.search', searchId: this.#current, projectId: target.projectId, query: this.#query });
	}

	/** Hands the Device's search back to the newest other finder still holding a query, or stops it. */
	#release() {
		if (!this.#sentTo) return;
		const { device, projectId } = this.#sentTo;
		const sent = senders.get(device) ?? [];
		const others = sent.filter((search) => search !== this);
		senders.set(device, others);
		if (sent.at(-1) !== this) return;
		const next = others.findLast((search) => search.#query);
		if (next) next.#run();
		else device.send({ type: 'sessions.search', searchId: crypto.randomUUID(), projectId, query: '' });
	}

	#message(message: ServerMessage) {
		if (message.type === 'hello' && this.#query) this.#run();
		if (message.type !== 'sessions.found' || message.searchId !== this.#current) return;
		this.found = message.results;
		this.coverage = message.coverage;
		if (!this.pending) return;
		this.pending = false;
		this.#onfound?.();
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
