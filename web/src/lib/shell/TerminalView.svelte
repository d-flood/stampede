<script lang="ts" module>
	import { init } from 'ghostty-web';

	const ghostty = init();
</script>

<script lang="ts">
	import { appearance, type Appearance } from '#lib/config.svelte.ts';
	import type { Device, Project, ServerMessage } from '#lib/device.svelte.ts';
	import type { Cursor } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { linkify } from '#lib/links.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import Composer from '#lib/shell/Composer.svelte';
	import CopyMode from '#lib/shell/CopyMode.svelte';
	import KeyBar, { control } from '#lib/shell/KeyBar.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Button from '#lib/ui/Button.svelte';
	import { FitAddon, Terminal } from 'ghostty-web';
	import { onMount, type Snippet } from 'svelte';
	import ArrowElbowDownLeft from '~icons/ph/arrow-elbow-down-left';
	import Clipboard from '~icons/ph/clipboard';
	import Copy from '~icons/ph/copy';
	import NotePencil from '~icons/ph/note-pencil';

	type Props = {
		device: Device;
		terminalId: string;
		pending?: string;
		active: boolean;
		displayed?: boolean;
		focused: boolean;
		ended?: boolean;
		project?: Project;
		onopen?(path: string, cursor?: Cursor): void;
		onbrowse?(url: string): void;
		children?: Snippet;
	};

	let { device, terminalId, pending, active, displayed = true, focused, ended = false, project, onopen, onbrowse, children }: Props = $props();

	let container: HTMLDivElement;
	let view = $state.raw<Terminal>();
	let fit: FitAddon | undefined;
	let channel: number | undefined;
	let size: { cols: number; rows: number } | undefined;
	let early: Uint8Array[] = [];
	let writing = false;
	let filled = $state(false);
	let shown = '';
	let typing = $state(false);
	let ctrl = $state(false);
	let copying = $state<string>();
	let composing = $state(false);
	let pasteFallback = $state(false);
	let composer = $state<Composer>();
	let dragged = $state<number>();
	let wheeled = 0;
	let hidden = $state(document.hidden);
	const live = $derived(device.connection === 'connected');
	const observer = new ResizeObserver(propose);

	onMount(() => {
		device.send({ type: 'terminal.attach', terminalId });
		const unlisten = device.listen({ message, output });
		container.addEventListener('focusin', lead);
		container.addEventListener('pointerdown', lead);
		container.addEventListener('keydown', keydown, true);
		container.addEventListener('paste', paste, true);
		return () => {
			container.removeEventListener('keydown', keydown, true);
			container.removeEventListener('paste', paste, true);
			observer.disconnect();
			unlisten();
			if (channel !== undefined) device.send({ type: 'terminal.detach', channel });
			view?.dispose();
		};
	});

	$effect(() => {
		const next = appearance();
		const key = JSON.stringify(next);
		if (key === shown) return;
		let stale = false;
		Promise.all([ghostty, document.fonts.load(`15px ${next.fontFamily}`)]).then(() => {
			if (stale) return;
			shown = key;
			open(next);
		});
		return () => {
			stale = true;
		};
	});

	$effect(() => {
		if (active && filled) painted(terminalId);
	});

	$effect(() => {
		if (active && displayed && !hidden) view?.resume();
		else view?.pause();
	});

	$effect(() => {
		if (!focused || !view) return;
		window.terminal = view;
		if (composing || pending) composer?.focus();
		else view.focus();
	});

	// ghostty-web can't change its theme after open(), so a new look gets a
	// new view, re-attached for a fresh snapshot.
	function open({ theme, fontFamily, fontSize, scrollback }: Appearance) {
		const replaced = view;
		replaced?.dispose();
		const element = document.createElement('div');
		container.replaceChildren(element);
		// ghostty-web measures scrollback in bytes; reserve 4 KB per configured row.
		const next = new Terminal({ ...size, theme, fontFamily, fontSize, disableStdin: ended, scrollback: scrollback * 4096 });
		fit = new FitAddon();
		next.loadAddon(fit);
		// ghostty-web's open() takes focus; only the focused effect may.
		const { focus } = next;
		next.focus = () => {};
		next.open(element);
		next.focus = focus;
		observer.disconnect();
		observer.observe(element);
		for (const input of [element, next.textarea]) {
			for (const name of ['autocomplete', 'autocorrect', 'autocapitalize']) input?.setAttribute(name, 'off');
			input?.setAttribute('spellcheck', 'false');
		}
		// ghostty-web reads beforeinput only on its textarea, so text a soft keyboard commits to the focused element would be lost.
		element.addEventListener('beforeinput', (event) => {
			if (event.target === element && event.inputType === 'insertText' && event.data && !event.isComposing) send(event.data);
		});
		if (onopen) linkify(next, () => project, onopen, onbrowse);
		next.attachCustomWheelEventHandler((event) => {
			if (!next.hasMouseTracking()) return false;
			scroll(lines(event), event);
			return true;
		});
		next.onData((data) => {
			if (!writing) send(data);
		});
		view = next;
		filled = false;
		if (!replaced) {
			early.splice(0).forEach(fill);
			return;
		}
		if (channel !== undefined) device.send({ type: 'terminal.detach', channel });
		channel = undefined;
		device.send({ type: 'terminal.attach', terminalId });
	}

	function message(message: ServerMessage) {
		switch (message.type) {
			case 'hello':
				channel = undefined;
				early = [];
				device.send({ type: 'terminal.attach', terminalId });
				break;
			case 'terminal.attached':
				if (message.terminalId !== terminalId) break;
				if (channel !== undefined) device.send({ type: 'terminal.detach', channel });
				channel = message.channel;
				size = { cols: message.cols, rows: message.rows };
				early = [];
				if (!view) break;
				view.resize(message.cols, message.rows);
				write('\x1bc');
				propose();
				break;
			case 'terminal.resized':
				if (message.terminalId !== terminalId) break;
				size = { cols: message.cols, rows: message.rows };
				view?.resize(message.cols, message.rows);
				break;
		}
	}

	function output(from: number, bytes: Uint8Array) {
		if (from !== channel) return;
		if (view) fill(bytes);
		else early.push(bytes);
	}

	function fill(bytes: Uint8Array) {
		write(bytes);
		filled = true;
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
		input(data);
	}

	function input(data: string) {
		if (channel !== undefined && live) device.input(channel, data);
	}

	// ghostty-web reports clicks but never the wheel, which it turns into arrow keys on the
	// alternate screen, so apps tracking the mouse, like Claude Code, get wheel reports from here.
	function scroll(amount: number, at?: { clientX: number; clientY: number }) {
		if (!view) return;
		const tracking = view.hasMouseTracking();
		if (!tracking && view.buffer.active.type === 'normal') return view.scrollLines(amount);
		wheeled += amount;
		const count = Math.trunc(wheeled);
		wheeled -= count;
		if (!count) return;
		if (!tracking) return input((count < 0 ? '\x1b[A' : '\x1b[B').repeat(Math.abs(count)));
		const button = count < 0 ? 64 : 65;
		const [col, row] = cell(at);
		const report = view.getMode(1006)
			? `\x1b[<${button};${col};${row}M`
			: `\x1b[M${String.fromCharCode(32 + button, 32 + col, 32 + row)}`;
		input(report.repeat(Math.abs(count)));
	}

	function grid() {
		return container.querySelector('canvas')?.getBoundingClientRect();
	}

	function cell(at?: { clientX: number; clientY: number }) {
		const rect = grid();
		if (!view) return [1, 1];
		if (!at || !rect) return [Math.ceil(view.cols / 2), Math.ceil(view.rows / 2)];
		const clamp = (value: number, max: number) => Math.min(max, Math.max(1, Math.floor(value) + 1));
		return [
			clamp(((at.clientX - rect.left) / rect.width) * view.cols, view.cols),
			clamp(((at.clientY - rect.top) / rect.height) * view.rows, view.rows)
		];
	}

	function rowHeight() {
		return (grid()?.height ?? 0) / (view?.rows ?? 1) || 1;
	}

	function lines({ deltaY, deltaMode }: WheelEvent) {
		if (deltaMode === WheelEvent.DOM_DELTA_LINE) return deltaY;
		if (deltaMode === WheelEvent.DOM_DELTA_PAGE) return deltaY * (view?.rows ?? 1);
		return deltaY / rowHeight();
	}

	function grab(event: PointerEvent) {
		dragged = event.clientY;
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
	}

	function drag({ clientY }: PointerEvent) {
		if (dragged === undefined) return;
		scroll((dragged - clientY) / rowHeight());
		dragged = clientY;
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
		if (channel === undefined || !view || !live || ended) return false;
		text = text.replace(/\r\n?/g, '\n');
		const pasted = view.hasBracketedPaste() ? `\x1b[200~${text}\x1b[201~` : text.replaceAll('\n', '\r');
		device.input(channel, enter ? `${pasted}\r` : pasted);
		return true;
	}

	function submit(text: string) {
		if (channel === undefined || !view || !live || ended || !text.trim()) return false;
		text = text.replace(/\r\n?/g, '\n');
		const pasted = view.hasBracketedPaste() ? `\x1b[200~${text}\x1b[201~` : text.replaceAll('\n', '\r');
		device.send({ type: 'terminal.submit', terminalId, text: `${pasted}\r` });
		return true;
	}

	function paste(event: ClipboardEvent) {
		event.preventDefault();
		event.stopPropagation();
		const text = event.clipboardData?.getData('text/plain');
		if (text) compose(text, false);
	}

	async function pasteClipboard() {
		try {
			if (compose(await navigator.clipboard.readText(), false)) {
				pasteFallback = false;
				view?.textarea?.focus();
			}
		} catch {
			pasteFallback = true;
			composing = true;
			composer?.focus();
		}
	}

	function newline() {
		if (ended) return;
		if (view?.hasBracketedPaste()) compose('\n', false);
		else input('\x1b[13;2u');
		view?.textarea?.focus();
	}

	function keydown(event: KeyboardEvent) {
		if (event.isComposing) return;
		const key = event.key.toLowerCase();
		if (key === 'c' && !event.altKey && ((event.ctrlKey && event.shiftKey && !event.metaKey) || (event.metaKey && !event.ctrlKey && !event.shiftKey))) {
			event.preventDefault();
			event.stopPropagation();
			const text = view?.getSelection();
			if (text) navigator.clipboard.writeText(text).catch(() => (copying = text));
		} else if (key === 'c' && event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey) {
			event.preventDefault();
			event.stopPropagation();
			if (!ended) input('\x03');
		} else if (ended) return;
		else if (key === 'v' && (event.ctrlKey || event.metaKey) && !event.altKey) {
			event.stopPropagation();
			view?.textarea?.focus();
		} else if (event.key === 'Enter' && event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey) {
			event.preventDefault();
			event.stopPropagation();
			newline();
		}
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

<svelte:document onvisibilitychange={() => (hidden = document.hidden)} />

<div class="panel" role="tabpanel" hidden={!active}>
	<div class="toolbar">
		{#if !ended}
			<IconButton label="Paste" title="Paste" disabled={!live} onclick={pasteClipboard}><Clipboard /></IconButton>
			<IconButton label="Newline" title="Insert newline (Shift+Enter)" disabled={!live} onclick={newline}>
				<ArrowElbowDownLeft />
			</IconButton>
			<IconButton label="Composer" aria-pressed={composing} onclick={() => (composing = !composing)}>
				<NotePencil />
			</IconButton>
		{:else}
			<IconButton label="Copy mode" onclick={copy}><Copy /></IconButton>
		{/if}
	</div>
	{@render children?.()}
	<div class="view">
		<div class="terminal" title="Copy selection (Ctrl+Shift+C or Cmd+C)" bind:this={container} onfocusin={() => (typing = true)} onfocusout={blur}></div>
		{#if form() === 'phone'}
			<div
				class="rail"
				class:dragging={dragged !== undefined}
				aria-hidden="true"
				onpointerdown={grab}
				onpointermove={drag}
				onpointerup={() => (dragged = undefined)}
				onpointercancel={() => (dragged = undefined)}
			></div>
		{/if}
	</div>
	{#if typing && !ended && form() === 'phone'}
		<KeyBar bind:ctrl disabled={!live} application={() => !!view?.getMode(1)} onkey={send} oncopy={copy} />
	{/if}
	{#if pending && !ended}
		<Composer bind:this={composer} id={`pending:${terminalId}`} draft={pending} code disabled={!live || !filled} onsend={submit}>
			{#snippet actions(text, clear)}
				<span role="status">Review command before running</span>
				<Button variant="primary" disabled={!text.trim() || !live || !filled} onclick={() => { if (submit(text)) clear(); }}>Run</Button>
			{/snippet}
		</Composer>
	{:else if composing && !ended}
		{#if pasteFallback}
			<p role="status">Clipboard access unavailable. Paste into Composer, then choose Insert.</p>
		{/if}
		<Composer bind:this={composer} id={terminalId} disabled={!live} onsend={compose} />
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

	.view {
		display: flex;
		flex: 1;
		min-height: 0;
	}

	.terminal {
		flex: 1;
		min-width: 0;
		min-height: 0;
		overflow: auto;
	}

	.rail {
		position: relative;
		z-index: 1;
		display: flex;
		flex: none;
		align-items: center;
		justify-content: center;
		width: calc(var(--target-min) / 2);
		border-left: var(--border-width) solid var(--border);
		background: var(--surface-1);
		touch-action: none;
	}

	.rail::before {
		content: '';
		width: var(--space-1);
		height: var(--target-min);
		border-radius: var(--radius-lg);
		background: var(--muted-fg);
	}

	.rail::after {
		content: '';
		position: absolute;
		inset: 0 0 0 calc(var(--target-min) / -2);
	}

	.rail.dragging {
		background: var(--hover);
	}

	.terminal > :global(div) {
		height: 100%;
	}
</style>
