<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { Snippet } from 'svelte';

	let { text, children }: { text: string; children: Snippet<[Record<string, unknown>]> } = $props();
</script>

<Tooltip.Provider delayDuration={400}>
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				{@render children(props)}
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Content sideOffset={4}>
				{#snippet child({ wrapperProps, props })}
					<div {...wrapperProps}>
						<div {...props} class="tooltip">{text}</div>
					</div>
				{/snippet}
			</Tooltip.Content>
		</Tooltip.Portal>
	</Tooltip.Root>
</Tooltip.Provider>

<style>
	.tooltip {
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--surface-3);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-sm);
	}
</style>
