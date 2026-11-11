<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = Omit<HTMLAttributes<HTMLElement>, 'class'> & { element?: 'span' | 'h2' | 'h3' | 'legend' };

	let { element = 'span', children, ...rest }: Props = $props();
</script>

<svelte:element this={element} class="eyebrow" {...rest}>{@render children?.()}</svelte:element>

<style>
	.eyebrow {
		margin: 0;
		padding: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}
</style>
