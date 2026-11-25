import type { ClientMessage, Device, ServerMessage } from '#lib/device.svelte.ts';
import type { Session } from '#lib/editor/lsp-editor.ts';
import { hostOf } from '#lib/host.ts';
import type { Compartment } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';

export type State = 'off' | 'starting' | 'on' | 'error';

export type Attached = { view: EditorView; compartment: Compartment; path: string; languageId: string; editor: boolean; shown: boolean };

export function uri(path: string) {
	return `file://${path.split('/').map(encodeURIComponent).join('/')}`;
}

export class LanguageServers {
	switches = $state<Record<string, boolean>>({});
	states = $state<Record<string, State>>({});
	sessions = new Map<string, Session>();
	#device: (projectId: string) => Device;
	#root: (projectId: string) => string | undefined;
	#open: (projectId: string, path: string) => void;
	#waiting = new Map<string, (view: EditorView | null) => void>();

	constructor(device: (projectId: string) => Device, root: (projectId: string) => string | undefined, open: (projectId: string, path: string) => void) {
		this.#device = device;
		this.#root = root;
		this.#open = open;
	}

	send(projectId: string, message: ClientMessage) {
		this.#device(projectId).send(message);
	}

	root(projectId: string) {
		return this.#root(projectId);
	}

	toggle(projectId: string) {
		this.send(projectId, { type: 'lsp.switch', projectId, on: !this.switches[projectId] });
	}

	message(message: ServerMessage, host?: string) {
		switch (message.type) {
			case 'hello':
				for (const session of this.sessions.values()) if (hostOf(session.projectId) === host) session.update('off');
				break;
			case 'lsp.switch':
				this.switches[message.projectId] = message.on;
				for (const session of this.sessions.values()) {
					if (message.on && session.projectId === message.projectId && session.shown) {
						this.send(session.projectId, { type: 'lsp.open', channel: session.channel });
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

	displayed(view: EditorView, shown: boolean) {
		for (const session of this.sessions.values()) session.show(view, shown);
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
