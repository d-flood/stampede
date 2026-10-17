<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';

	let { item }: { item: Extract<Item, { kind: 'html' }> } = $props();
	let frame: HTMLIFrameElement;
	let height = $state(240);

	const srcdoc = $derived(`<!doctype html>
<html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>:root{color-scheme:light dark;font:14px/1.5 system-ui}*{box-sizing:border-box}body{margin:0;padding:16px;overflow-wrap:anywhere;display:flow-root}img,svg{max-width:100%}</style>
<script>
addEventListener('DOMContentLoaded', () => {
	const resize = () => parent.postMessage({type:'oikonomos-html-size',height:Math.ceil(document.body.getBoundingClientRect().height)}, '*');
	new ResizeObserver(resize).observe(document.body);
	resize();
});
<\/script></head><body>${item.html}</body></html>`);

	function resize(event: MessageEvent) {
		if (event.source !== frame?.contentWindow || event.data?.type !== 'oikonomos-html-size') return;
		const value = event.data.height;
		if (typeof value === 'number' && Number.isFinite(value)) height = Math.max(80, Math.min(1200, value));
	}
</script>

<svelte:window onmessage={resize} />

<section class="html" aria-label={item.title}>
	<div class="title">{item.title}</div>
	<iframe bind:this={frame} title={item.title} {srcdoc} sandbox="allow-scripts" referrerpolicy="no-referrer" style:height="{height}px"></iframe>
</section>

<style>
	.html {
		min-width: 0;
		overflow: hidden;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
	}

	.title {
		padding: var(--space-2) var(--space-4);
		background: var(--surface-2);
		font-size: 0.92em;
		overflow-wrap: anywhere;
	}

	iframe {
		display: block;
		width: 100%;
		border: 0;
		background: var(--surface-1);
	}
</style>
