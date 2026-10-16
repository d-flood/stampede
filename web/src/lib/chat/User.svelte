<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';

	let { item }: { item: Extract<Item, { kind: 'user' }> } = $props();
</script>

<div class="user">{item.text}</div>

<style>
	.user {
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
