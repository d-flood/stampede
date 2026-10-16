<script lang="ts">
	import { appearance, type Appearance } from '#lib/config.svelte.ts';
	import type { Device, Project, ServerMessage } from '#lib/device.ts';
	import type { Cursor } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { linkify } from '#lib/links.ts';
	import Composer from '#lib/shell/Composer.svelte';
	import CopyMode from '#lib/shell/CopyMode.svelte';
	import KeyBar, { control } from '#lib/shell/KeyBar.svelte';
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
	let typing = $state(false);
	let ctrl = $state(false);
	let copying = $state<string>();
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
		// ghostty-web's scrollback is a byte budget; 4 KB a row keeps Copy mode's 2,000 rows up to about 300 columns.
		const next = new Terminal({ theme, fontFamily, disableStdin: ended, scrollback: 2000 * 4096 });
		fit = new FitAddon();
		next.loadAddon(fit);
		next.open(element);
		for (const input of [element, next.textarea]) {
			for (const name of ['autocomplete', 'autocorrect', 'autocapitalize']) input?.setAttribute(name, 'off');
			input?.setAttribute('spellcheck', 'false');
		}
		// ghostty-web reads only keydown and compositionend, so text a soft keyboard commits directly would be lost.
		element.addEventListener('beforeinput', (event) => {
			if (event.inputType === 'insertText' && event.data && !event.isComposing) send(event.data);
		});
		if (onopen) linkify(next, () => project, onopen);
		next.onData((data) => {
			if (!writing) send(data);
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

	function send(data: string) {
		if (ctrl) {
			ctrl = false;
			data = control(data);
		}
		if (channel !== undefined) device.input(channel, data);
	}

	function blur({ relatedTarget }: FocusEvent) {
		typing = container.contains(relatedTarget as Node | null);
		if (!typing) ctrl = false;
	}

	function copy() {
		const buffer = view?.buffer.active;
		if (!view || !buffer) return;
		const lines = [];
		for (let y = Math.max(0, buffer.length - view.rows - 2000); y < buffer.length; y++) {
			const line = buffer.getLine(y);
			const cells = Array.from({ length: line?.length ?? 0 }, (_, x) => line?.getCell(x));
			// Blank cells read as '', and the second half of a wide character has width 0.
			lines.push(cells.map((cell) => (cell?.getWidth() ? cell.getChars() || ' ' : '')).join('').trimEnd());
		}
		copying = lines.join('\n').trimEnd();
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
	<div class="terminal" bind:this={container} onfocusin={() => (typing = true)} onfocusout={blur}></div>
	{#if typing && !ended && form() === 'phone'}
		<KeyBar bind:ctrl application={() => !!view?.getMode(1)} onkey={send} oncopy={copy} />
	{/if}
	{#if composing && !ended}
		<Composer bind:this={composer} {terminalId} onsend={compose} />
	{/if}
</div>

<CopyMode bind:text={copying} />

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
