<script lang="ts">
	import { connect, type Device } from '#lib/device.ts';
	import { FitAddon, Terminal, init } from 'ghostty-web';
	import { onMount } from 'svelte';

	let container: HTMLDivElement;
	let device: Device | undefined;
	let view: Terminal | undefined;
	let fit: FitAddon | undefined;
	let current: { terminalId: string; channel: number } | undefined;
	let writing = false;

	onMount(() => {
		let disposed = false;
		init().then(() => {
			if (!disposed) start();
		});
		return () => {
			disposed = true;
			device?.close();
			view?.dispose();
		};
	});

	function start() {
		view = window.terminal = new Terminal();
		fit = new FitAddon();
		view.loadAddon(fit);
		view.open(container);
		view.onData((data) => {
			if (current && !writing) device?.input(current.channel, data);
		});
		container.addEventListener('focusin', focus);
		container.addEventListener('pointerdown', focus);
		new ResizeObserver(propose).observe(container);
		device = connect({
			message(message) {
				switch (message.type) {
					case 'hello': {
						const terminals = message.terminals;
						const terminalId = terminals.find((id) => id === current?.terminalId) ?? terminals.at(-1);
						if (terminalId) device?.send({ type: 'terminal.attach', terminalId });
						break;
					}
					case 'terminal.attached':
						current = { terminalId: message.terminalId, channel: message.channel };
						view?.resize(message.cols, message.rows);
						write('\x1bc');
						propose();
						break;
					case 'terminal.resized':
						if (message.terminalId === current?.terminalId) view?.resize(message.cols, message.rows);
						break;
				}
			},
			output(channel, bytes) {
				if (channel === current?.channel) write(bytes);
			}
		});
	}

	// Query responses come from the holder; ghostty-web's own ones are
	// emitted synchronously inside write() and dropped by onData.
	function write(data: string | Uint8Array) {
		writing = true;
		view?.write(data);
		writing = false;
	}

	function propose() {
		const size = fit?.proposeDimensions();
		if (current && size) device?.send({ type: 'terminal.resize', terminalId: current.terminalId, ...size });
	}

	function focus() {
		if (current) device?.send({ type: 'terminal.focus', terminalId: current.terminalId });
	}

	function create() {
		current = undefined;
		write('\x1bc');
		device?.send({ type: 'terminal.create' });
		view?.focus();
	}
</script>

<main>
	<button onclick={create}>New Terminal</button>
	<div class="terminal" bind:this={container}></div>
</main>

<style>
	:global(body) {
		margin: 0;
	}

	main {
		display: grid;
		grid-template-rows: auto 1fr;
		height: 100dvh;
	}

	button {
		justify-self: start;
	}

	.terminal {
		min-height: 0;
		overflow: auto;
	}
</style>
