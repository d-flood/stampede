<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';

	let { label, pressed, children, ...rest }: HTMLButtonAttributes & { label: string; pressed: boolean } = $props();
</script>

<button type="button" title={label} aria-label={label} aria-pressed={pressed} {...rest}><span>{@render children?.()}</span></button>

<style>
	button {
		display: inline-grid;
		flex: none;
		place-items: center;
		min-width: var(--target-min);
		min-height: var(--target-min);
		padding: 0 2px;
		border: none;
		background: none;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		cursor: pointer;
		transition: color var(--dur-fast);
	}

	span {
		display: inline-flex;
		box-sizing: border-box;
		align-items: center;
		justify-content: center;
		min-width: var(--control-min);
		height: var(--control-min);
		padding: 0 var(--space-1);
		border: var(--border-width) solid transparent;
		border-radius: var(--radius-sm);
		white-space: nowrap;
		transition:
			background var(--dur-fast),
			border-color var(--dur-fast);
	}

	button:hover {
		color: var(--color-fg);
	}

	button:hover span {
		background: var(--hover);
	}

	button:focus-visible {
		outline: none;
	}

	button:focus-visible span {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: 1px;
	}

	[aria-pressed='true'] {
		color: var(--color-fg);
	}

	[aria-pressed='true'] span {
		border-color: var(--status-working);
		background: var(--surface-3);
	}
</style>
