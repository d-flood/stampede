<script lang="ts">
	import type { OpenCanvas } from '#lib/canvas.svelte.ts';
	import { sandboxed } from '#lib/chat/html.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { onMount } from 'svelte';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';

	let { canvas }: { canvas: OpenCanvas } = $props();
	let frame = $state<HTMLIFrameElement>();
	const scroll = { x: 0, y: 0 };
	const shown = $derived(canvas.version ?? canvas.versions);
	const srcdoc = $derived(canvas.html === undefined ? undefined : sandboxed(canvas.html, `addEventListener('load', () => scrollTo(${scroll.x}, ${scroll.y}));
addEventListener('scroll', () => parent.postMessage({ type: 'stampede-canvas-scroll', x: scrollX, y: scrollY }, '*'), { passive: true });`));

	onMount(() => {
		canvas.load(canvas.version);
	});

	function scrolled(event: MessageEvent) {
		if (event.source !== frame?.contentWindow || event.data?.type !== 'stampede-canvas-scroll') return;
		const { x, y } = event.data;
		if (Number.isFinite(x) && Number.isFinite(y)) Object.assign(scroll, { x, y });
	}

	function step(by: number) {
		const version = shown + by;
		canvas.load(version < canvas.versions ? version : undefined);
	}
</script>

<svelte:window onmessage={scrolled} />

<section class="canvas" aria-label={canvas.name}>
	<div class="heading">
		<div class="title">{canvas.name}</div>
		{#if canvas.versions > 1}
			<IconButton label="Previous version" disabled={shown <= 1} onclick={() => step(-1)}><CaretLeft /></IconButton>
			<span class="version" class:past={canvas.version !== undefined}>v{shown} of {canvas.versions}</span>
			<IconButton label="Next version" disabled={canvas.version === undefined} onclick={() => step(1)}><CaretRight /></IconButton>
		{/if}
	</div>
	{#if canvas.missing}
		<p class="missing">This canvas is no longer on disk.</p>
	{:else if srcdoc}
		<iframe bind:this={frame} title={canvas.name} {srcdoc} sandbox="allow-scripts" referrerpolicy="no-referrer"></iframe>
	{/if}
</section>

<style>
	.canvas {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-width: 0;
	}

	.heading {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-2) var(--space-1) var(--space-4);
		border-bottom: var(--border-width) solid var(--border);
		background: var(--surface-2);
		font-size: 0.92em;
	}

	.title {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.version {
		flex-shrink: 0;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums;
	}

	.version.past {
		color: var(--status-waiting);
	}

	iframe {
		flex: 1;
		min-height: 0;
		width: 100%;
		border: 0;
		background: var(--surface-1);
	}

	.missing {
		margin: var(--space-6) auto;
		color: var(--muted-fg);
	}
</style>
