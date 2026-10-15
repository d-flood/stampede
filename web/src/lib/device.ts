import { loadConfig } from '#lib/config.svelte.ts';

export type ServerMessage =
	| { type: 'hello'; version: string; terminals: string[] }
	| { type: 'terminal.attached'; terminalId: string; channel: number; cols: number; rows: number }
	| { type: 'terminal.resized'; terminalId: string; cols: number; rows: number }
	| { type: 'config.changed' };

export type ClientMessage =
	| { type: 'hello'; deviceId: string }
	| { type: 'terminal.create' }
	| { type: 'terminal.attach'; terminalId: string }
	| { type: 'terminal.resize'; terminalId: string; cols: number; rows: number }
	| { type: 'terminal.focus'; terminalId: string }
	| { type: 'terminal.detach'; channel: number };

export type Device = {
	send(message: ClientMessage): void;
	input(channel: number, data: string): void;
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

export function connect(handlers: {
	message(message: ServerMessage): void;
	output(channel: number, bytes: Uint8Array): void;
}): Device {
	const id = deviceId();
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
				return handlers.message(message);
			}
			const view = new DataView(data);
			if (view.getUint8(0) === OUTPUT) handlers.output(view.getUint32(1), new Uint8Array(data, 5));
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

	function close() {
		closed = true;
		socket.close();
	}

	open();
	return { send, input, close };
}
