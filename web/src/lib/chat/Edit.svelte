<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import type { Item } from '#lib/device.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import { form } from '#lib/form.ts';

	let { item, ondiff }: { item: Extract<Item, { kind: 'edit' }>; ondiff(path: string): void } = $props();
</script>

<details class="edit">
	<summary>
		<span class="title">Edited <span class="path" title={item.path}>{form() === 'phone' ? item.path.split('/').at(-1) : item.path}</span></span>
		<span class="counts"><span class="added">+{item.added}</span> <span class="removed">−{item.removed}</span></span>
	</summary>
	{#if item.diff}
		<div class="diff">
			<Code code={item.diff} path={item.path} diff />
		</div>
	{/if}
	{#if !item.path.startsWith('/')}
		<Button variant="link" onclick={() => ondiff(item.path)}>Open diff</Button>
	{/if}
</details>

<style>
	summary {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		list-style: none;
		cursor: pointer;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	.title {
		min-width: 0;
		overflow: hidden;
		color: var(--color-fg);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.path,
	.counts,
	.diff {
		font-family: var(--font-mono);
		tab-size: 4;
	}

	.path {
		color: var(--muted-fg);
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

	.edit > :global(button) {
		display: flex;
		margin-left: auto;
	}

	.diff {
		max-height: 24em;
		margin-top: var(--space-1);
		overflow: auto;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
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
