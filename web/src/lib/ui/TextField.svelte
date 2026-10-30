<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';

	let { value = $bindable(), ref = $bindable(), mono = false, ...rest }: HTMLInputAttributes & { ref?: HTMLInputElement; mono?: boolean } = $props();
</script>

<input bind:this={ref} bind:value class:mono {...rest} />

<style>
	input {
		box-sizing: border-box;
		width: 100%;
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
	}

	.mono {
		font-family: var(--font-mono);
	}

	input:focus-visible {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: 1px;
	}

	input[aria-invalid='true'] {
		border-color: var(--ansi-1);
	}
</style>
