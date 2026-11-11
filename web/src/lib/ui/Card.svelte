<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';

	let { primary = false, children, ...rest }: HTMLButtonAttributes & { primary?: boolean } = $props();
</script>

<button type="button" class="card" class:primary {...rest}>{@render children?.()}</button>

<style>
	.card {
		display: block;
		box-sizing: border-box;
		width: 100%;
		min-width: 0;
		padding: var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
		text-align: start;
		cursor: pointer;
		transition:
			background var(--dur-fast),
			border-color var(--dur-fast);
	}

	.card:hover:not(:disabled) {
		border-color: var(--border-strong);
		background: var(--hover);
	}

	.card:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	.card:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.primary,
	.primary:hover:not(:disabled) {
		border-color: var(--accent);
		box-shadow: inset 0 0 0 var(--border-width) var(--accent);
	}
</style>
