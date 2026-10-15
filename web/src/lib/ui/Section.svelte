<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, actions, children }: { title: string; actions?: Snippet; children: Snippet } = $props();
</script>

<section>
	<header>
		<h2>{title}</h2>
		{@render actions?.()}
	</header>
	<div>{@render children()}</div>
</section>

<style>
	section {
		display: flex;
		flex-direction: column;
		min-height: 0;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
	}

	header {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-1) 0 var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
	}

	h2 {
		flex: 1;
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	div {
		padding: var(--space-3);
		overflow: auto;
	}
</style>
