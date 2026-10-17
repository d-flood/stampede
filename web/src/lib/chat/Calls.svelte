<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';
	import type { Snippet } from 'svelte';

	let { items, activity, children }: { items: Item[]; activity?: string | null; children: Snippet } = $props();

	const count = $derived(items.reduce((count, item) => count + (item.kind === 'tools' ? item.calls.length : ['command', 'edit'].includes(item.kind) ? 1 : 0), 0));
	const latest = $derived.by(() => {
		if (activity) return activity;
		for (const item of items.toReversed()) {
			if (item.kind === 'command') return `$ ${item.command}`;
			if (item.kind === 'edit') return `Edited ${item.path}`;
			if (item.kind === 'tools') {
				const call = item.calls.at(-1);
				if (call) return `${call.name}${call.kind === 'other' ? '' : ` ${call.detail}`}`;
			}
		}
		return '';
	});
</script>

{#if count}
	<details class="calls">
		<summary><span class="count">{count} tool {count === 1 ? 'call' : 'calls'}</span><span class="latest">{latest}</span></summary>
		<div class="content">{@render children()}</div>
	</details>
{:else}
	{@render children()}
{/if}

<style>
	summary { display: flex; align-items: center; gap: var(--space-2); min-height: var(--target-min); color: var(--muted-fg); font-size: var(--font-size-sm); list-style: none; cursor: pointer; }
	summary::-webkit-details-marker { display: none; }
	summary::before { content: '▸'; flex: none; }
	details[open] > summary::before { content: '▾'; }
	summary:hover { color: var(--color-fg); }
	.count { flex: none; }
	.latest { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--font-mono); }
	.latest::before { content: '· '; }
	.content { padding-top: var(--space-2); }
</style>
