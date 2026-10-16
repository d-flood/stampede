<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';

	let { item, first }: { item: Extract<Item, { kind: 'user' }>; first: boolean } = $props();

	const lines = $derived(item.text.split('\n'));
</script>

{#if lines.length > 12}
	<details class="user">
		<summary>
			<span class="first">
				{#if first}<span class="label">First prompt</span>{' · '}{/if}{lines[0]}
			</span>
			<span class="label">{lines.length} lines</span>
		</summary>
		<div class="text">{item.text}</div>
	</details>
{:else}
	<div class="user text">{item.text}</div>
{/if}

<style>
	.user {
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
	}

	.text {
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	details.user {
		padding-block: 0;
	}

	summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		min-height: var(--target-min);
		list-style: none;
		cursor: pointer;
		font-size: 0.92em;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	summary::after {
		content: '▸';
		color: var(--muted-fg);
	}

	details[open] > summary::after {
		content: '▾';
	}

	.first {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.label {
		color: var(--muted-fg);
	}

	details .text {
		padding-bottom: var(--space-3);
	}
</style>
