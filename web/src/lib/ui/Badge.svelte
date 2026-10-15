<script lang="ts">
	import type { Snippet } from 'svelte';

	let { tone = 'neutral', children }: { tone?: 'neutral' | 'working' | 'waiting' | 'done'; children: Snippet } =
		$props();
</script>

<span class={tone}>{@render children()}</span>

<style>
	span {
		display: inline-grid;
		place-items: center;
		box-sizing: border-box;
		min-width: 1.6em;
		height: 1.6em;
		padding: 0 var(--space-1);
		border-radius: 0.8em;
		background: var(--surface-3);
		color: var(--color-fg);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
		line-height: 1;
	}

	.working {
		background: var(--status-working);
		color: var(--color-bg);
	}

	.waiting {
		background: var(--status-waiting);
		color: var(--color-bg);
	}

	.done {
		background: var(--status-done);
		color: var(--color-bg);
	}
</style>
