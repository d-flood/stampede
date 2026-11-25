<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = { path?: string; title?: string; deleted?: boolean; wrap?: boolean; children?: Snippet };

	let { path, title, deleted = false, wrap = false, children }: Props = $props();
</script>

<div class="toolbar" class:wrap>
	{#if path !== undefined}
		{@const cut = path.lastIndexOf('/') + 1}
		<span class="path" title={title ?? path}>{path.slice(0, cut)}<span class="name">{path.slice(cut)}</span></span>
	{/if}
	{#if deleted}<span class="deleted">deleted on disk</span>{/if}
	{@render children?.()}
</div>

<style>
	.toolbar {
		display: flex;
		flex: none;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-1) 0 var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
	}

	.wrap {
		flex-wrap: wrap;
	}

	.path {
		flex: 1;
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.name {
		color: var(--color-fg);
	}

	.wrap .path {
		flex-basis: 8rem;
	}

	.deleted {
		flex: none;
		color: var(--status-waiting);
		font-size: var(--font-size-sm);
	}
</style>
