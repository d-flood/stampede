<script lang="ts">
	import type { Snippet } from 'svelte';

	let { label, children }: { label: string; children: Snippet } = $props();
</script>

<section aria-label={label} class="suggestions">{@render children()}</section>

<style>
	.suggestions {
		position: absolute;
		right: var(--space-2);
		bottom: calc(100% + var(--space-1));
		left: var(--space-2);
		z-index: 1;
		display: flex;
		flex-direction: column;
		max-height: min(24rem, 50vh);
		padding: var(--space-1);
		overflow-y: auto;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
	}
</style>
