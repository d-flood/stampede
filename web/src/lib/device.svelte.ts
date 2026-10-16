import { loadConfig, type Agent } from '#lib/config.svelte.ts';
import type { State } from '#lib/editor/lsp.svelte.ts';
import { on } from 'svelte/events';

export type Project = { id: string; path: string };

export type Status = 'working' | 'waiting' | 'done';

export type Terminal = {
	id: string;
	owner: { project: string } | 'drawer';
	rig?: string;
	agent?: Agent;
	status?: Status;
	agentSession?: string;
	label: string;
	state: 'live' | 'ended';
};

export type ToolCall = { kind: 'read' | 'search' | 'fetch' | 'web' | 'other'; name: string; detail: string };

export type Item =
	| { kind: 'user'; text: string; images: number }
	| { kind: 'text'; text: string; streaming: boolean }
	| { kind: 'thinking'; seconds: number; text?: string }
	| { kind: 'tools'; calls: ToolCall[] }
	| { kind: 'command'; command: string; status: 'running' | 'ok' | 'failed'; output: string }
	| { kind: 'notice'; notice: 'compacted' | 'error' | 'limit'; text: string };

export type Context = { used: number; window: number };

export type Tokens = { input: number; output: number };

export type Chat = {
	id: string;
	projectId: string;
	rig: string;
	agent: Agent;
	status: Status;
	agentSession?: string;
	context: Context | null;
	tokens: Tokens | null;
};

export type Attention = { projectId: string; label: string; status: Status } & ({ terminalId: string } | { chatId: string });

export function tabOf(item: Attention): string {
	return 'chatId' in item ? item.chatId : item.terminalId;
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
	| { type: 'hello'; version: string; terminals: Terminal[]; chats: Chat[] }
	| { type: 'terminal.created'; terminal: Terminal }
	| { type: 'terminals.changed'; projectId?: string; drawer?: true; terminals: Terminal[] }
	| { type: 'terminal.status'; terminalId: string; status: Status }
	| { type: 'terminal.title'; terminalId: string; label: string }
	| { type: 'terminal.attached'; terminalId: string; channel: number; cols: number; rows: number }
	| { type: 'terminal.resized'; terminalId: string; cols: number; rows: number }
	| { type: 'chat.created'; chat: Chat }
	| { type: 'chats.changed'; projectId: string; chats: Chat[] }
	| { type: 'chat.items'; chatId: string; items: Item[] }
	| { type: 'chat.item'; chatId: string; index: number; item: Item }
	| { type: 'chat.status'; chatId: string; status: Status }
	| { type: 'chat.meters'; chatId: string; context: Context | null; tokens: Tokens | null }
	| { type: 'config.changed' }
	| { type: 'projects.changed'; projects: Project[] }
	| { type: 'project.opened'; projectId: string }
	| { type: 'project.error'; path: string; error: string }
	| { type: 'picker.listed'; directories: string[] }
	| { type: 'fs.changed'; projectId: string; paths: string[]; git?: true }
	| { type: 'attention.changed'; items: Attention[] }
	| { type: 'git.changed'; projectId: string }
	| { type: 'search.started'; searchId: string }
	| { type: 'search.results'; searchId: string; file: string; matches: Found[] }
	| { type: 'search.done'; searchId: string; truncated: boolean; error?: string }
	| { type: 'lsp.switch'; projectId: string; on: boolean }
	| { type: 'lsp.state'; channel: string; state: State }
	| { type: 'lsp'; channel: string; message: string };

export type ClientMessage =
	| { type: 'hello'; deviceId: string; browserId: string }
	| { type: 'terminal.create'; projectId: string; rig?: string }
	| { type: 'terminal.create'; owner: 'drawer' }
	| { type: 'terminal.close'; terminalId: string }
	| { type: 'terminal.resume'; terminalId: string }
	| { type: 'terminal.attach'; terminalId: string }
	| { type: 'terminal.resize'; terminalId: string; cols: number; rows: number }
	| { type: 'terminal.focus'; terminalId: string }
	| { type: 'terminal.detach'; channel: number }
	| { type: 'chat.create'; projectId: string; rig: string }
	| { type: 'chat.send'; chatId: string; text: string }
	| { type: 'chat.close' | 'chat.attach' | 'chat.detach'; chatId: string }
	| { type: 'projects.list' }
	| { type: 'project.open'; path: string }
	| { type: 'project.close'; projectId: string }
	| { type: 'project.reorder'; order: string[] }
	| { type: 'picker.list' }
	| { type: 'device.view'; terminalIds: string[]; chatIds: string[] }
	| ({ type: 'search.start' } & Search)
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
