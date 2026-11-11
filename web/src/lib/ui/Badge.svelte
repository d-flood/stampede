<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';

	type Props = HTMLAttributes<HTMLSpanElement> & { tone?: 'neutral' | 'working' | 'waiting' | 'done' | 'soft' | 'outline'; children: Snippet };

	let { tone = 'neutral', children, ...rest }: Props = $props();
</script>

<span class={tone} {...rest}>{@render children()}</span>

<style>
	span {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-1);
		box-sizing: border-box;
		min-width: 1.6em;
		height: 1.6em;
		padding: 0 var(--space-1);
		border-radius: 0.8em;
		background: var(--border);
		color: var(--color-fg);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
		line-height: 1;
		white-space: nowrap;
	}

	span :global(svg) {
		flex: none;
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

	.soft {
		background: color-mix(in oklab, currentColor 18%, transparent);
		color: inherit;
	}

	.outline {
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		background: none;
		color: var(--muted-fg);
		font-weight: normal;
	}
</style>
