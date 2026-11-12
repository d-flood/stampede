<script lang="ts">
	import type { OpenCanvas } from '#lib/canvas.svelte.ts';
	import { sandboxed } from '#lib/chat/html.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { onMount } from 'svelte';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';
	import Desktop from '~icons/ph/desktop';
	import ArrowsIn from '~icons/ph/arrows-in';
	import ArrowsOut from '~icons/ph/arrows-out';

	const desktopWidth = 1280;

	let { canvas }: { canvas: OpenCanvas } = $props();
	let frame = $state<HTMLIFrameElement>();
	const scroll = { x: 0, y: 0 };
	const shown = $derived(canvas.version ?? canvas.versions);
	const srcdoc = $derived(canvas.html === undefined ? undefined : sandboxed(canvas.html, `addEventListener('load', () => scrollTo(${scroll.x}, ${scroll.y}));
addEventListener('scroll', () => parent.postMessage({ type: 'stampede-canvas-scroll', x: scrollX, y: scrollY }, '*'), { passive: true });`));
	let width = $state(0);
	let stageWidth = $state(0);
	let stageHeight = $state(0);
	let section = $state<HTMLElement>();
	let desktop = $state(false);
	let fullscreen = $state(false);
	const scale = $derived(desktop ? Math.min(1, stageWidth / desktopWidth) : 1);
	let applied: string | undefined;
	// Hold new HTML while hidden: a frame loaded without layout measures everything as zero.
	const loaded = $derived.by(() => (width ? (applied = srcdoc) : applied));

	onMount(() => {
		canvas.load(canvas.version);
	});

	function scrolled(event: MessageEvent) {
		if (event.source !== frame?.contentWindow || event.data?.type !== 'stampede-canvas-scroll') return;
		const { x, y } = event.data;
		if (Number.isFinite(x) && Number.isFinite(y)) Object.assign(scroll, { x, y });
	}

	async function toggleFullscreen() {
		if (document.fullscreenElement) return document.exitFullscreen();
		try {
			await section?.requestFullscreen();
			await screen.orientation.lock('landscape');
		} catch {
			// Fullscreen can be refused and orientation lock is unsupported on desktop.
		}
	}

	function step(by: number) {
		const version = shown + by;
		canvas.load(version < canvas.versions ? version : undefined);
	}
</script>

<svelte:window onmessage={scrolled} />
<svelte:document onfullscreenchange={() => (fullscreen = document.fullscreenElement === section)} />

<section class="canvas" aria-label={canvas.name} bind:this={section} bind:clientWidth={width}>
	<div class="heading">
		<div class="title">{canvas.name}</div>
		{#if canvas.versions > 1}
			<IconButton label="Previous version" disabled={shown <= 1} onclick={() => step(-1)}><CaretLeft /></IconButton>
			<span class="version" class:past={canvas.version !== undefined}>v{shown} of {canvas.versions}</span>
			<IconButton label="Next version" disabled={canvas.version === undefined} onclick={() => step(1)}><CaretRight /></IconButton>
		{/if}
		<IconButton label="Desktop view" title="Desktop view" pressed={desktop} onclick={() => (desktop = !desktop)}><Desktop /></IconButton>
		{#if document.fullscreenEnabled}
			<IconButton label={fullscreen ? 'Exit full screen' : 'Full screen'} title={fullscreen ? 'Exit full screen' : 'Full screen'} onclick={toggleFullscreen}>
				{#if fullscreen}<ArrowsIn />{:else}<ArrowsOut />{/if}
			</IconButton>
		{/if}
	</div>
	{#if canvas.missing}
		<p class="missing">This canvas is no longer on disk.</p>
	{:else if srcdoc}
		<div class="stage" bind:clientWidth={stageWidth} bind:clientHeight={stageHeight}>
			<iframe
				bind:this={frame}
				title={canvas.name}
				srcdoc={loaded}
				sandbox="allow-scripts"
				referrerpolicy="no-referrer"
				style:width={scale < 1 ? `${desktopWidth}px` : undefined}
				style:height={scale < 1 ? `${stageHeight / scale}px` : undefined}
				style:transform={scale < 1 ? `scale(${scale})` : undefined}
			></iframe>
		</div>
	{/if}
</section>

<style>
	.canvas {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-width: 0;
	}

	.canvas:fullscreen {
		background: var(--surface-1);
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

	.stage {
		flex: 1;
		min-height: 0;
		overflow: hidden;
	}

	iframe {
		display: block;
		width: 100%;
		height: 100%;
		border: 0;
		transform-origin: 0 0;
		background: var(--surface-1);
	}

	.missing {
		margin: var(--space-6) auto;
		color: var(--muted-fg);
	}
</style>
