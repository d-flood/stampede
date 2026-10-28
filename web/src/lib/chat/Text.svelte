<script lang="ts">
	import { renderCopyable } from '#lib/chat/markdown.ts';
	import type { Item } from '#lib/device.svelte.ts';
	import { untrack } from 'svelte';
	import { on } from 'svelte/events';

	let { item, active = true }: { item: Extract<Item, { kind: 'text' }>; active?: boolean } = $props();

	let framed = $state.raw(untrack(() => ({ text: item.text, streaming: item.streaming })));
	$effect(() => {
		if (!active) return;
		if (!item.streaming) { framed = { text: item.text, streaming: false }; return; }
		if (item.text !== framed.text) {
			const frame = requestAnimationFrame(() => {
				framed = { text: item.text, streaming: item.streaming };
			});
			return () => cancelAnimationFrame(frame);
		}
	});

	const html = $derived(renderCopyable(framed.text, !framed.streaming));

	function copy(event: MouseEvent) {
		const button = (event.target as Element).closest<HTMLElement>('.copy');
		const code = button?.parentElement!.querySelector('code');
		if (!button || !code) return;
		navigator.clipboard.writeText(code.textContent.trimEnd()).then(() => {
			button.dataset.copied = '';
			setTimeout(() => delete button.dataset.copied, 1500);
		});
	}
</script>

<div class="text" class:streaming={item.streaming} class:commentary={item.phase === 'commentary'} {@attach (text) => on(text, 'click', copy)}>{@html html}</div>

<style>
	.text {
		font-family: var(--font-chat);
		font-size: var(--font-size-chat);
		overflow-wrap: anywhere;
	}

	.commentary {
		color: color-mix(in oklab, var(--color-fg) 76%, var(--color-bg));
		font-size: calc(0.95 * var(--font-size-chat));
	}

	.text > :global(:first-child) {
		margin-top: 0;
	}

	.text > :global(:last-child) {
		margin-bottom: 0;
	}

	.text :global(:is(p, ul, ol, pre, blockquote, table, .code-block)) {
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
		font-size: var(--font-size-code);
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

	.text :global(.code-block) {
		display: flex;
		align-items: flex-start;
		border-radius: var(--radius-lg);
		background: var(--surface-1);
	}

	.text :global(.code-block pre) {
		flex: 1;
		min-width: 0;
		margin: 0;
	}

	.text :global(.copy) {
		display: inline-grid;
		place-items: center;
		padding: 1px;
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--muted-fg);
		font-size: 0.92em;
		vertical-align: text-bottom;
		cursor: pointer;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	.text :global(.code-block .copy) {
		flex: none;
		min-width: var(--target-min);
		min-height: var(--target-min);
		margin: var(--space-1);
		font-size: 1em;
	}

	.text :global(.copy:hover) {
		background: var(--hover);
		color: var(--color-fg);
	}

	.text :global(.copy:focus-visible) {
		outline: var(--border-width) solid var(--status-working);
	}

	.text :global(.copy[data-copied]) {
		color: var(--status-done);
	}

	.text :global(:is(.copy > :last-child, .copy[data-copied] > :first-child)) {
		display: none;
	}

	.text :global(.copy[data-copied] > :last-child) {
		display: block;
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
