<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = Omit<HTMLAttributes<HTMLDivElement>, 'children'> & { segments: Snippet[] };

	let { segments, ...rest }: Props = $props();
</script>

<div class="group" role="group" {...rest}>
	{#each segments as segment, index (index)}
		{#if index}<span class="divider" aria-hidden="true"></span>{/if}
		{@render segment()}
	{/each}
</div>

<style>
	.group {
		display: inline-flex;
		flex: none;
		align-items: center;
		min-width: 0;
		max-width: 100%;
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-md);
	}

	.divider {
		flex: none;
		align-self: stretch;
		width: var(--border-width);
		background: var(--border);
	}
</style>
