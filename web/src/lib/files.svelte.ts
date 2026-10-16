import { form } from '#lib/form.ts';
import type { Commit } from '#lib/git.ts';
import fuzzysort, { type Snapshot } from 'fuzzysort';

export type Entry ={ name: string; path: string; dir: boolean; ignored: boolean };

export type Contents = { content: string; hash: string; binary?: undefined } | { binary: true; hash: string };

export type Cursor = { line: number; column: number };

type Saved<T = { hash: string }> = (T & { conflict?: undefined }) | { conflict: Contents };

export type Invalid = { line: number | null; message: string };

export class OpenFile {
	projectId: string;
	path: string;
	dirty = $state(false);
	deleted = $state(false);
	cursor = $state<Cursor>();

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

	read(): Promise<Contents> {
		return read(this.projectId, this.path);
	}

	save(content: string, baseHash: string): Promise<Saved> {
		return put(`/api/projects/${this.projectId}/file`, { path: this.path, content, baseHash });
	}
}

export class OpenConfig extends OpenFile {
	invalid = $state<Invalid>();

	constructor(projectId: string) {
		super(projectId, 'config.toml');
	}

	get id() {
		return `config:${this.projectId}`;
	}

	async read(): Promise<Contents> {
		const contents = await json<Contents & { error: Invalid | null }>(await fetch('/api/config/file'));
		this.invalid = contents.error ?? undefined;
		return contents;
	}

	async save(content: string, baseHash: string): Promise<Saved> {
		const result = await put<{ hash: string; error: Invalid | null }>('/api/config/file', { content, baseHash });
		if (!result.conflict) this.invalid = result.error ?? undefined;
		return result;
	}
}

export type Layout = 'split' | 'unified';

export class OpenDiff extends OpenFile {
	against: 'HEAD' | 'disk' | Commit;
	from = $state<string>();
	snapshot = $state.raw<{ disk: string; buffer: string }>();
	layout = $state<Layout>(form() === 'phone' ? 'unified' : 'split');

	constructor(projectId: string, path: string, against: 'HEAD' | 'disk' | Commit, from?: string) {
		super(projectId, path);
		this.against = against;
		this.from = from;
	}

	get id() {
		return `diff:${typeof this.against === 'string' ? this.against : this.against.sha}:${this.projectId}:${this.path}`;
	}

	get versus() {
		return typeof this.against === 'string' ? this.against : this.against.short;
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

async function put<T>(url: string, body: object): Promise<Saved<T>> {
	const response = await fetch(url, {
		method: 'PUT',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
	if (response.status === 409) return { conflict: await response.json() };
	return json(response);
}
