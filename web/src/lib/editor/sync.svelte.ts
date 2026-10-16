import type { Device } from '#lib/device.svelte.ts';
import { OpenConfig, RequestError, type OpenFile } from '#lib/files.svelte.ts';
import { Text } from '@codemirror/state';
import { EditorView } from '@codemirror/view';

export type Disk = { content: string; hash: string };

export class Sync {
	conflict = $state<Disk>();
	#file: () => OpenFile;
	#view: () => EditorView | undefined;
	#device?: Device;
	#hash = '';
	#saved = Text.empty;
	#saving = false;
	#stale = false;
	#queued = false;
	#generation = 0;

	constructor(file: () => OpenFile, view: () => EditorView | undefined) {
		this.#file = file;
		this.#view = view;
	}

	listener = EditorView.updateListener.of((update) => {
		if (update.docChanged) this.#file().dirty = !update.state.doc.eq(this.#saved);
	});

	start(hash: string, doc: Text) {
		this.#hash = hash;
		this.#saved = doc;
	}

	listen(device: Device) {
		this.#device = device;
		return device.listen({
			message: (message) => {
				const file = this.#file();
				if (message.type === 'hello') return this.#resync();
				if (message.type === 'config.changed' && file instanceof OpenConfig) return this.refresh();
				if (message.type !== 'fs.changed' || message.projectId !== file.projectId || file instanceof OpenConfig) return;
				if (message.paths.some((path) => file.path === path || file.path.startsWith(`${path}/`))) this.refresh();
			}
		});
	}

	async write() {
		const file = this.#file();
		const view = this.#view();
		if (!view || !file.dirty || this.#saving) return;
		if (this.#device?.connection === 'reconnecting') {
			this.#queued = true;
			return;
		}
		const doc = view.state.doc;
		this.#saving = true;
		this.#generation++;
		try {
			const result = await file.save(doc.toString(), this.#hash);
			if (result.conflict) {
				if (!result.conflict.binary) this.conflict = result.conflict;
				return;
			}
			file.deleted = false;
			this.#hash = result.hash;
			this.#saved = doc;
			this.conflict = undefined;
			file.dirty = !(this.#view() ?? view).state.doc.eq(doc);
		} finally {
			this.#saving = false;
			if (this.#stale) {
				this.#stale = false;
				this.refresh();
			}
		}
	}

	#resync() {
		if (this.#queued) {
			this.#queued = false;
			this.write();
		}
		this.refresh();
	}

	async refresh() {
		const file = this.#file();
		if (!this.#view()) return;
		if (this.#saving) {
			this.#stale = true;
			return;
		}
		const current = ++this.#generation;
		try {
			const contents = await file.read();
			if (current !== this.#generation) return;
			file.deleted = false;
			if (contents.binary) return;
			if (contents.hash === this.#hash) this.conflict = undefined;
			else if (file.dirty) this.conflict = contents;
			else this.load(contents);
		} catch (error) {
			if (current === this.#generation && error instanceof RequestError && error.status === 404) {
				file.deleted = true;
				const disk = { content: '', hash: '' };
				if (file.dirty) this.conflict = disk;
				else this.load(disk);
			}
		}
	}

	load(disk: Disk) {
		const view = this.#view();
		if (!view) return;
		const doc = view.state.doc.toString();
		const next = disk.content;
		let from = 0;
		while (from < doc.length && from < next.length && doc[from] === next[from]) from++;
		let end = 0;
		while (end < doc.length - from && end < next.length - from && doc.at(-1 - end) === next.at(-1 - end)) end++;
		view.dispatch({ changes: { from, to: doc.length - end, insert: next.slice(from, next.length - end) } });
		this.#saved = view.state.doc;
		this.#hash = disk.hash;
		this.conflict = undefined;
		this.#file().dirty = false;
	}

	keepMine() {
		const view = this.#view();
		if (!view || !this.conflict) return;
		this.#saved = view.state.toText(this.conflict.content);
		this.#hash = this.conflict.hash;
		this.conflict = undefined;
		this.#file().dirty = !view.state.doc.eq(this.#saved);
	}
}
