<script lang="ts">
	import { count } from '#lib/chat/count.ts';
	import type { Context } from '#lib/device.svelte.ts';

	let { context, compact = false }: { context: Context; compact?: boolean } = $props();

	const percent = $derived(Math.round((100 * context.used) / context.window));
	const figure = $derived(`${count(context.used)} / ${count(context.window)}`);
</script>

<div
	class="context"
	class:compact
	role="meter"
	aria-label="Context"
	aria-valuemin={0}
	aria-valuemax={context.window}
	aria-valuenow={Math.min(context.used, context.window)}
	aria-valuetext="{figure} ({percent}%)"
>
	{#if !compact}<span>Context</span>{/if}
	<span class="track"><span class="fill" style:width="{Math.min(percent, 100)}%"></span></span>
	<span class="figure">{compact ? `${percent}%` : figure}</span>
</div>

<style>
	.context {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.track {
		width: 120px;
		height: 4px;
		border-radius: var(--radius-sm);
		background: var(--surface-3);
	}

	.compact .track {
		width: 48px;
	}

	.fill {
		display: block;
		height: 100%;
		border-radius: inherit;
		background: color-mix(in oklab, var(--color-fg) 75%, var(--color-bg));
	}

	.figure {
		font-family: var(--font-mono);
	}
</style>
