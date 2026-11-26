<script lang="ts">
	import type { HTMLAttributes } from 'svelte/elements';

	let { children, ...rest }: HTMLAttributes<HTMLElement> = $props();
</script>

<kbd {...rest}>{@render children?.()}</kbd>

<style>
	kbd {
		display: inline-flex;
		box-sizing: border-box;
		flex: none;
		align-items: baseline;
		justify-content: center;
		gap: var(--space-1);
		min-width: 1.6em;
		padding: 0 var(--space-2);
		border: var(--border-width) solid color-mix(in lab, var(--color-fg) 14%, transparent);
		border-radius: var(--radius-sm);
		background: color-mix(in lab, var(--color-fg) 4%, transparent);
		color: var(--muted-fg);
		font: inherit;
		font-size: calc(var(--font-size-sm) - 1px);
		font-variant-numeric: tabular-nums;
		line-height: 1.5;
		text-align: center;
		white-space: nowrap;
	}
</style>
