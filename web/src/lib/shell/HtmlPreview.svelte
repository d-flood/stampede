<script lang="ts">
	import { sandboxed } from '#lib/chat/html.ts';

	let { text, title }: { text: string; title: string } = $props();
	let frame = $state<HTMLIFrameElement>();
	const scroll = { x: 0, y: 0 };
	const srcdoc = $derived(sandboxed(text, `addEventListener('load', () => scrollTo(${scroll.x}, ${scroll.y}));
addEventListener('scroll', () => parent.postMessage({ type: 'stampede-preview-scroll', x: scrollX, y: scrollY }, '*'), { passive: true });`));
	let width = $state(0);
	let applied: string | undefined;
	// Hold new HTML while hidden: a frame loaded without layout measures everything as zero.
	const loaded = $derived.by(() => (width ? (applied = srcdoc) : applied));

	function scrolled(event: MessageEvent) {
		if (event.source !== frame?.contentWindow || event.data?.type !== 'stampede-preview-scroll') return;
		const { x, y } = event.data;
		if (Number.isFinite(x) && Number.isFinite(y)) Object.assign(scroll, { x, y });
	}
</script>

<svelte:window onmessage={scrolled} />

<div class="preview" role="region" aria-label="HTML preview" bind:clientWidth={width}>
	<iframe bind:this={frame} {title} srcdoc={loaded} sandbox="allow-scripts" referrerpolicy="no-referrer"></iframe>
</div>

<style>
	.preview {
		height: 100%;
	}

	iframe {
		display: block;
		width: 100%;
		height: 100%;
		border: 0;
		background: var(--surface-1);
	}
</style>
