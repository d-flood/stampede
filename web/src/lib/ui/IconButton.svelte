<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';

	let { label, children, ...rest }: HTMLButtonAttributes & { label: string } = $props();
</script>

<button aria-label={label} {...rest}>{@render children?.()}</button>

<style>
	button {
		display: inline-grid;
		place-items: center;
		min-width: var(--target-min);
		min-height: var(--target-min);
		padding: 0;
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--muted-fg);
		font-size: var(--font-size-lg);
		cursor: pointer;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	button:hover:not(:disabled),
	button[data-state='open'] {
		background: var(--hover);
		color: var(--color-fg);
	}

	button:focus-visible {
		outline: var(--border-width) solid var(--status-working);
	}

	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
