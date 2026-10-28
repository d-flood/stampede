export class OpenCanvas {
	html = $state<string>();
	versions = $state(0);
	version = $state<number>();
	missing = $state(false);
	#request = 0;

	constructor(public projectId: string, public canvas: string) {}

	get id() {
		return `canvas:${this.canvas}`;
	}

	get name() {
		return canvasName(this.canvas);
	}

	async load(version: number | undefined) {
		const request = ++this.#request;
		this.version = version;
		const response = await fetch(`/api/canvases/${this.canvas}${version ? `?version=${version}` : ''}`).catch(() => undefined);
		const html = response?.ok ? await response.text() : undefined;
		if (request !== this.#request) return;
		this.missing = html === undefined;
		if (html === undefined) return;
		this.versions = Number(response!.headers.get('x-canvas-versions'));
		this.html = html;
	}

	changed(versions: number) {
		this.versions = versions;
		if (this.version === undefined) this.load(undefined);
	}
}

export function canvasName(canvas: string) {
	return canvas.split('/').at(-1)!.replace(/\.html$/, '');
}
