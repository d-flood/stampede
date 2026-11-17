<script lang="ts">
	import type { HTMLSelectAttributes } from 'svelte/elements';
	import CaretDown from '~icons/ph/caret-down-bold';

	let { value = $bindable(), children, ...rest }: HTMLSelectAttributes = $props();
</script>

<span class="select">
	<select bind:value {...rest}>{@render children?.()}</select>
	<CaretDown aria-hidden="true" />
</span>

<style>
	.select {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		align-items: center;
		width: 100%;
		min-width: 0;
	}

	.select > :global(*) {
		grid-area: 1 / 1;
	}

	select {
		box-sizing: border-box;
		width: 100%;
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-1) calc(2 * var(--space-2) + 1.2em) var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-md);
		appearance: none;
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		text-overflow: ellipsis;
		cursor: pointer;
		transition: border-color var(--dur-fast);
	}

	select:hover:not(:disabled) {
		border-color: color-mix(in oklab, var(--accent) 60%, var(--border-strong));
	}

	select:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	select:disabled {
		color: var(--muted-fg);
		cursor: default;
	}

	.select > :global(svg) {
		justify-self: end;
		margin-inline-end: calc(var(--space-2) + var(--border-width));
		color: var(--muted-fg);
		pointer-events: none;
	}
</style>
