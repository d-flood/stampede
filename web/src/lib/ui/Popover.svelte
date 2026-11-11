<script lang="ts">
	import { Popover } from 'bits-ui';
	import type { Snippet } from 'svelte';

	type Props = {
		label: string;
		open?: boolean;
		ref?: HTMLElement;
		side?: 'top' | 'bottom';
		align?: 'start' | 'end';
		width?: string;
		portal?: boolean;
		trigger: Snippet<[Record<string, unknown>]>;
		children: Snippet;
		onopenautofocus?: (event: Event) => void;
	};

	let { label, open = $bindable(false), ref = $bindable(), side = 'bottom', align = 'start', width, portal = true, trigger, children, onopenautofocus }: Props = $props();
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			{@render trigger(props)}
		{/snippet}
	</Popover.Trigger>
	<Popover.Portal disabled={!portal}>
		<Popover.Content {side} {align} sideOffset={4} collisionPadding={8} onOpenAutoFocus={onopenautofocus}>
			{#snippet child({ wrapperProps, props })}
				<div {...wrapperProps}>
					<div {...props} bind:this={ref} class="popover" role="dialog" aria-label={label} tabindex="-1" style:width={width && `min(${width}, 100vw - 2 * var(--space-4))`}>
						{@render children()}
					</div>
				</div>
			{/snippet}
		</Popover.Content>
	</Popover.Portal>
</Popover.Root>

<style>
	.popover {
		z-index: 50;
		box-sizing: border-box;
		max-height: min(var(--bits-floating-available-height), 100dvh - 2 * var(--space-2));
		overflow-y: auto;
		padding: var(--space-3) var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-md);
		outline: none;
	}
</style>
