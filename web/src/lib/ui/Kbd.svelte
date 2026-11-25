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
		border-radius: var(--radius-sm);
		background: var(--surface-2);
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		line-height: 1.5;
		text-align: center;
		white-space: nowrap;
	}
</style>
