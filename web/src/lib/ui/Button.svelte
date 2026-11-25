<script lang="ts">
	import ProgressLine from '#lib/ui/ProgressLine.svelte';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import { mergeProps } from 'bits-ui';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	type Props = HTMLButtonAttributes & { variant?: 'default' | 'primary' | 'ghost' | 'danger' | 'link'; size?: 'sm' | 'lg'; round?: boolean; tip?: string; busy?: boolean };

	let { variant = 'default', size, round = false, tip, busy, children: content, ...rest }: Props = $props();
</script>

{#snippet button(props: HTMLButtonAttributes)}
	<button class={[variant, size, { round, busy, progress: busy !== undefined }]} aria-busy={busy || undefined} {...props}>
		{@render content?.()}
		{#if busy !== undefined}<ProgressLine active={busy} />{/if}
	</button>
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
		background: var(--surface-3);
	}

	button:active:not(:disabled) {
		scale: 0.97;
	}

	button:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	button:disabled {
		border-color: var(--border);
		background: var(--surface-1);
		color: var(--muted-fg);
		cursor: default;
	}

	.primary {
		--fill: var(--accent);
		border-color: transparent;
		background: var(--fill);
		color: var(--color-bg);
		font-weight: 600;
	}

	.primary:hover:not(:disabled) {
		--fill: color-mix(in oklab, var(--accent), var(--color-fg) 8%);
		background: var(--fill);
	}

	.primary:disabled {
		border-color: transparent;
		background: var(--surface-2);
		color: var(--muted-fg);
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

	.progress {
		position: relative;
		overflow: clip;
	}

	.busy {
		cursor: progress;
	}

	.primary.busy,
	.primary.busy:hover:not(:disabled) {
		--fill: color-mix(in oklab, var(--accent) 22%, var(--surface-2));
		color: var(--color-fg);
	}

	.danger:not(:disabled) {
		border-color: color-mix(in oklab, var(--ansi-1) 45%, var(--border-strong));
		color: var(--ansi-1);
	}

	.danger:hover:not(:disabled) {
		background: color-mix(in oklab, var(--ansi-1) 14%, var(--surface-2));
	}

	.sm {
		gap: var(--space-1);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		font-size: var(--font-size-sm);
	}

	.lg {
		padding: 0 var(--space-4);
		font-size: var(--font-size-lg);
	}

	.round {
		width: max(var(--target-min), 32px);
		height: max(var(--target-min), 32px);
		padding: 0;
		border-radius: var(--radius-pill);
	}

	.link {
		padding: 0;
		border-color: transparent;
		background: none;
		box-shadow: none;
		color: var(--accent);
		font-weight: inherit;
	}

	.link:hover:not(:disabled) {
		background: none;
		text-decoration: underline;
	}

	.link:active:not(:disabled) {
		box-shadow: none;
	}

	.link:disabled {
		border-color: transparent;
		background: none;
	}
</style>
