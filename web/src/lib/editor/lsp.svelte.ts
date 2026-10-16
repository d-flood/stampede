import type { ClientMessage, Device, ServerMessage } from '#lib/device.svelte.ts';
import { setDiagnostics } from '@codemirror/lint';
import {
	hoverTooltips,
	jumpToDefinitionKeymap,
	LSPClient,
	LSPPlugin,
	serverCompletion,
	Workspace,
	type Transport,
	type WorkspaceFile
} from '@codemirror/lsp-client';
import { ChangeSet, Compartment, type Extension, type Text } from '@codemirror/state';
import { EditorView, keymap, ViewPlugin } from '@codemirror/view';

export type State = 'off' | 'starting' | 'on' | 'error';

type Position = { line: number; character: number };

type Report = {
	kind: 'full' | 'unchanged';
	items: { range: { start: Position; end: Position }; severity?: number; message: string }[];
};

type Attached = { view: EditorView; compartment: Compartment; path: string; languageId: string; editor: boolean };

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

const severities = ['error', 'error', 'warning', 'info', 'hint'] as const;

function language(path: string) {
	return languages[path.match(/\.([^./]+)$/)?.[1].toLowerCase() ?? ''];
}

export function serverLanguage(path: string): string | undefined {
	return language(path)?.[0];
}

function uri(path: string) {
	return `file://${path.split('/').map(encodeURIComponent).join('/')}`;
}

function offset(doc: Text, { line, character }: Position) {
	const found = doc.line(Math.min(line + 1, doc.lines));
	return Math.min(found.from + character, found.to);
}

export class LanguageServers {
	switches = $state<Record<string, boolean>>({});
	states = $state<Record<string, State>>({});
	#sessions = new Map<string, Session>();
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

	session(projectId: string, language: string): Session {
		const channel = `lsp:${projectId}:${language}`;
		let session = this.#sessions.get(channel);
		if (!session) this.#sessions.set(channel, (session = new Session(this, projectId, channel)));
		return session;
	}

	message(message: ServerMessage) {
		switch (message.type) {
			case 'hello':
				for (const session of this.#sessions.values()) session.update('off');
				break;
			case 'lsp.switch':
				this.switches[message.projectId] = message.on;
				for (const session of this.#sessions.values()) {
					if (message.on && session.projectId === message.projectId && session.views.size) {
						this.send({ type: 'lsp.open', channel: session.channel });
					}
				}
				break;
			case 'lsp.state':
				this.#sessions.get(message.channel)?.update(message.state);
				break;
			case 'lsp':
				this.#sessions.get(message.channel)?.receive(message.message);
				break;
			case 'fs.changed': {
				const root = this.#root(message.projectId);
				if (!root) break;
				const changes = message.paths.map((path) => ({ uri: uri(`${root}/${path}`), type: 2 }));
				for (const session of this.#sessions.values()) {
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

export function languageServer(servers: LanguageServers, projectId: string, path: string, editor: boolean): Extension {
	const found = language(path);
	const root = servers.root(projectId);
	if (!found || !root) return [];
	const [name, languageId] = found;
	const session = servers.session(projectId, name);
	const compartment = new Compartment();
	return [
		compartment.of([]),
		ViewPlugin.define((view) => {
			const entry = { view, compartment, path: `${root}/${path}`, languageId, editor };
			session.attach(entry);
			return { destroy: () => session.detach(entry) };
		})
	];
}

class Session {
	readonly servers: LanguageServers;
	readonly projectId: string;
	readonly channel: string;
	views = new Set<Attached>();
	#client?: LSPClient;
	#handlers = new Set<(message: string) => void>();
	#timer?: ReturnType<typeof setTimeout>;

	#transport: Transport = {
		send: (message) => this.servers.send({ type: 'lsp', channel: this.channel, message }),
		subscribe: (handler) => this.#handlers.add(handler),
		unsubscribe: (handler) => this.#handlers.delete(handler)
	};

	constructor(servers: LanguageServers, projectId: string, channel: string) {
		this.servers = servers;
		this.projectId = projectId;
		this.channel = channel;
	}

	attach(entry: Attached) {
		if (!this.views.size && this.servers.switches[this.projectId]) this.servers.send({ type: 'lsp.open', channel: this.channel });
		this.views.add(entry);
		this.servers.attached(entry);
		queueMicrotask(() => this.#connect(entry));
	}

	detach(entry: Attached) {
		this.views.delete(entry);
		if (!this.views.size && this.servers.switches[this.projectId]) this.servers.send({ type: 'lsp.close', channel: this.channel });
	}

	update(state: State) {
		this.servers.states[this.channel] = state;
		if (state === 'on' && !this.#client) this.#start();
		else if (state !== 'on' && this.#client) this.#stop();
	}

	receive(message: string) {
		if (message.includes('"client/registerCapability"') || message.includes('"client/unregisterCapability"')) {
			const { id, method } = JSON.parse(message);
			// TypeScript 7 waits on this reply before serving anything, and the client would refuse it.
			if (method === 'client/registerCapability' || method === 'client/unregisterCapability') {
				return this.#transport.send(JSON.stringify({ jsonrpc: '2.0', id, result: null }));
			}
		}
		for (const handler of this.#handlers) handler(message);
	}

	changed(changes: { uri: string; type: number }[]) {
		if (!this.#client) return;
		this.#client.notification('workspace/didChangeWatchedFiles', { changes });
		this.refresh();
	}

	refresh() {
		clearTimeout(this.#timer);
		this.#timer = setTimeout(() => this.#pull(), 500);
	}

	#start() {
		const root = this.servers.root(this.projectId);
		if (!root) return;
		const refresh = ViewPlugin.define(() => {
			this.refresh();
			return { update: (update) => void (update.docChanged && this.refresh()) };
		});
		this.#client = new LSPClient({
			rootUri: uri(root),
			timeout: 30_000,
			workspace: (client) => new SharedWorkspace(client, this),
			notificationHandlers: { 'window/showMessage': () => true },
			extensions: [serverCompletion({ override: true }), hoverTooltips(), keymap.of(jumpToDefinitionKeymap), refresh]
		}).connect(this.#transport);
		for (const entry of this.views) this.#connect(entry);
	}

	#stop() {
		this.#client?.disconnect();
		this.#client = undefined;
		for (const { view, compartment } of this.views) {
			view.dispatch({ effects: compartment.reconfigure([]) });
			view.dispatch(setDiagnostics(view.state, []));
		}
	}

	#connect(entry: Attached) {
		if (!this.#client || !this.views.has(entry) || LSPPlugin.get(entry.view)) return;
		entry.view.dispatch({ effects: entry.compartment.reconfigure(this.#client.plugin(uri(entry.path), entry.languageId)) });
	}

	#pull() {
		const client = this.#client;
		if (!client) return;
		client.sync();
		for (const file of client.workspace.files as SharedFile[]) {
			client.request<object, Report>('textDocument/diagnostic', { textDocument: { uri: file.uri } }).then(
				(report) => {
					if (report.kind !== 'full' || client !== this.#client) return;
					for (const view of file.views) show(view, report);
				},
				() => {}
			);
		}
	}
}

function show(view: EditorView, report: Report) {
	const plugin = LSPPlugin.get(view);
	if (!plugin) return;
	const at = (position: Position) => plugin.unsyncedChanges.mapPos(offset(plugin.syncedDoc, position));
	const diagnostics = report.items.map(({ range, severity, message }) => ({
		from: at(range.start),
		to: at(range.end),
		severity: severities[severity ?? 1],
		message
	}));
	view.dispatch(setDiagnostics(view.state, diagnostics));
}

class SharedFile implements WorkspaceFile {
	readonly uri: string;
	readonly languageId: string;
	version = 0;
	doc: Text;
	views: EditorView[];
	replaced = false;

	constructor(uri: string, languageId: string, view: EditorView) {
		this.uri = uri;
		this.languageId = languageId;
		this.doc = view.state.doc;
		this.views = [view];
	}

	getView(main?: EditorView) {
		return main && this.views.includes(main) ? main : (this.views[0] ?? null);
	}
}

class SharedWorkspace extends Workspace {
	files: SharedFile[] = [];
	#session: Session;

	constructor(client: LSPClient, session: Session) {
		super(client);
		this.#session = session;
	}

	syncFiles() {
		const updates = [];
		for (const file of this.files) {
			const main = file.views[0];
			const plugin = LSPPlugin.get(main);
			if (!plugin) continue;
			const prevDoc = file.doc;
			// A second view of the file takes over when the first closes, and may not match it.
			const changes = file.replaced
				? ChangeSet.of({ from: 0, to: prevDoc.length, insert: main.state.doc }, prevDoc.length)
				: plugin.unsyncedChanges;
			file.replaced = false;
			if (changes.empty) continue;
			file.doc = main.state.doc;
			file.version++;
			for (const view of file.views) LSPPlugin.get(view)?.clear();
			updates.push({ file, prevDoc, changes });
		}
		return updates;
	}

	openFile(uri: string, languageId: string, view: EditorView) {
		const file = this.files.find((file) => file.uri === uri);
		if (file) {
			file.views.push(view);
			return;
		}
		const opened = new SharedFile(uri, languageId, view);
		this.files.push(opened);
		this.client.didOpen(opened);
	}

	closeFile(uri: string, view: EditorView) {
		const file = this.files.find((file) => file.uri === uri);
		if (!file) return;
		const main = file.views[0];
		file.views = file.views.filter((other) => other !== view);
		if (!file.views.length) {
			this.files = this.files.filter((other) => other !== file);
			this.client.didClose(uri);
		} else if (view === main) {
			file.replaced = true;
		}
	}

	displayFile(uri: string) {
		return this.#session.servers.display(this.#session, uri);
	}
}
