<script lang="ts">
	import { getHtmlPreview, type HtmlItem } from '#lib/chat/html.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import ArrowsOut from '~icons/ph/arrows-out';
	import X from '~icons/ph/x';

	let { item, full = false, onclose }: { item: HtmlItem; full?: boolean; onclose?: () => void } = $props();
	const preview = getHtmlPreview();
	let frame: HTMLIFrameElement;
	let height = $state(240);

	const srcdoc = $derived(`<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>:root{color-scheme:light dark;font:14px/1.5 system-ui}*{box-sizing:border-box}body{margin:0;padding:16px;overflow-wrap:anywhere;display:flow-root}img,svg{max-width:100%}</style>
<script>
addEventListener('DOMContentLoaded', () => {
	const resize = () => parent.postMessage({type:'stampede-html-size',height:Math.ceil(document.body.getBoundingClientRect().height)}, '*');
	new ResizeObserver(resize).observe(document.body);
	resize();
});
<\/script></head><body>${item.html}</body></html>`);

	function resize(event: MessageEvent) {
		if (event.source !== frame?.contentWindow || event.data?.type !== 'stampede-html-size') return;
		const value = event.data.height;
		if (typeof value === 'number' && Number.isFinite(value)) height = Math.max(80, Math.min(1200, value));
	}
</script>

<svelte:window onmessage={resize} />

<section class="html" class:full aria-label={item.title}>
	<div class="heading">
		<div class="title">{item.title}</div>
		{#if full}
			<IconButton label="Close full screen" onclick={onclose}><X /></IconButton>
		{:else}
			<IconButton label="Full screen" onclick={() => preview(item)}><ArrowsOut /></IconButton>
		{/if}
	</div>
	<iframe bind:this={frame} title={item.title} {srcdoc} sandbox="allow-scripts" referrerpolicy="no-referrer" style:height={full ? undefined : `${height}px`}></iframe>
</section>

<style>
	.html {
		min-width: 0;
		overflow: hidden;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
	}

	.heading {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-2) var(--space-1) var(--space-4);
		background: var(--surface-2);
		font-size: 0.92em;
	}

	.title {
		flex: 1;
		min-width: 0;
		overflow-wrap: anywhere;
	}

	iframe {
		display: block;
		width: 100%;
		border: 0;
		background: var(--surface-1);
	}

	.full {
		display: flex;
		flex-direction: column;
		height: 100%;
		border: 0;
		border-radius: 0;
	}

	.full .heading {
		padding-top: max(var(--space-1), env(safe-area-inset-top));
		padding-right: max(var(--space-2), env(safe-area-inset-right));
		padding-left: max(var(--space-4), env(safe-area-inset-left));
	}

	.full iframe {
		flex: 1;
		min-height: 0;
	}
</style>
