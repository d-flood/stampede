import { loadConfig, type Agent } from '#lib/config.svelte.ts';

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

export type Attention = { projectId: string; terminalId: string; label: string; status: Status };

export type ServerMessage =
	| { type: 'hello'; version: string; terminals: Terminal[] }
	| { type: 'terminal.created'; terminal: Terminal }
	| { type: 'terminals.changed'; projectId?: string; drawer?: true; terminals: Terminal[] }
	| { type: 'terminal.status'; terminalId: string; status: Status }
	| { type: 'terminal.title'; terminalId: string; label: string }
	| { type: 'terminal.attached'; terminalId: string; channel: number; cols: number; rows: number }
	| { type: 'terminal.resized'; terminalId: string; cols: number; rows: number }
	| { type: 'config.changed' }
	| { type: 'projects.changed'; projects: Project[] }
	| { type: 'project.opened'; projectId: string }
	| { type: 'project.error'; path: string; error: string }
	| { type: 'picker.listed'; directories: string[] }
	| { type: 'fs.changed'; projectId: string; paths: string[]; git?: true }
	| { type: 'attention.changed'; items: Attention[] }
	| { type: 'git.changed'; projectId: string };

export type ClientMessage =
	| { type: 'hello'; deviceId: string }
	| { type: 'terminal.create'; projectId: string; rig?: string }
	| { type: 'terminal.create'; owner: 'drawer' }
	| { type: 'terminal.close'; terminalId: string }
	| { type: 'terminal.resume'; terminalId: string }
	| { type: 'terminal.attach'; terminalId: string }
	| { type: 'terminal.resize'; terminalId: string; cols: number; rows: number }
	| { type: 'terminal.focus'; terminalId: string }
	| { type: 'terminal.detach'; channel: number }
	| { type: 'projects.list' }
	| { type: 'project.open'; path: string }
	| { type: 'project.close'; projectId: string }
	| { type: 'project.reorder'; order: string[] }
	| { type: 'picker.list' }
	| { type: 'device.view'; terminalId: string | null };

type Listener = {
	message?(message: ServerMessage): void;
	output?(channel: number, bytes: Uint8Array): void;
};

export type Device = {
	send(message: ClientMessage): void;
	input(channel: number, data: string): void;
	listen(listener: Listener): () => void;
	close(): void;
};

const OUTPUT = 1;
const INPUT = 2;

function deviceId(): string {
	let id = sessionStorage.getItem('deviceId');
	if (!id) {
		id = crypto.randomUUID();
		sessionStorage.setItem('deviceId', id);
	}
	return id;
}

export function connect(): Device {
	const id = deviceId();
	const listeners = new Set<Listener>();
	let socket: WebSocket;
	let closed = false;

	function open() {
		socket = new WebSocket(new URL('/ws', location.href.replace(/^http/, 'ws')));
		socket.binaryType = 'arraybuffer';
		socket.onopen = () => send({ type: 'hello', deviceId: id });
		socket.onmessage = ({ data }) => {
			if (typeof data === 'string') {
				const message: ServerMessage = JSON.parse(data);
				if (message.type === 'config.changed') loadConfig();
				for (const listener of listeners) listener.message?.(message);
				return;
			}
			const view = new DataView(data);
			if (view.getUint8(0) !== OUTPUT) return;
			const bytes = new Uint8Array(data, 5);
			for (const listener of listeners) listener.output?.(view.getUint32(1), bytes);
		};
		socket.onclose = () => {
			if (!closed) setTimeout(open, 1000);
		};
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
		closed = true;
		socket.close();
	}

	open();
	return { send, input, listen, close };
}
