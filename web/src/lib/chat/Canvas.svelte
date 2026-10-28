<script lang="ts">
	import { canvasName } from '#lib/canvas.svelte.ts';
	import type { Item } from '#lib/device.svelte.ts';
	import Browsers from '~icons/ph/browsers';

	let { item, oncanvas }: { item: Extract<Item, { kind: 'canvas' }>; oncanvas(canvas: string): void } = $props();
</script>

<div class="canvas" class:write={item.write}>
	{#if item.write}<span class="icon"><Browsers /></span>{/if}
	<span class="title">{item.write ? 'Wrote' : 'Edited'} canvas <span class="name">{canvasName(item.canvas)}</span></span>
	{#if !item.write}
		<span class="counts"><span class="added">+{item.added}</span> <span class="removed">−{item.removed}</span></span>
	{/if}
	<button class="open" onclick={() => oncanvas(item.canvas)}>Open</button>
</div>

<style>
	.canvas {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		min-height: var(--target-min);
		padding: 0 var(--space-4);
		overflow: hidden;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		font-size: 0.92em;
	}

	.icon {
		display: inline-flex;
		flex: none;
		color: var(--muted-fg);
	}

	.title {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.name,
	.counts {
		font-family: var(--font-mono);
	}

	.counts {
		flex-shrink: 0;
	}

	.counts .added {
		color: var(--git-added);
	}

	.counts .removed {
		color: var(--git-deleted);
	}

	.open {
		flex-shrink: 0;
		min-height: var(--target-min);
		padding: 0;
		border: 0;
		background: none;
		color: var(--status-working);
		font: inherit;
		cursor: pointer;
	}

	.open:hover {
		text-decoration: underline;
	}
</style>
