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
		border-bottom-width: calc(2 * var(--border-width));
		border-radius: var(--radius-sm);
		background: var(--color-bg);
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		line-height: 1.5;
		text-align: center;
		white-space: nowrap;
	}
</style>
