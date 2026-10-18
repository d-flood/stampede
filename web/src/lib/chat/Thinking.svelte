<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';
	import CaretRight from '~icons/ph/caret-right';

	let { item }: { item: Extract<Item, { kind: 'thinking' }> } = $props();

	const label = $derived(item.seconds ? `Thought for ${item.seconds}s` : 'Thought briefly');
</script>

{#if item.text}
	<details class="thinking">
		<summary><span class="caret"><CaretRight /></span>{label}</summary>
		<div class="text">{item.text}</div>
	</details>
{:else}
	<div class="thinking">{label}</div>
{/if}

<style>
	.thinking {
		color: var(--muted-fg);
		font-size: 0.92em;
		font-style: italic;
	}

	div.thinking,
	summary {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
	}

	summary {
		list-style: none;
		cursor: pointer;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	.caret {
		display: inline-flex;
		flex: none;
		width: 1em;
		transition: transform var(--dur-fast);
	}

	details[open] > summary > .caret {
		transform: rotate(90deg);
	}

	summary:hover {
		color: var(--color-fg);
	}

	.text {
		padding: 0 0 0 var(--space-4);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
