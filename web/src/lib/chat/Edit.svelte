<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import type { Item } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import CaretRight from '~icons/ph/caret-right';

	let { item, ondiff }: { item: Extract<Item, { kind: 'edit' }>; ondiff(path: string): void } = $props();
</script>

{#snippet counts()}
	<span class="counts"><span class="added">+{item.added}</span> <span class="removed">−{item.removed}</span></span>
{/snippet}

{#snippet diff()}
	{#if item.diff}
		<div class="diff">
			<Code code={item.diff} path={item.path} diff />
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
			<span class="caret"><CaretRight /></span>
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

	.caret {
		display: inline-flex;
		flex: none;
		width: 1em;
		color: var(--muted-fg);
		transition: transform var(--dur-fast);
	}

	details[open] > summary > .caret {
		transform: rotate(90deg);
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

	.diff :global(.code-line) {
		padding: 0 var(--space-4);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.diff :global(.added) {
		background: color-mix(in oklab, var(--git-added) 14%, transparent);
	}

	.diff :global(.removed) {
		background: color-mix(in oklab, var(--git-deleted) 14%, transparent);
	}

	.diff :global(.hunk) {
		color: var(--muted-fg);
	}
</style>
