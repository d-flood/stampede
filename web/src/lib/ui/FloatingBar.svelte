<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = HTMLAttributes<HTMLDivElement> & { edge?: 'top' | 'bottom'; tone?: 'neutral' | 'waiting'; docked?: boolean; belowDialogs?: boolean; passive?: boolean };

	let { edge = 'bottom', tone = 'neutral', docked = false, belowDialogs = false, passive = false, children, ...rest }: Props = $props();
</script>

<div class={['bar', edge, tone, { docked, passive, below: belowDialogs }]} {...rest}>{@render children?.()}</div>

<style>
	.bar {
		position: fixed;
		left: 50%;
		z-index: 55;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: center;
		gap: var(--space-2) var(--space-3);
		box-sizing: border-box;
		max-width: calc(100vw - 2 * var(--space-4));
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-md);
		translate: -50%;
	}

	.top {
		top: var(--space-2);
	}

	.bottom {
		bottom: var(--space-6);
	}

	.below {
		z-index: 45;
	}

	.passive {
		pointer-events: none;
	}

	.waiting {
		background: color-mix(in oklab, var(--status-waiting) 16%, var(--color-bg));
	}

	.docked {
		position: static;
		flex: none;
		max-width: none;
		border-width: var(--border-width) 0 0;
		border-radius: 0;
		box-shadow: none;
		translate: none;
	}
</style>
