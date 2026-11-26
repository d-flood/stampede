<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import CaretDown from '~icons/ph/caret-down-bold';
	import CaretRight from '~icons/ph/caret-right-bold';

	type Props = HTMLButtonAttributes & { value: string; leading?: Snippet; caret?: 'down' | 'right'; bare?: boolean };

	let { value, leading, caret = 'down', bare = false, ...rest }: Props = $props();
</script>

<button type="button" class:bare {...rest}>
	{@render leading?.()}
	<span class="value">{value}</span>
	{#if caret === 'right'}<CaretRight />{:else}<CaretDown />{/if}
</button>

<style>
	button {
		display: flex;
		box-sizing: border-box;
		align-items: center;
		gap: var(--space-2);
		width: 100%;
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		text-align: start;
		cursor: pointer;
		transition: border-color var(--dur-fast);
	}

	button:hover:not(:disabled) {
		border-color: color-mix(in oklab, var(--accent) 60%, var(--border-strong));
	}

	.bare {
		border-color: transparent;
		background: none;
	}

	.bare:hover:not(:disabled) {
		border-color: transparent;
		background: var(--hover);
	}

	button:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	button:disabled {
		color: var(--muted-fg);
		cursor: default;
	}

	button > :global(svg:last-child) {
		flex: none;
		color: var(--muted-fg);
	}

	.value {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
