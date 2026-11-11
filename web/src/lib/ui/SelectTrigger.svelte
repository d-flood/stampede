<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import CaretDown from '~icons/ph/caret-down';
	import CaretRight from '~icons/ph/caret-right';

	type Props = HTMLButtonAttributes & { value: string; leading?: Snippet; caret?: 'down' | 'right' };

	let { value, leading, caret = 'down', ...rest }: Props = $props();
</script>

<button type="button" {...rest}>
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
