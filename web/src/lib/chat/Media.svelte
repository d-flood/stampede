<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';

	let { item }: { item: Extract<Item, { kind: 'media' }> } = $props();
	let failed = $state(false);
	const src = $derived(`/api/media/${encodeURIComponent(item.asset)}`);
	const video = $derived(/\.(mp4|webm)$/.test(item.asset));
</script>

<figure aria-label={item.title}>
	<figcaption>
		<span>{item.title}</span>
		<a href={src} target="_blank" rel="noreferrer" aria-label="Open {item.title}">Open</a>
	</figcaption>
	{#if video}
		<video src={src} controls playsinline muted preload="metadata" aria-label={item.title} onerror={() => failed = true}></video>
	{:else}
		<img src={src} alt={item.title} loading="lazy" onerror={() => failed = true} />
	{/if}
	{#if failed}<p>Couldn’t load this {video ? 'recording' : 'image'}. Use Open to view or download it.</p>{/if}
</figure>

<style>
	figure {
		min-width: 0;
		margin: 0;
		overflow: hidden;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
	}

	figcaption {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-4);
		background: var(--surface-2);
		font-size: 0.92em;
		overflow-wrap: anywhere;
	}

	a {
		flex-shrink: 0;
		color: var(--status-working);
	}

	img, video {
		display: block;
		width: 100%;
		max-height: 70dvh;
		object-fit: contain;
	}

	p {
		margin: 0;
		padding: var(--space-3) var(--space-4);
		color: var(--muted-fg);
	}
</style>
