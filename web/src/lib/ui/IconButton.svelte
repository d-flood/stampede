<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';

	let { label, pressed, children, ...rest }: HTMLButtonAttributes & { label: string; pressed?: boolean } = $props();
</script>

<button aria-label={label} aria-pressed={pressed} {...rest}>{@render children?.()}</button>

<style>
	button {
		display: inline-grid;
		flex: none;
		place-items: center;
		min-width: var(--target-min);
		min-height: var(--target-min);
		padding: 0 2px;
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		white-space: nowrap;
		cursor: pointer;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	button :global(svg) {
		font-size: var(--font-size-lg);
	}

	button:hover:not(:disabled),
	button[data-state='open'] {
		background: var(--hover);
		color: var(--color-fg);
	}

	button[aria-pressed='true'] {
		background: var(--selected-bg);
		box-shadow: inset 0 0 0 var(--border-width) var(--selected-border);
		color: var(--color-fg);
	}

	button:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
</style>
