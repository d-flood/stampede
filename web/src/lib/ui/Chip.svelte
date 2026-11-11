<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';

	type Props = HTMLButtonAttributes & { element?: 'button' | 'span'; variant?: 'default' | 'ghost'; mono?: boolean; pending?: boolean; pressed?: boolean };

	let { element = 'button', variant = 'default', mono = false, pending = false, pressed, children, ...rest }: Props = $props();
</script>

<svelte:element this={element} type={element === 'button' ? 'button' : undefined} class={['chip', variant, { mono, pending }]} aria-pressed={pressed} {...rest}>
	{@render children?.()}
</svelte:element>

<style>
	.chip {
		display: inline-flex;
		box-sizing: border-box;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		max-width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-pill);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		white-space: nowrap;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	/* Zero specificity lets callers tint an icon, such as a running Shell. */
	:where(.chip) :global(svg) {
		flex: none;
		color: var(--muted-fg);
	}

	button {
		cursor: pointer;
	}

	button:hover:not(:disabled),
	button[data-state='open'] {
		background: var(--hover);
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

	.ghost {
		border-color: transparent;
		background: none;
		color: var(--muted-fg);
	}

	.mono {
		font-family: var(--font-mono);
	}

	.pending {
		border-style: dashed;
		border-color: var(--accent);
	}

	[aria-pressed='true'] {
		border-color: var(--selected-border);
		background: var(--selected-bg);
		color: var(--color-fg);
	}
</style>
