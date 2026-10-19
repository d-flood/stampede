<script lang="ts">
	import type { OpenBrowser } from '#lib/browser.svelte.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { onMount, tick } from 'svelte';
	import ArrowClockwise from '~icons/ph/arrow-clockwise';
	import ArrowSquareOut from '~icons/ph/arrow-square-out';

	let { browser, focused = false }: { browser: OpenBrowser; focused?: boolean } = $props();
	let address = $state('http://localhost:5173');
	let src = $state('');
	let pending = $state(false);
	let error = $state('');
	let revision = $state(0);
	let controller: AbortController | undefined;
	let iframe = $state<HTMLIFrameElement>();
	let output = $state<HTMLDivElement>();
	let consoleOpen = $state(false);
	let consoleReady = $state(false);
	type Entry = { level: string; text: string; time: number; id: number };
	let entries = $state.raw<Entry[]>([]);
	let sequence = 0;

	onMount(() => {
		address = browser.url || address;
		painted(browser.id);
		window.addEventListener('message', receive);
		if (browser.url) navigate();
		return () => {
			controller?.abort();
			window.removeEventListener('message', receive);
		};
	});

	function isEntry(value: unknown): value is Omit<Entry, 'id'> {
		return typeof value === 'object' && value !== null
			&& 'level' in value && typeof value.level === 'string' && ['log', 'debug', 'info', 'warn', 'error', 'trace'].includes(value.level)
			&& 'text' in value && typeof value.text === 'string' && value.text.length <= 8000
			&& 'time' in value && typeof value.time === 'number' && Number.isFinite(value.time);
	}

	function receive(event: MessageEvent<unknown>) {
		if (!src || event.source !== iframe?.contentWindow || event.origin !== new URL(src).origin) return;
		const data = event.data;
		if (typeof data !== 'object' || data === null || !('type' in data) || data.type !== 'stampede.console' || !('kind' in data)) return;
		if (data.kind === 'ready') {
			entries = [];
			consoleReady = true;
		} else if (data.kind === 'disconnected') consoleReady = false;
		else if (data.kind === 'clear') entries = [];
		else if (data.kind === 'entries' && 'entries' in data && Array.isArray(data.entries) && data.entries.length <= 101 && data.entries.every(isEntry)) {
			const follow = !output || output.scrollHeight - output.scrollTop - output.clientHeight < 30;
			entries = [...entries, ...data.entries.map((entry) => ({ ...entry, id: sequence++ }))].slice(-500);
			if (follow) scrollToEnd();
		}
	}

	async function scrollToEnd() {
		await tick();
		if (output) output.scrollTop = output.scrollHeight;
	}

	function toggleConsole() {
		consoleOpen = !consoleOpen;
		if (consoleOpen) scrollToEnd();
	}

	function reloadPreview() {
		consoleReady = false;
		entries = [];
		revision++;
	}

	async function navigate() {
		controller?.abort();
		const request = new AbortController();
		controller = request;
		pending = true;
		error = '';
		try {
			const response = await fetch('/api/browser', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ url: address.trim() }),
				signal: request.signal
			});
			if (!response.ok) throw new Error(await response.text());
			const preview: { url: string; src: string } = await response.json();
			if (request.signal.aborted) return;
			browser.url = address = preview.url;
			src = preview.src;
			reloadPreview();
		} catch (cause) {
			if (!request.signal.aborted) error = cause instanceof Error ? cause.message : 'Cannot open preview.';
		} finally {
			if (!request.signal.aborted) pending = false;
		}
	}
</script>

<div class="browser" role="tabpanel" aria-label="Browser">
	<form onsubmit={(event) => { event.preventDefault(); navigate(); }}>
		<IconButton label="Reload preview" type="button" disabled={!src || pending} onclick={reloadPreview}><ArrowClockwise /></IconButton>
		<input aria-label="Host address" bind:value={address} placeholder="http://localhost:5173" spellcheck="false" autocapitalize="off" autocomplete="off" inputmode="url" {@attach (element) => { if (focused) element.focus(); }} />
		<Button type="submit" disabled={pending}>{pending ? 'Opening…' : 'Go'}</Button>
		<IconButton label="Open preview in new window" type="button" disabled={!src} onclick={() => window.open(src, '_blank', 'noopener')}><ArrowSquareOut /></IconButton>
	</form>
	{#if error}<p class="error" role="alert">{error}</p>{/if}
	{#if src}
		{#key revision}
			<iframe bind:this={iframe} title="Host preview" {src} sandbox="allow-scripts allow-same-origin allow-forms allow-modals allow-downloads" referrerpolicy="no-referrer"></iframe>
		{/key}
	{:else}
		<div class="empty"><p>Open a development server on the Host.</p><p>localhost points to the Host here.</p></div>
	{/if}
	{#if src}
		<section class="console" class:open={consoleOpen} aria-label="Preview console">
			<div class="console-toolbar">
				<button class="console-toggle" aria-expanded={consoleOpen} onclick={toggleConsole}>Console <span>{entries.length}</span></button>
				{#if consoleOpen}<Button disabled={!entries.length} onclick={() => entries = []}>Clear console</Button>{/if}
			</div>
			{#if consoleOpen}
				<div class="entries" bind:this={output} role="log" aria-label="Console output" aria-live="off">
					{#each entries as entry (entry.id)}
						<div class="entry {entry.level}"><span class="level">{entry.level}</span><pre>{entry.text}</pre></div>
					{:else}
						<p class="console-empty">{consoleReady ? 'No console output.' : 'Waiting for console capture. The page’s script policy may block the bridge.'}</p>
					{/each}
				</div>
			{/if}
		</section>
	{/if}
</div>

<style>
	.browser {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-width: 0;
		min-height: 0;
	}

	form {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-1);
		padding: var(--space-1);
		border-bottom: var(--border-width) solid var(--border);
	}

	input {
		flex: 1;
		min-width: 0;
		min-height: var(--target-min);
		box-sizing: border-box;
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--color-bg);
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	input:focus-visible {
		outline: var(--border-width) solid var(--status-working);
	}

	iframe {
		flex: 1;
		width: 100%;
		min-height: 0;
		border: none;
		background: white;
	}

	.console {
		flex: none;
		min-height: 0;
		border-top: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	.console.open {
		display: flex;
		flex-direction: column;
		flex: 0 1 40%;
		min-height: min(140px, 40%);
	}

	.console-toolbar {
		display: flex;
		flex: none;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-1);
	}

	.console-toggle {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		cursor: pointer;
	}

	.console-toggle span, .level, .console-empty { color: var(--muted-fg); }
	.console-toggle:hover { background: var(--hover); }
	.console-toggle:focus-visible, .entries:focus-visible { outline: var(--border-width) solid var(--status-working); outline-offset: -1px; }

	.entries {
		flex: 1;
		min-height: 0;
		overflow: auto;
		overscroll-behavior: contain;
		font: var(--font-size-sm) var(--font-mono);
	}

	.entry {
		display: flex;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3);
		border-top: var(--border-width) solid var(--border);
	}

	.level { flex: 0 0 5ch; }
	pre { margin: 0; min-width: 0; font: inherit; white-space: pre-wrap; overflow-wrap: anywhere; }
	.warn, .warn .level { color: var(--ansi-3); }
	.error, .error .level { color: var(--ansi-1); }
	.console-empty { padding: var(--space-3); }

	.empty {
		display: grid;
		flex: 1;
		align-content: center;
		gap: var(--space-2);
		padding: var(--space-3);
		color: var(--muted-fg);
		text-align: center;
	}

	p { margin: 0; }

	.error {
		padding: var(--space-2) var(--space-3);
		color: var(--ansi-1);
		overflow-wrap: anywhere;
	}
</style>
