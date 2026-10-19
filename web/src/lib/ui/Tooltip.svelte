<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { Snippet } from 'svelte';

	let { text, delayDuration = 400, disabled = false, children }: { text: string; delayDuration?: number; disabled?: boolean; children: Snippet<[Record<string, unknown>]> } = $props();
</script>

<Tooltip.Provider {delayDuration}>
	<Tooltip.Root>
		<Tooltip.Trigger {disabled}>
			{#snippet child({ props })}
				{@render children(props)}
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Content sideOffset={4} collisionPadding={8}>
				{#snippet child({ wrapperProps, props })}
					<div {...wrapperProps}>
						<div {...props} class="tooltip" role="tooltip">{text}</div>
					</div>
				{/snippet}
			</Tooltip.Content>
		</Tooltip.Portal>
	</Tooltip.Root>
</Tooltip.Provider>

<style>
	.tooltip {
		z-index: 50;
		max-width: min(28rem, calc(100vw - 16px));
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--surface-3);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-sm);
		line-height: 1.5;
		white-space: pre-line;
	}
</style>
