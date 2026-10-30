<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';

	let { label, pressed, children, ...rest }: HTMLButtonAttributes & { label: string; pressed: boolean } = $props();
</script>

<button type="button" title={label} aria-label={label} aria-pressed={pressed} {...rest}>{@render children?.()}</button>

<style>
	button {
		flex: none;
		min-width: var(--target-min);
		min-height: var(--target-min);
		border: var(--border-width) solid transparent;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		cursor: pointer;
		transition:
			background var(--dur-fast),
			border-color var(--dur-fast),
			color var(--dur-fast);
	}

	button:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	button:focus-visible {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: 1px;
	}

	[aria-pressed='true'] {
		border-color: var(--status-working);
		background: var(--surface-3);
		color: var(--color-fg);
	}
</style>
