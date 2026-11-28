<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import type { Item } from '#lib/device.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import { form } from '#lib/form.ts';

	let { item, ondiff }: { item: Extract<Item, { kind: 'edit' }>; ondiff(path: string): void } = $props();
</script>

{#snippet heading()}
	<span class="title">Edited <span class="path" title={item.path}>{form() === 'phone' ? item.path.split('/').at(-1) : item.path}</span></span>
	<span class="counts">{#if item.added}<span class="added">+{item.added}</span>{/if}{#if item.added && item.removed}{" "}{/if}{#if item.removed}<span class="removed">−{item.removed}</span>{/if}</span>
{/snippet}

{#if item.elided}
	<div class="edit summary">{@render heading()}</div>
{:else}
	<details class="edit">
		<summary>{@render heading()}</summary>
		{#if item.diff}
			<div class="diff">
				<Code code={item.diff} path={item.path} diff />
			</div>
		{/if}
		{#if !item.path.startsWith('/')}
			<Button variant="link" onclick={() => ondiff(item.path)}>Open diff</Button>
		{/if}
	</details>
{/if}

<style>
	summary,
	.summary {
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
