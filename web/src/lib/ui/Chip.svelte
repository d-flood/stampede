<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';

	type Props = HTMLButtonAttributes & { element?: 'button' | 'span'; variant?: 'default' | 'ghost'; pending?: boolean; pressed?: boolean };

	let { element = 'button', variant = 'default', pending = false, pressed, children, ...rest }: Props = $props();
</script>

<svelte:element this={element} type={element === 'button' ? 'button' : undefined} class={['chip', variant, { pending }]} aria-pressed={pressed} {...rest}>
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
		border: var(--border-width) solid transparent;
		border-radius: var(--radius-pill);
		background: var(--surface-2);
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
	}

	:where(.chip) :global(svg:not([color])) {
		color: var(--muted-fg);
	}

	button {
		cursor: pointer;
	}

	/* Keeps the touch target at full size while the pill itself stays slim. */
	@media (pointer: coarse) {
		button.chip {
			position: relative;
			min-height: var(--control-min);
		}

		button.chip::after {
			content: '';
			position: absolute;
			inset: calc((var(--control-min) - var(--target-min)) / 2) 0;
		}
	}

	button:hover:not(:disabled),
	button[data-state='open'] {
		background: var(--surface-3);
		color: var(--color-fg);
	}

	button.ghost:hover:not(:disabled),
	button.ghost[data-state='open'] {
		background: var(--hover);
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

	.pending {
		border-style: dashed;
		border-color: var(--accent);
	}

	/* A toggle reads as off by its outline alone, so only the chosen ones carry a fill. */
	[aria-pressed='false'] {
		border-color: var(--border-strong);
		background: none;
		color: var(--muted-fg);
	}

	[aria-pressed='true'],
	button[aria-pressed='true']:hover:not(:disabled) {
		background: var(--selected-bg);
		color: var(--color-fg);
	}
</style>
