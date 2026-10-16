<script lang="ts">
	import type { Item } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';

	let { item, ondiff }: { item: Extract<Item, { kind: 'edit' }>; ondiff(path: string): void } = $props();

	const signs: Record<string, string> = { '+': 'added', '-': 'removed', '@': 'hunk' };

	const lines = $derived(item.diff.split('\n').filter((line) => line));
</script>

{#snippet counts()}
	<span class="counts"><span class="added">+{item.added}</span> <span class="removed">−{item.removed}</span></span>
{/snippet}

{#snippet diff()}
	{#if lines.length}
		<div class="diff">
			{#each lines as line, index (index)}
				<div class={signs[line[0]]}>{line}</div>
			{/each}
		</div>
	{/if}
{/snippet}

{#snippet open()}
	{#if !item.path.startsWith('/')}
		<button class="open" onclick={() => ondiff(item.path)}>Open diff</button>
	{/if}
{/snippet}

{#if form() === 'phone'}
	<details class="edit">
		<summary>
			<span class="title">Edited <span class="path">{item.path.split('/').at(-1)}</span></span>
			{@render counts()}
		</summary>
		{@render diff()}
		{@render open()}
	</details>
{:else}
	<div class="edit">
		<div class="head">
			<span class="title">Edited <span class="path">{item.path}</span></span>
			{@render counts()}
			{@render open()}
		</div>
		{@render diff()}
	</div>
{/if}

<style>
	.edit {
		overflow: hidden;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
	}

	.head,
	summary {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		min-height: var(--target-min);
		padding: 0 var(--space-4);
		font-size: 0.92em;
	}

	.head {
		background: var(--surface-2);
	}

	summary {
		list-style: none;
		cursor: pointer;
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

	.title {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.path,
	.counts,
	.diff {
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

	details .open {
		display: block;
		margin-left: auto;
		padding: 0 var(--space-4);
		font-size: 0.92em;
	}

	.diff {
		max-height: 24em;
		overflow: auto;
		border-top: var(--border-width) solid var(--border);
		font-size: 0.92em;
		line-height: 1.6;
	}

	.diff div {
		padding: 0 var(--space-4);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.diff .added {
		background: color-mix(in oklab, var(--git-added) 14%, transparent);
	}

	.diff .removed {
		background: color-mix(in oklab, var(--git-deleted) 14%, transparent);
	}

	.diff .hunk {
		color: var(--muted-fg);
	}
</style>
