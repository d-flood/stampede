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
		flush?: boolean;
		portal?: boolean;
		trigger: Snippet<[Record<string, unknown>]>;
		children: Snippet;
		onopenautofocus?: (event: Event) => void;
		oncloseautofocus?: (event: Event) => void;
	};

	let { label, open = $bindable(false), ref = $bindable(), side = 'bottom', align = 'start', width, flush = false, portal = true, trigger, children, onopenautofocus, oncloseautofocus }: Props = $props();
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			{@render trigger(props)}
		{/snippet}
	</Popover.Trigger>
	<Popover.Portal disabled={!portal}>
		<Popover.Content {side} {align} sideOffset={4} collisionPadding={8} onOpenAutoFocus={onopenautofocus} onCloseAutoFocus={oncloseautofocus}>
			{#snippet child({ wrapperProps, props })}
				<div {...wrapperProps}>
					<div {...props} bind:this={ref} class="popover" class:flush role="dialog" aria-label={label} tabindex="-1" style:width={width && `min(${width}, 100vw - 2 * var(--space-4))`}>
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
		border: var(--border-width) solid color-mix(in lab, var(--float) 88%, var(--color-fg));
		border-radius: var(--radius-lg);
		--hover: color-mix(in lab, var(--float) 90%, var(--color-fg));
		background: var(--float);
		box-shadow:
			0 1px 2px color-mix(in oklab, var(--ansi-0) 10%, transparent),
			0 6px 18px -6px color-mix(in oklab, var(--ansi-0) 24%, transparent);
		color: var(--color-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-md);
		outline: none;
	}

	.flush {
		display: flex;
		flex-direction: column;
		padding: 0;
		overflow: hidden;
		--bare-radius: calc(var(--radius-lg) - var(--border-width));
	}
</style>
