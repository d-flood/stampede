<script lang="ts">
	import type { HTMLSelectAttributes } from 'svelte/elements';

	let { value = $bindable(), children, ...rest }: HTMLSelectAttributes = $props();
</script>

<select bind:value {...rest}>{@render children?.()}</select>

<style>
	select {
		box-sizing: border-box;
		width: 100%;
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		text-overflow: ellipsis;
	}

	select:focus-visible {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: 1px;
	}

	select:disabled {
		color: var(--muted-fg);
	}
</style>
