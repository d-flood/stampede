import { form } from '#lib/form.ts';
import type { Commit } from '#lib/git.ts';
import fuzzysort, { type Snapshot } from 'fuzzysort';

export type Entry ={ name: string; path: string; dir: boolean; ignored: boolean };

export type Contents = { content: string; hash: string; binary?: undefined } | { binary: true; hash: string };

export type Cursor = { line: number; column: number };

type Saved<T = { hash: string }> = (T & { conflict?: undefined }) | { conflict: Contents };

export type Invalid = { line: number | null; message: string };

export function image(path: string): boolean {
	return /\.(avif|bmp|gif|ico|jpe?g|png|svg|webp)$/i.test(path);
}

export function imageSource(projectId: string, path: string, src: string): string | null {
	if (/^(https?|data):/i.test(src)) return src;
	if (/^([a-z][a-z0-9+.-]*:|\/|#)/i.test(src)) return null;
	const parts = path.split('/').slice(0, -1);
	for (const segment of src.split(/[?#]/, 1)[0].split('/')) {
		let name = segment;
		try {
			name = decodeURIComponent(segment);
		} catch {}
		if (!name || name === '.') continue;
		if (name !== '..') parts.push(name);
		else if (!parts.pop()) return null;
	}
	return `/api/projects/${projectId}/image?${new URLSearchParams({ path: parts.join('/') })}`;
}

export class OpenFile {
	projectId: string;
	path = $state('');
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

/** Which branches a Comparison compares, by name, so its Diff tab follows them as they move. */
export type Selection = { target: string; source: string };

/** A PR review's Comparison, by pull request number. */
export type Reviewing = { pr: number };

/**
 * What a Diff compares: HEAD, the index or the disk with the file on disk; the index with HEAD; a commit with its
 * parent; or a Comparison's Source with where it split off its Target.
 */
export type Against = 'HEAD' | 'index' | 'staged' | 'disk' | Commit | Selection | Reviewing;

export function comparing(against: Against): against is Selection | Reviewing {
	return typeof against === 'object' && ('target' in against || 'pr' in against);
}

/** The refs a Comparison names, a PR review's being its hidden fetched refs. */
export function branches(compared: Selection | Reviewing): Selection {
	if (!('pr' in compared)) return compared;
	return { target: `refs/stampede/pr/${compared.pr}/target`, source: `refs/stampede/pr/${compared.pr}/source` };
}

export class OpenDiff extends OpenFile {
	against: Against;
	from = $state<string>();
	snapshot = $state.raw<{ disk: string; buffer: string }>();
	layout = $state<Layout>(form() === 'phone' ? 'unified' : 'split');

	constructor(projectId: string, path: string, against: Against, from?: string) {
		super(projectId, path);
		this.against = against;
		this.from = from;
	}

	get id() {
		const { against } = this;
		if (comparing(against) && 'pr' in against) return `diff:pr:${against.pr}:${this.projectId}`;
		if (comparing(against)) return `diff:compare:${against.target}...${against.source}:${this.projectId}`;
		return `diff:${typeof against === 'string' ? against : against.sha}:${this.projectId}:${this.path}`;
	}

	get versus() {
		const { against } = this;
		if (comparing(against)) return 'pr' in against ? `#${against.pr}` : against.target;
		return typeof against === 'string' ? against : against.short;
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
	failed = $state(false);
	#projectId: string;
	#stale = true;

	constructor(projectId: string) {
		this.#projectId = projectId;
	}

	async refresh() {
		if (!this.#stale) return;
		this.#stale = false;
		this.failed = false;
		try {
			this.snapshot = fuzzysort.snapshot(await json<string[]>(await fetch(`/api/projects/${this.#projectId}/files`)));
		} catch {
			this.#stale = true;
			this.failed = true;
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

export async function remove(projectId: string, path: string): Promise<void> {
	const response = await fetch(`/api/projects/${projectId}/file?${new URLSearchParams({ path })}`, { method: 'DELETE' });
	if (!response.ok) throw new RequestError(response.status, await response.text());
	invalidate(projectId);
}

export async function create(projectId: string, path: string, dir: boolean): Promise<void> {
	const response = await fetch(`/api/projects/${projectId}/file`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ path, dir })
	});
	if (!response.ok) throw new RequestError(response.status, await response.text());
	invalidate(projectId);
}

export async function copy(projectId: string, path: string, destination: string): Promise<{ path: string }> {
	const result = await json<{ path: string }>(await fetch(`/api/projects/${projectId}/file/copy`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ path, destination })
	}));
	invalidate(projectId);
	return result;
}

export async function rename(projectId: string, path: string, name: string): Promise<{ path: string }> {
	const result = await json<{ path: string }>(await fetch(`/api/projects/${projectId}/file/rename`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ path, name })
	}));
	invalidate(projectId);
	return result;
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
