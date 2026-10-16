<script lang="ts">
	import Transcript from '#lib/chat/Transcript.svelte';
	import type { Item } from '#lib/device.svelte.ts';

	let { item, ondiff }: { item: Extract<Item, { kind: 'subagent' }>; ondiff(path: string): void } = $props();
</script>

<details>
	<summary>
		<span class="task">↳ {item.name ? `${item.name} · ` : ''}{item.task}</span>
		<span class="status" class:working={item.status === 'working'} class:waiting={item.status === 'waiting'}>{item.status}</span>
	</summary>
	<div class="activity"><Transcript items={item.items} {ondiff} /></div>
</details>

<style>
	summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		min-height: var(--target-min);
		list-style: none;
		cursor: pointer;
	}

	summary::-webkit-details-marker {
		display: none;
	}
	.task {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	details[open] .task {
		white-space: normal;
		overflow-wrap: anywhere;
	}

	.status {
		color: var(--status-done);
		flex-shrink: 0;
		font-size: var(--font-size-sm);
	}
	.working {
		color: var(--status-working);
	}
	.waiting {
		color: var(--status-waiting);
	}
	.activity {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		margin-left: var(--space-3);
		padding-left: var(--space-3);
		border-left: var(--border-width) solid var(--border);
	}
</style>
