<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';

	let { item }: { item: Extract<Item, { kind: 'notice' }> } = $props();
</script>

<div class="notice {item.notice}">{item.text}</div>

<style>
	.notice {
		font-size: 0.92em;
		overflow-wrap: anywhere;
	}

	.compacted {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		color: var(--muted-fg);
	}

	.compacted::before,
	.compacted::after {
		content: '';
		flex: 1;
		border-top: var(--border-width) solid var(--border);
	}

	.error,
	.limit {
		padding: var(--space-2) var(--space-3);
		border-left: 2px solid;
		border-radius: var(--radius-md);
		background: var(--surface-1);
	}

	.error {
		border-left-color: var(--ansi-1);
	}

	.limit {
		border-left-color: var(--status-waiting);
	}
</style>
