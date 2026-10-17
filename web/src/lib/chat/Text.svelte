<script lang="ts">
	import { render } from '#lib/chat/markdown.ts';
	import type { Item } from '#lib/device.svelte.ts';

	let { item }: { item: Extract<Item, { kind: 'text' }> } = $props();

	const html = $derived(render(item.text));
</script>

<div class="text" class:streaming={item.streaming} class:commentary={item.phase === 'commentary'}>{@html html}</div>

<style>
	.text {
		overflow-wrap: anywhere;
	}

	.commentary {
		color: color-mix(in oklab, var(--color-fg) 76%, var(--color-bg));
		font-size: 0.95em;
	}

	.text > :global(:first-child) {
		margin-top: 0;
	}

	.text > :global(:last-child) {
		margin-bottom: 0;
	}

	.text :global(:is(p, ul, ol, pre, blockquote, table)) {
		margin: 0 0 var(--space-3);
	}

	.text :global(:is(h1, h2, h3, h4, h5, h6)) {
		margin: var(--space-4) 0 var(--space-2);
		font-size: 1em;
	}

	.text :global(:is(ul, ol)) {
		padding-left: 1.5em;
	}

	.text :global(code) {
		padding: 1px var(--space-1);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		font-family: var(--font-mono);
		font-size: 0.92em;
	}

	.text :global(pre) {
		overflow-x: auto;
		padding: var(--space-3);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
	}

	.text :global(pre code) {
		padding: 0;
		background: none;
	}

	.text :global(blockquote) {
		padding-left: var(--space-3);
		border-left: 2px solid var(--border);
		color: var(--muted-fg);
	}

	.text :global(table) {
		display: block;
		overflow-x: auto;
		border-collapse: collapse;
	}

	.text :global(:is(th, td)) {
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border);
	}

	.text :global(a) {
		color: var(--status-working);
	}

	.streaming > :global(:last-child)::after {
		content: '';
		display: inline-block;
		width: 0.5em;
		height: 1em;
		margin-left: 2px;
		background: var(--status-working);
		vertical-align: text-bottom;
	}
</style>
