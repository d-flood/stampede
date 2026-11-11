<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = Omit<HTMLAttributes<HTMLElement>, 'class'> & { element?: 'div' | 'p' | 'h3' | 'li' };

	let { element = 'div', children, ...rest }: Props = $props();
</script>

<svelte:element this={element} class="caption" {...rest}>{@render children?.()}</svelte:element>

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
</style>
