<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';

	let { children, ...rest }: HTMLAttributes<HTMLElement> = $props();
</script>

<kbd {...rest}>{@render children?.()}</kbd>

<style>
	kbd {
		display: inline-block;
		box-sizing: border-box;
		flex: none;
		min-width: 1.6em;
		padding: 0 var(--space-1);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-sm);
		color: color-mix(in oklab, var(--muted-fg) 80%, transparent);
		font: inherit;
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
		line-height: 1.4;
		text-align: center;
		white-space: nowrap;
	}
</style>
