<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';

	let { item }: { item: Extract<Item, { kind: 'thinking' }> } = $props();
</script>

<div class="thinking" title={item.text}>
	<span class="label">{item.seconds ? `Thought for ${item.seconds}s` : 'Thought briefly'}</span>
	{#if item.text}<span class="text">{item.text}</span>{/if}
</div>

<style>
	.thinking {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		color: var(--muted-fg);
		font-style: italic;
	}

	.label {
		flex: none;
	}

	.text {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
