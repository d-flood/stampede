<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = Omit<HTMLAttributes<HTMLElement>, 'class'> & {
		element?: 'div' | 'p' | 'h3' | 'li';
		/** Heads a group of `md` ListItems, in line with their marks. */
		size?: 'md';
		/** Labels a run of `md` ListItems within a group, a step quieter than the group's heading. */
		sub?: boolean;
	};

	let { element = 'div', size, sub = false, children, ...rest }: Props = $props();
</script>

<svelte:element this={element} class={['caption', size, { sub }]} {...rest}>{@render children?.()}</svelte:element>

<style>
	.caption {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		margin: 0;
		padding: 0 var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: normal;
	}

	.md,
	.sub {
		padding-inline: var(--space-3);
	}

	.md {
		font-weight: 500;
	}

	.sub {
		min-height: var(--control-min);
		color: color-mix(in oklab, var(--muted-fg) 75%, transparent);
	}
</style>
