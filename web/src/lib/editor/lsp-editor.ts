import { language, uri, type Attached, type LanguageServers, type State } from '#lib/editor/lsp.svelte.ts';
import { forEachDiagnostic, setDiagnostics, type Diagnostic } from '@codemirror/lint';
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

type Position = { line: number; character: number };

type Report = {
	kind: 'full' | 'unchanged';
	items: {
		range: { start: Position; end: Position };
		severity?: number;
		tags?: number[];
		code?: string | number;
		source?: string;
		message: string;
	}[];
};

const severities = ['error', 'error', 'warning', 'info', 'hint'] as const;
const tagClasses: Record<number, string> = { 1: 'cm-lint-unnecessary', 2: 'cm-lint-deprecated' };
const owners = new WeakMap<Diagnostic, string>();

function offset(doc: Text, { line, character }: Position) {
	const found = doc.line(Math.min(line + 1, doc.lines));
	return Math.min(found.from + character, found.to);
}

export function languageServer(servers: LanguageServers, projectId: string, path: string, editor: boolean): Extension {
	const found = language(path);
	const root = servers.root(projectId);
	if (!found || !root) return [];
	const [names, languageId] = found;
	return names.map((name, index) => {
		const channel = `lsp:${projectId}:${name}`;
		const session = servers.sessions.get(channel) ?? new Session(servers, projectId, channel, index === 0);
		servers.sessions.set(channel, session);
		const compartment = new Compartment();
		return [
			compartment.of([]),
			ViewPlugin.define((view) => {
				const entry = { view, compartment, path: `${root}/${path}`, languageId, editor, shown: false };
				session.attach(entry);
				return {
					update: (update) => {
						if (update.docChanged) session.edited(update.view);
					},
					destroy: () => session.detach(entry)
				};
			})
		];
	});
}

export class Session {
	readonly servers: LanguageServers;
	readonly projectId: string;
	readonly channel: string;
	readonly primary: boolean;
	views = new Set<Attached>();
	#client?: LSPClient;
	#handlers = new Set<(message: string) => void>();
	#timer?: ReturnType<typeof setTimeout>;

	#transport: Transport = {
		send: (message) => this.servers.send({ type: 'lsp', channel: this.channel, message }),
		subscribe: (handler) => this.#handlers.add(handler),
		unsubscribe: (handler) => this.#handlers.delete(handler)
	};

	constructor(servers: LanguageServers, projectId: string, channel: string, primary: boolean) {
		this.servers = servers;
		this.projectId = projectId;
		this.channel = channel;
		this.primary = primary;
	}

	get shown() {
		return [...this.views].some((entry) => entry.shown);
	}

	show(view: EditorView, shown: boolean) {
		const entry = [...this.views].find((entry) => entry.view === view);
		if (!entry) return;
		const before = this.shown;
		entry.shown = shown;
		this.visibility(before);
	}

	visibility(before: boolean) {
		if (before !== this.shown && this.servers.switches[this.projectId]) {
			this.servers.send({ type: this.shown ? 'lsp.open' : 'lsp.close', channel: this.channel });
		}
	}

	attach(entry: Attached) {
		this.views.add(entry);
		this.servers.attached(entry);
		queueMicrotask(() => this.#connect(entry));
	}

	detach(entry: Attached) {
		const before = this.shown;
		this.views.delete(entry);
		if (!this.primary) this.#client?.workspace.closeFile(uri(entry.path), entry.view);
		this.visibility(before);
	}

	edited(view: EditorView) {
		if (!this.#client) return;
		(this.#client.workspace as SharedWorkspace).edited(view);
		this.refresh();
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
		this.#client = new LSPClient({
			rootUri: uri(root),
			timeout: 30_000,
			workspace: (client) => new SharedWorkspace(client, this),
			notificationHandlers: { 'window/showMessage': () => true },
			extensions: [
				{ clientCapabilities: { textDocument: { diagnostic: { tagSupport: { valueSet: [1, 2] } } } } },
				serverCompletion({ override: true }),
				hoverTooltips(),
				keymap.of(jumpToDefinitionKeymap)
			]
		}).connect(this.#transport);
		for (const entry of this.views) this.#connect(entry);
	}

	#stop() {
		this.#client?.disconnect();
		this.#client = undefined;
		for (const { view, compartment } of this.views) {
			view.dispatch({ effects: compartment.reconfigure([]) });
			publish(view, this.channel, []);
		}
	}

	#connect(entry: Attached) {
		const client = this.#client;
		if (!client || !this.views.has(entry)) return;
		if (!this.primary) client.workspace.openFile(uri(entry.path), entry.languageId, entry.view);
		else if (!LSPPlugin.get(entry.view)) {
			entry.view.dispatch({ effects: entry.compartment.reconfigure(client.plugin(uri(entry.path), entry.languageId)) });
		}
		this.refresh();
	}

	#pull() {
		const client = this.#client;
		if (!client) return;
		client.sync();
		for (const file of client.workspace.files as SharedFile[]) {
			const { doc, version } = file;
			client.request<object, Report>('textDocument/diagnostic', { textDocument: { uri: file.uri } }).then(
				(report) => {
					if (report.kind !== 'full' || client !== this.#client || file.version !== version) return;
					for (const view of file.views) this.#show(view, report, doc);
				},
				() => {}
			);
		}
	}

	#show(view: EditorView, report: Report, doc: Text) {
		let at = (position: Position) => offset(doc, position);
		if (this.primary) {
			const plugin = LSPPlugin.get(view);
			if (!plugin) return;
			if (!plugin.syncedDoc.eq(doc)) return publish(view, this.channel, []);
			at = (position) => plugin.unsyncedChanges.mapPos(offset(plugin.syncedDoc, position));
		} else if (!view.state.doc.eq(doc)) return;
		const diagnostics = report.items.map(({ range, severity, tags, code, source, message }) => ({
			from: at(range.start),
			to: at(range.end),
			severity: severities[severity ?? 1],
			markClass: tags?.map((tag) => tagClasses[tag]).filter(Boolean).join(' '),
			source: this.primary ? undefined : [source, code].filter(Boolean).join(' '),
			message
		}));
		publish(view, this.channel, diagnostics);
	}
}

function publish(view: EditorView, channel: string, diagnostics: Diagnostic[]) {
	const kept: Diagnostic[] = [];
	forEachDiagnostic(view.state, (diagnostic, from, to) => {
		const owner = owners.get(diagnostic);
		if (owner === channel) return;
		const moved = { ...diagnostic, from, to };
		if (owner) owners.set(moved, owner);
		kept.push(moved);
	});
	for (const diagnostic of diagnostics) owners.set(diagnostic, channel);
	view.dispatch(setDiagnostics(view.state, [...kept, ...diagnostics]));
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

	edited(view: EditorView) {
		const file = this.files.find((file) => file.views.includes(view));
		if (!file || file.views[0] === view) return;
		file.views = [view, ...file.views.filter((other) => other !== view)];
		file.replaced = true;
	}

	syncFiles() {
		const updates = [];
		const primary = this.#session.primary;
		for (const file of this.files) {
			const main = file.views[0];
			const plugin = LSPPlugin.get(main);
			if (primary ? !plugin : main.state.doc === file.doc) continue;
			const prevDoc = file.doc;
			// The last edited view owns the document, and other views of the file may not match it.
			// Diagnostics-only sessions have no plugin tracking their changes.
			const changes =
				primary && plugin && !file.replaced
					? plugin.unsyncedChanges
					: ChangeSet.of({ from: 0, to: prevDoc.length, insert: main.state.doc }, prevDoc.length);
			file.replaced = false;
			if (changes.empty) continue;
			file.doc = main.state.doc;
			file.version++;
			if (primary) for (const view of file.views) LSPPlugin.get(view)?.clear();
			updates.push({ file, prevDoc, changes });
		}
		return updates;
	}

	openFile(uri: string, languageId: string, view: EditorView) {
		const file = this.files.find((file) => file.uri === uri);
		if (file) {
			if (!file.views.includes(view)) file.views.push(view);
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
