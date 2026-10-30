<script lang="ts">
	import type { HTMLTextareaAttributes } from 'svelte/elements';

	let { value = $bindable(), ref = $bindable(), ...rest }: HTMLTextareaAttributes & { ref?: HTMLTextAreaElement } = $props();
</script>

<textarea bind:this={ref} bind:value {...rest}></textarea>

<style>
	textarea {
		display: block;
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
		line-height: 1.5;
		resize: vertical;
	}

	textarea:focus-visible {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: 1px;
	}
</style>
