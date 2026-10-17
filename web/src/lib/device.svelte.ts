import { loadConfig, type Agent } from '#lib/config.svelte.ts';
import type { State } from '#lib/editor/lsp.svelte.ts';
import { on } from 'svelte/events';

export type Project = { id: string; path: string; worktree?: Worktree };

export type Worktree = { parent: string; epic: string; role: 'integration' | { ticket: number } };

export type Status = 'working' | 'waiting' | 'done';

export type Terminal = {
	id: string;
	owner: { project: string } | 'drawer';
	label: string;
	state: 'live' | 'ended';
};

export type ToolCall = { kind: 'read' | 'search' | 'fetch' | 'web' | 'other'; name: string; detail: string };

export type Item =
	| { kind: 'user'; text: string; images: number; template?: string }
	| { kind: 'text'; text: string; streaming: boolean; phase: 'commentary' | 'final' }
	| { kind: 'thinking'; seconds: number; text?: string }
	| { kind: 'tools'; calls: ToolCall[] }
	| { kind: 'html'; title: string; html: string }
	| { kind: 'subagent'; name?: string; task: string; status: Status; activity?: string | null; items: Item[] }
	| { kind: 'edit'; path: string; diff: string; added: number; removed: number }
	| { kind: 'command'; command: string; status: 'running' | 'ok' | 'failed'; output: string }
	| { kind: 'notice'; notice: 'compacted' | 'error' | 'limit' | 'interrupted' | 'rewound'; text: string };

export type Request =
	| { kind: 'command'; id: string; words: string[][]; suggestion: string[][]; sessionPrefixes: boolean }
	| { kind: 'question'; id: string; questions: Question[] }
	| { kind: 'plan'; id: string; plan: string }
	| { kind: 'edit'; id: string; path: string; diff: string }
	| { kind: 'other'; id: string; title: string; detail: string };

export type Question = { id: string; header: string; question: string; options: { label: string; description: string; preview?: string }[]; multiSelect: boolean };
export type AnsweredQuestions = { after: number; questions: Question[]; answers: string[][] };
export type Image = { mediaType: string; data: string };
export type Answer = { allow: true; rule?: { prefix: string[]; scope: 'chat' | 'always' }[] } | { allow: false; instead?: string } | { answers: string[][] };
export type Todo = { text: string; status: 'pending' | 'active' | 'done' };

export type Context = { used: number; window: number };

export type Tokens = { input: number; output: number };

export type Setting = 'model' | 'effort' | 'mode';

export type Settings = Partial<Record<Setting, string>> & { pending?: Setting[] };

export type Choice = { value: string; label: string };

export type Capabilities = {
	rewindsFiles: boolean;
	models: string[];
	efforts: string[];
	modes: Choice[];
	commands: { name: string; description: string; hint: string }[];
};

export type Builtin = Setting | 'compact' | 'clear' | 'rewind';

export type Chat = {
	id: string;
	projectId: string;
	rig: string;
	agent: Agent;
	status: Status;
	agentSession?: string;
	activity?: string | null;
	context: Context | null;
	tokens: Tokens | null;
	queued: string[];
	todos: Todo[];
	answeredQuestions: AnsweredQuestions[];
	settings: Settings;
};

export type Reading = { agent: Agent; window: 'five-hour' | 'weekly'; used: number; resetsAt?: number; readAt: number };

export type Session = { agent: Agent; sessionId: string; firstPrompt: string; updatedAt: number };

export type TicketStatus = 'not-started' | 'in-progress' | 'merging' | 'merged' | 'skipped';

export type Note = { author: 'agent' | 'david' | 'oikonomos'; text: string; at: number };

export type Claim = { chat: string; rig: string; edited?: boolean };

export type Ticket = {
	number: number;
	title: string;
	body: string;
	blockers: number[];
	status: TicketStatus;
	notes: Note[];
	claim?: Claim;
	worktree?: string;
	attention?: string;
	resolver?: string;
	releasing?: boolean;
};

export type Run = { rig: string; parallel: number; paused?: { reason: 'david' } | { reason: 'usage'; agent: Agent; window: 'five-hour' | 'weekly'; resetsAt?: number } };

export type Epic = {
	slug: string;
	title: string;
	brief: string;
	setup: string | null;
	archived: boolean;
	base?: string;
	integration?: string;
	run?: Run;
	landed?: { base: string; head: string };
	kept?: string;
	notes?: Note[];
	tickets: Ticket[];
};

export type Attention = { projectId: string; label: string; status: Status } & ({ chatId: string } | { epic: string });

export function tabOf(item: Attention): string {
	return 'chatId' in item ? item.chatId : `epic:${item.projectId}:${item.epic}`;
}

export type Found = { line: number; column: number; preview: string };

export type Search = {
	searchId: string;
	projectId: string;
	query: string;
	regex: boolean;
	caseSensitive: boolean;
	wholeWord: boolean;
	include: string[];
	exclude: string[];
	includeIgnored: boolean;
};

export type ServerMessage =
	| { type: 'hello'; version: string; buildId: string; terminals: Terminal[]; chats: Chat[] }
	| { type: 'terminal.created'; terminal: Terminal }
	| { type: 'terminals.changed'; projectId?: string; drawer?: true; terminals: Terminal[] }
	| { type: 'terminal.title'; terminalId: string; label: string }
	| { type: 'terminal.attached'; terminalId: string; channel: number; cols: number; rows: number }
	| { type: 'terminal.resized'; terminalId: string; cols: number; rows: number }
	| { type: 'chat.created'; chat: Chat }
	| { type: 'chats.changed'; projectId: string; chats: Chat[] }
	| { type: 'chat.items'; chatId: string; items: Item[] }
	| { type: 'chat.item'; chatId: string; index: number; item: Item }
	| { type: 'chat.status'; chatId: string; status: Status }
	| { type: 'chat.activity'; chatId: string; activity: string | null }
	| { type: 'chat.meters'; chatId: string; context: Context | null; tokens: Tokens | null }
	| { type: 'sessions.listed'; projectId: string; sessions: Session[] }
	| { type: 'chat.todos'; chatId: string; todos: Todo[] }
	| { type: 'chat.queued'; chatId: string; queued: string[] }
	| { type: 'chat.request'; chatId: string; request: Request }
	| { type: 'chat.resolved'; chatId: string; requestId: string }
	| { type: 'chat.settings'; chatId: string; settings: Settings }
	| { type: 'chat.capabilities'; chatId: string; capabilities: Capabilities }
	| { type: 'config.changed' }
	| { type: 'projects.changed'; projects: Project[] }
	| { type: 'project.opened'; projectId: string }
	| { type: 'project.error'; path: string; error: string }
	| { type: 'picker.listed'; directories: string[] }
	| { type: 'fs.changed'; projectId: string; paths: string[]; git?: true }
	| { type: 'attention.changed'; items: Attention[] }
	| { type: 'usage.changed'; readings: Reading[] }
	| { type: 'git.changed'; projectId: string }
	| { type: 'epics.changed'; projectId: string; epics: Epic[] }
	| { type: 'epic.changed'; projectId: string; epic: Epic }
	| { type: 'ticket.started'; projectId: string; epic: string; ticket: number; chatId: string }
	| { type: 'ticket.error'; projectId: string; epic: string; ticket: number; error: string }
	| { type: 'search.started'; searchId: string }
	| { type: 'search.results'; searchId: string; file: string; matches: Found[] }
	| { type: 'search.done'; searchId: string; truncated: boolean; error?: string }
	| { type: 'lsp.switch'; projectId: string; on: boolean }
	| { type: 'lsp.state'; channel: string; state: State }
	| { type: 'lsp'; channel: string; message: string };

export type ClientMessage =
	| { type: 'hello'; deviceId: string; browserId: string }
	| { type: 'terminal.create'; projectId: string }
	| { type: 'terminal.create'; owner: 'drawer' }
	| { type: 'terminal.close'; terminalId: string }
	| { type: 'terminal.attach'; terminalId: string }
	| { type: 'terminal.resize'; terminalId: string; cols: number; rows: number }
	| { type: 'terminal.focus'; terminalId: string }
	| { type: 'terminal.detach'; channel: number }
	| { type: 'chat.create'; projectId: string; rig: string }
	| { type: 'chat.resume'; projectId: string; agent: Agent; sessionId: string }
	| { type: 'sessions.list'; projectId: string }
	| { type: 'chat.send'; chatId: string; text: string; queue: boolean; images?: Image[] }
	| { type: 'chat.close' | 'chat.attach' | 'chat.detach' | 'chat.interrupt'; chatId: string }
	| { type: 'chat.answer'; chatId: string; requestId: string; answer: Answer }
	| { type: 'chat.run'; chatId: string; command: Builtin; value?: string | number }
	| { type: 'projects.list' }
	| { type: 'project.open'; path: string }
	| { type: 'project.close'; projectId: string }
	| { type: 'project.reorder'; order: string[] }
	| { type: 'picker.list' }
	| { type: 'device.view'; chatIds: string[]; epics: string[] }
	| ({ type: 'search.start' } & Search)
	| { type: 'epics.subscribe'; projectId: string }
	| { type: 'ticket.start'; projectId: string; epic: string; ticket: number; rig: string }
	| { type: 'lsp.switch'; projectId: string; on: boolean }
	| { type: 'lsp.open' | 'lsp.close'; channel: string }
	| { type: 'lsp'; channel: string; message: string };

type Listener = {
	message?(message: ServerMessage): void;
	output?(channel: number, bytes: Uint8Array): void;
};

export type Connection = 'connected' | 'reconnecting';

export type Device = {
	readonly connection: Connection;
	send(message: ClientMessage): void;
	input(channel: number, data: string): void;
	listen(listener: Listener): () => void;
	close(): void;
};

const OUTPUT = 1;
const INPUT = 2;
const FIRST_RETRY = 500;
const LAST_RETRY = 5000;

export function deviceId(): string {
	let id = sessionStorage.getItem('deviceId');
	if (!id) {
		id = crypto.randomUUID();
		sessionStorage.setItem('deviceId', id);
	}
	return id;
}

let claimed: Promise<void> | undefined;

export function claimDeviceId(): Promise<void> {
	return (claimed ??= new Promise((resolve) => {
		const channel = new BroadcastChannel('oikonomos-windows');
		const token = crypto.randomUUID();
		let ready = false;
		let occupied = false;
		channel.onmessage = ({ data }) => {
			if (data.type === 'probe' && data.deviceId === deviceId() && (ready || token < data.token)) {
				channel.postMessage({ type: 'occupied', token: data.token });
			} else if (data.type === 'occupied' && data.token === token) occupied = true;
		};
		channel.postMessage({ type: 'probe', deviceId: deviceId(), token });
		setTimeout(() => {
			if (occupied) sessionStorage.setItem('deviceId', crypto.randomUUID());
			ready = true;
			resolve();
		}, 50);
	}));
}

export function browserId(): string {
	let id = localStorage.getItem('browserId');
	if (!id) {
		id = crypto.randomUUID();
		localStorage.setItem('browserId', id);
	}
	return id;
}

export function connect(): Device {
	const id = deviceId();
	const listeners = new Set<Listener>();
	let socket: WebSocket;
	let connection = $state<Connection>('connected');
	let delay = FIRST_RETRY;
	let retry: ReturnType<typeof setTimeout> | undefined;

	function open() {
		clearTimeout(retry);
		socket = new WebSocket(new URL('/ws', location.href.replace(/^http/, 'ws')));
		socket.binaryType = 'arraybuffer';
		socket.onopen = () => send({ type: 'hello', deviceId: id, browserId: browserId() });
		socket.onmessage = ({ data }) => {
			if (typeof data === 'string') {
				const message: ServerMessage = JSON.parse(data);
				if (message.type === 'hello') {
					connection = 'connected';
					delay = FIRST_RETRY;
				}
				if (message.type === 'config.changed') loadConfig();
				for (const listener of listeners) listener.message?.(message);
				return;
			}
			const view = new DataView(data);
			if (view.getUint8(0) !== OUTPUT) return;
			const bytes = new Uint8Array(data, 5);
			for (const listener of listeners) listener.output?.(view.getUint32(1), bytes);
		};
		socket.onclose = drop;
	}

	function drop() {
		socket.onclose = null;
		socket.close();
		connection = 'reconnecting';
		clearTimeout(retry);
		retry = setTimeout(open, delay);
		delay = Math.min(delay * 2, LAST_RETRY);
	}

	function wake() {
		if (connection === 'connected' || socket.readyState === WebSocket.OPEN) return;
		socket.onclose = null;
		socket.close();
		open();
	}

	function send(message: ClientMessage) {
		if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
	}

	function input(channel: number, data: string) {
		const bytes = new TextEncoder().encode(data);
		const frame = new Uint8Array(5 + bytes.length);
		const view = new DataView(frame.buffer);
		view.setUint8(0, INPUT);
		view.setUint32(1, channel);
		frame.set(bytes, 5);
		if (socket.readyState === WebSocket.OPEN) socket.send(frame);
	}

	function listen(listener: Listener) {
		listeners.add(listener);
		return () => listeners.delete(listener);
	}

	function close() {
		off.forEach((listener) => listener());
		clearTimeout(retry);
		socket.onclose = null;
		socket.close();
	}

	open();
	const off = [
		// A socket can outlive its network for minutes, so going offline drops it at once.
		on(window, 'offline', drop),
		on(window, 'online', wake),
		on(document, 'visibilitychange', () => document.visibilityState === 'visible' && wake())
	];
	return {
		get connection() {
			return connection;
		},
		send,
		input,
		listen,
		close
	};
}
