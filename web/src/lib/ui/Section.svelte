<script lang="ts">
	import Eyebrow from '#lib/ui/Eyebrow.svelte';
	import type { Snippet } from 'svelte';

	let { title, actions, children }: { title: string; actions?: Snippet; children: Snippet } = $props();
</script>

<section>
	<header>
		<Eyebrow element="h2">{title}</Eyebrow>
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

	header > :global(h2) {
		flex: 1;
	}

	div {
		padding: var(--space-3);
		overflow: auto;
	}
</style>
