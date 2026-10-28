<script lang="ts">
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import { mergeProps } from 'bits-ui';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	let { variant = 'default', tip, children: content, ...rest }: HTMLButtonAttributes & { variant?: 'default' | 'primary' | 'ghost' | 'danger'; tip?: string } =
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
		--sheen: color-mix(in oklab, var(--color-fg) 8%, transparent);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-md);
		background: linear-gradient(color-mix(in oklab, var(--surface-2), var(--color-fg) 3%), var(--surface-2));
		box-shadow:
			inset 0 1px 0 var(--sheen),
			var(--shadow-lift);
		color: var(--color-fg);
		font: inherit;
		font-weight: 500;
		white-space: nowrap;
		cursor: pointer;
		transition:
			background var(--dur-fast),
			border-color var(--dur-fast),
			box-shadow var(--dur-fast),
			color var(--dur-fast),
			opacity var(--dur-fast),
			scale var(--dur-fast) var(--ease-out);
	}

	button:hover:not(:disabled) {
		background: linear-gradient(color-mix(in oklab, var(--surface-3), var(--color-fg) 3%), var(--surface-3));
	}

	button:active:not(:disabled) {
		box-shadow: inset 0 1px 2px color-mix(in oklab, var(--ansi-0) 25%, transparent);
		scale: 0.97;
	}

	button:focus-visible {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: 1px;
	}

	button:disabled {
		border-color: var(--border);
		background: var(--surface-1);
		box-shadow: none;
		color: var(--muted-fg);
		cursor: default;
	}

	.primary {
		--fill: var(--status-working);
		border-color: color-mix(in oklab, var(--fill), var(--color-fg) 16%);
		background: linear-gradient(color-mix(in oklab, var(--fill), var(--color-fg) 3%), var(--fill));
		color: var(--color-bg);
		font-weight: 600;
	}

	.primary:hover:not(:disabled) {
		--fill: color-mix(in oklab, var(--status-working), var(--color-fg) 8%);
		background: linear-gradient(color-mix(in oklab, var(--fill), var(--color-fg) 3%), var(--fill));
	}

	.primary:disabled {
		border-color: color-mix(in oklab, var(--status-working) 22%, var(--border));
		background: color-mix(in oklab, var(--status-working) 7%, var(--surface-1));
		color: color-mix(in oklab, var(--status-working) 35%, var(--muted-fg));
	}

	.ghost {
		border-color: transparent;
		background: none;
		box-shadow: none;
		color: var(--muted-fg);
	}

	.ghost:hover:not(:disabled) {
		background: var(--hover);
		color: var(--color-fg);
	}

	.ghost:disabled {
		border-color: transparent;
		background: none;
		opacity: 0.45;
	}

	.danger:not(:disabled) {
		border-color: color-mix(in oklab, var(--ansi-1) 45%, var(--border-strong));
		color: var(--ansi-1);
	}

	.danger:hover:not(:disabled) {
		background: color-mix(in oklab, var(--ansi-1) 14%, var(--surface-2));
	}
</style>
