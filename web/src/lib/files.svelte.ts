export type Entry = { name: string; path: string; dir: boolean; ignored: boolean };

export type Contents = { content: string; hash: string; binary?: undefined } | { binary: true; hash: string };

export class OpenFile {
	projectId: string;
	path: string;
	dirty = $state(false);

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

async function json<T>(response: Response): Promise<T> {
	if (!response.ok) throw new Error(await response.text());
	return response.json();
}

export async function list(projectId: string, path: string, includeIgnored: boolean): Promise<Entry[]> {
	const query = new URLSearchParams({ path });
	if (includeIgnored) query.set('includeIgnored', '1');
	return json(await fetch(`/api/projects/${projectId}/tree?${query}`));
}

export async function read(projectId: string, path: string): Promise<Contents> {
	return json(await fetch(`/api/projects/${projectId}/file?${new URLSearchParams({ path })}`));
}

export async function save(projectId: string, path: string, content: string, baseHash: string) {
	const response = await fetch(`/api/projects/${projectId}/file`, {
		method: 'PUT',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ path, content, baseHash })
	});
	return json<{ hash: string }>(response);
}
