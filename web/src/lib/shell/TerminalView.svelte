<script lang="ts">
	import { appearance, type Appearance } from '#lib/config.svelte.ts';
	import type { Device, Project, ServerMessage } from '#lib/device.ts';
	import type { Cursor } from '#lib/files.svelte.ts';
	import { linkify } from '#lib/links.ts';
	import Composer from '#lib/shell/Composer.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { FitAddon, Terminal, init } from 'ghostty-web';
	import { onMount, type Snippet } from 'svelte';
	import NotePencil from '~icons/ph/note-pencil';

	type Props = {
		device: Device;
		terminalId: string;
		active: boolean;
		focused: boolean;
		ended?: boolean;
		project?: Project;
		onopen?(path: string, cursor?: Cursor): void;
		children?: Snippet;
	};

	let { device, terminalId, active, focused, ended = false, project, onopen, children }: Props = $props();

	let container: HTMLDivElement;
	let view = $state.raw<Terminal>();
	let fit: FitAddon | undefined;
	let channel: number | undefined;
	let writing = false;
	let ready = $state(false);
	let shown = '';
	let composing = $state(false);
	let composer = $state<Composer>();

	onMount(() => {
		let disposed = false;
		init().then(() => {
			if (!disposed) ready = true;
		});
		const unlisten = device.listen({ message, output });
		container.addEventListener('focusin', lead);
		container.addEventListener('pointerdown', lead);
		const observer = new ResizeObserver(propose);
		observer.observe(container);
		return () => {
			disposed = true;
			observer.disconnect();
			unlisten();
			if (channel !== undefined) device.send({ type: 'terminal.detach', channel });
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

	$effect(() => {
		if (!focused || !view) return;
		window.terminal = view;
		if (composing) composer?.focus();
		else view.focus();
	});

	// ghostty-web can't change its theme after open(), so a new look gets a
	// new view, re-attached for a fresh snapshot.
	function open({ theme, fontFamily }: Appearance) {
		view?.dispose();
		const element = document.createElement('div');
		container.replaceChildren(element);
		const next = new Terminal({ theme, fontFamily, disableStdin: ended });
		fit = new FitAddon();
		next.loadAddon(fit);
		next.open(element);
		if (onopen) linkify(next, () => project, onopen);
		next.onData((data) => {
			if (channel !== undefined && !writing) device.input(channel, data);
		});
		view = next;
		if (channel !== undefined) device.send({ type: 'terminal.detach', channel });
		channel = undefined;
		device.send({ type: 'terminal.attach', terminalId });
	}

	function message(message: ServerMessage) {
		switch (message.type) {
			case 'hello':
				channel = undefined;
				if (view) device.send({ type: 'terminal.attach', terminalId });
				break;
			case 'terminal.attached':
				if (message.terminalId !== terminalId) break;
				if (channel !== undefined) device.send({ type: 'terminal.detach', channel });
				channel = message.channel;
				view?.resize(message.cols, message.rows);
				write('\x1bc');
				propose();
				break;
			case 'terminal.resized':
				if (message.terminalId === terminalId) view?.resize(message.cols, message.rows);
				break;
		}
	}

	function output(from: number, bytes: Uint8Array) {
		if (from === channel) write(bytes);
	}

	// Query responses come from the holder; ghostty-web's own ones are
	// emitted synchronously inside write() and dropped by onData.
	function write(data: string | Uint8Array) {
		writing = true;
		view?.write(data);
		writing = false;
	}

	function compose(text: string, enter: boolean) {
		if (channel === undefined || !view) return false;
		const pasted = view.hasBracketedPaste() ? `\x1b[200~${text}\x1b[201~` : text.replaceAll('\n', '\r');
		device.input(channel, enter ? `${pasted}\r` : pasted);
		return true;
	}

	function propose() {
		const size = fit?.proposeDimensions();
		if (channel !== undefined && size) device.send({ type: 'terminal.resize', terminalId, ...size });
	}

	function lead() {
		device.send({ type: 'terminal.focus', terminalId });
	}

	export function focus() {
		view?.focus();
	}
</script>

<div class="panel" role="tabpanel" hidden={!active}>
	{#if !ended}
		<div class="toolbar">
			<IconButton label="Composer" aria-pressed={composing} onclick={() => (composing = !composing)}>
				<NotePencil />
			</IconButton>
		</div>
	{/if}
	{@render children?.()}
	<div class="terminal" bind:this={container}></div>
	{#if composing && !ended}
		<Composer bind:this={composer} {terminalId} onsend={compose} />
	{/if}
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.panel[hidden] {
		display: none;
	}

	.toolbar {
		display: flex;
		flex: none;
		justify-content: flex-end;
		padding: 0 var(--space-1);
		border-bottom: var(--border-width) solid var(--border);
	}

	.toolbar :global([aria-pressed='true']) {
		background: var(--hover);
		color: var(--color-fg);
	}

	.terminal {
		flex: 1;
		min-height: 0;
		overflow: auto;
	}

	.terminal > :global(div) {
		height: 100%;
	}
</style>
