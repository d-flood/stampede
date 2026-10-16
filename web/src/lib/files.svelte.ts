import { form } from '#lib/form.ts';
import fuzzysort, { type Snapshot } from 'fuzzysort';

export type Entry ={ name: string; path: string; dir: boolean; ignored: boolean };

export type Contents = { content: string; hash: string; binary?: undefined } | { binary: true; hash: string };

export class OpenFile {
	projectId: string;
	path: string;
	dirty = $state(false);
	deleted = $state(false);

	constructor(projectId: string, path: string) {
		this.projectId = projectId;
		this.path = path;
	}

	get id() {
		return `file:${this.projectId}:${this.path}`;
	}

	get name() {
		return this.path.split('/').at(-1) ?? this.path;
	}
}

export type Layout = 'split' | 'unified';

export class OpenDiff extends OpenFile {
	against: 'HEAD' | 'disk';
	from?: string;
	snapshot = $state.raw<{ disk: string; buffer: string }>();
	layout = $state<Layout>(form() === 'phone' ? 'unified' : 'split');

	constructor(projectId: string, path: string, against: 'HEAD' | 'disk', from?: string) {
		super(projectId, path);
		this.against = against;
		this.from = from;
	}

	get id() {
		return `diff:${this.against}:${this.projectId}:${this.path}`;
	}
}

export class RequestError extends Error {
	status: number;

	constructor(status: number, message: string) {
		super(message);
		this.status = status;
	}
}

export async function json<T>(response: Response): Promise<T> {
	if (!response.ok) throw new RequestError(response.status, await response.text());
	return response.json();
}

class Listing {
	snapshot = $state.raw<Snapshot>();
	#projectId: string;
	#stale = true;

	constructor(projectId: string) {
		this.#projectId = projectId;
	}

	async refresh() {
		if (!this.#stale) return;
		this.#stale = false;
		try {
			this.snapshot = fuzzysort.snapshot(await json<string[]>(await fetch(`/api/projects/${this.#projectId}/files`)));
		} catch {
			this.#stale = true;
		}
	}

	invalidate() {
		this.#stale = true;
	}
}

const listings = new Map<string, Listing>();

export function listing(projectId: string): Listing {
	let found = listings.get(projectId);
	if (!found) listings.set(projectId, (found = new Listing(projectId)));
	return found;
}

export function invalidate(projectId: string) {
	listings.get(projectId)?.invalidate();
}

export async function list(projectId: string, path: string, includeIgnored: boolean): Promise<Entry[]> {
	const query = new URLSearchParams({ path });
	if (includeIgnored) query.set('includeIgnored', '1');
	return json(await fetch(`/api/projects/${projectId}/tree?${query}`));
}

export async function read(projectId: string, path: string): Promise<Contents> {
	return json(await fetch(`/api/projects/${projectId}/file?${new URLSearchParams({ path })}`));
}

export async function save(
	projectId: string,
	path: string,
	content: string,
	baseHash: string
): Promise<{ hash: string; conflict?: undefined } | { conflict: Contents }> {
	const response = await fetch(`/api/projects/${projectId}/file`, {
		method: 'PUT',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ path, content, baseHash })
	});
	if (response.status === 409) return { conflict: await response.json() };
	return json(response);
}
