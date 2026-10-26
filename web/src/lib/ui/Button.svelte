<script lang="ts">
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import { mergeProps } from 'bits-ui';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	let { variant = 'default', tip, children: content, ...rest }: HTMLButtonAttributes & { variant?: 'default' | 'primary' | 'danger'; tip?: string } =
		$props();
</script>

{#snippet button(props: HTMLButtonAttributes)}
	<button class={variant} {...props}>{@render content?.()}</button>
{/snippet}

{#if tip}
	<Tooltip text={tip} delayDuration={0} ignoreNonKeyboardFocus>
		{#snippet children(trigger)}
			{@render button(mergeProps(trigger, rest))}
		{/snippet}
	</Tooltip>
{:else}
	{@render button(rest)}
{/if}

<style>
	button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		color: var(--color-fg);
		font: inherit;
		cursor: pointer;
		transition: background var(--dur-fast);
	}

	button:hover:not(:disabled) {
		background: var(--hover);
	}

	button:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	button:disabled {
		color: var(--muted-fg);
		cursor: default;
	}

	.primary {
		border-color: var(--status-working);
		background: var(--status-working);
		color: var(--color-bg);
	}

	.primary:hover:not(:disabled) {
		background: color-mix(in oklab, var(--status-working) 85%, var(--color-fg));
	}

	.danger:not(:disabled) {
		color: var(--ansi-1);
	}
</style>
