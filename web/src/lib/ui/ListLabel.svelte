<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = Omit<HTMLAttributes<HTMLElement>, 'class'> & {
		element?: 'div' | 'p' | 'h3' | 'li';
		/** Labels a run of `md` ListItems within a group, in line with their titles. */
		sub?: boolean;
	};

	let { element = 'div', sub = false, children, ...rest }: Props = $props();
</script>

<svelte:element this={element} class={['caption', { sub }]} {...rest}>{@render children?.()}</svelte:element>

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

	/* Icons render at 1.2em of the row's text, so this lands on an md ListItem's title. */
	.sub {
		min-height: var(--control-min);
		padding-inline-start: calc(2 * var(--space-3) + 1.2 * var(--font-size-md));
		color: color-mix(in oklab, var(--muted-fg) 75%, transparent);
	}
</style>
