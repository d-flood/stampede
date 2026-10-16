import type { ClientMessage, Device, ServerMessage } from '#lib/device.svelte.ts';
import type { Session } from '#lib/editor/lsp-editor.ts';
import type { Compartment } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';

export type State = 'off' | 'starting' | 'on' | 'error';

export type Attached = { view: EditorView; compartment: Compartment; path: string; languageId: string; editor: boolean };

const languages: Record<string, [server: string, languageId: string]> = {
	ts: ['typescript', 'typescript'],
	mts: ['typescript', 'typescript'],
	cts: ['typescript', 'typescript'],
	tsx: ['typescript', 'typescriptreact'],
	js: ['typescript', 'javascript'],
	mjs: ['typescript', 'javascript'],
	cjs: ['typescript', 'javascript'],
	jsx: ['typescript', 'javascriptreact'],
	svelte: ['svelte', 'svelte'],
	py: ['python', 'python']
};

export function language(path: string) {
	return languages[path.match(/\.([^./]+)$/)?.[1].toLowerCase() ?? ''];
}

export function serverLanguage(path: string): string | undefined {
	return language(path)?.[0];
}

export function uri(path: string) {
	return `file://${path.split('/').map(encodeURIComponent).join('/')}`;
}

export class LanguageServers {
	switches = $state<Record<string, boolean>>({});
	states = $state<Record<string, State>>({});
	sessions = new Map<string, Session>();
	#device: Device;
	#root: (projectId: string) => string | undefined;
	#open: (projectId: string, path: string) => void;
	#waiting = new Map<string, (view: EditorView | null) => void>();

	constructor(device: Device, root: (projectId: string) => string | undefined, open: (projectId: string, path: string) => void) {
		this.#device = device;
		this.#root = root;
		this.#open = open;
	}

	send(message: ClientMessage) {
		this.#device.send(message);
	}

	root(projectId: string) {
		return this.#root(projectId);
	}

	toggle(projectId: string) {
		this.send({ type: 'lsp.switch', projectId, on: !this.switches[projectId] });
	}

	message(message: ServerMessage) {
		switch (message.type) {
			case 'hello':
				for (const session of this.sessions.values()) session.update('off');
				break;
			case 'lsp.switch':
				this.switches[message.projectId] = message.on;
				for (const session of this.sessions.values()) {
					if (message.on && session.projectId === message.projectId && session.views.size) {
						this.send({ type: 'lsp.open', channel: session.channel });
					}
				}
				break;
			case 'lsp.state':
				this.sessions.get(message.channel)?.update(message.state);
				break;
			case 'lsp':
				this.sessions.get(message.channel)?.receive(message.message);
				break;
			case 'fs.changed': {
				const root = this.#root(message.projectId);
				if (!root) break;
				const changes = message.paths.map((path) => ({ uri: uri(`${root}/${path}`), type: 2 }));
				for (const session of this.sessions.values()) {
					if (session.projectId === message.projectId) session.changed(changes);
				}
				break;
			}
		}
	}

	attached(entry: Attached) {
		const resolve = this.#waiting.get(entry.path);
		if (entry.editor && resolve) {
			this.#waiting.delete(entry.path);
			resolve(entry.view);
		}
	}

	display(session: Session, target: string): Promise<EditorView | null> {
		const root = this.#root(session.projectId);
		const path = decodeURIComponent(new URL(target).pathname);
		if (!root || !path.startsWith(`${root}/`)) return Promise.resolve(null);
		this.#open(session.projectId, path.slice(root.length + 1));
		const open = [...session.views].find((entry) => entry.editor && entry.path === path);
		if (open) return Promise.resolve(open.view);
		this.#waiting.get(path)?.(null);
		return new Promise((resolve) => {
			this.#waiting.set(path, resolve);
			setTimeout(() => {
				if (this.#waiting.get(path) !== resolve) return;
				this.#waiting.delete(path);
				resolve(null);
			}, 10_000);
		});
	}
}
