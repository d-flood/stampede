<script lang="ts">
	import { appearance, type Appearance } from '#lib/config.svelte.ts';
	import { connect, type Device, type ServerMessage } from '#lib/device.ts';
	import { FitAddon, Terminal, init } from 'ghostty-web';
	import { onMount } from 'svelte';

	let container: HTMLDivElement;
	let device: Device | undefined;
	let view: Terminal | undefined;
	let fit: FitAddon | undefined;
	let current: { terminalId: string; channel: number } | undefined;
	let writing = false;
	let ready = $state(false);
	let connected = $state(false);
	let shown = '';

	onMount(() => {
		let disposed = false;
		init().then(() => {
			if (!disposed) ready = true;
		});
		container.addEventListener('focusin', focus);
		container.addEventListener('pointerdown', focus);
		new ResizeObserver(propose).observe(container);
		return () => {
			disposed = true;
			device?.close();
			view?.dispose();
		};
	});

	$effect(() => {
		if (!ready) return;
		const next = appearance();
		const key = JSON.stringify(next);
		if (key === shown) return;
		let stale = false;
		document.fonts.load(`15px ${next.fontFamily}`).then(() => {
			if (stale) return;
			shown = key;
			open(next);
		});
		return () => {
			stale = true;
		};
	});

	// ghostty-web can't change its theme after open(), so a new look gets a
	// new view, re-attached for a fresh snapshot.
	function open({ theme, fontFamily }: Appearance) {
		view?.dispose();
		const element = document.createElement('div');
		container.replaceChildren(element);
		view = window.terminal = new Terminal({ theme, fontFamily });
		fit = new FitAddon();
		view.loadAddon(fit);
		view.open(element);
		view.onData((data) => {
			if (current && !writing) device?.input(current.channel, data);
		});
		if (!device) {
			device = connect({ message, output });
		} else if (current) {
			device.send({ type: 'terminal.detach', channel: current.channel });
			device.send({ type: 'terminal.attach', terminalId: current.terminalId });
			current = undefined;
		}
	}

	function message(message: ServerMessage) {
		switch (message.type) {
			case 'hello': {
				connected = true;
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
	}

	function output(channel: number, bytes: Uint8Array) {
		if (channel === current?.channel) write(bytes);
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
	<button onclick={create} disabled={!connected}>New Terminal</button>
	<div class="terminal" bind:this={container}></div>
</main>

<style>
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

	.terminal > :global(div) {
		height: 100%;
	}
</style>
