<script lang="ts">
	import type { Agent } from '#lib/config.svelte.ts';
	import TabIcon, { type Variant } from '#lib/ui/TabIcon.svelte';
	import TabMarks, { type Status } from '#lib/ui/TabMarks.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import { mergeProps } from 'bits-ui';
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import X from '~icons/ph/x';

	type Props = HTMLButtonAttributes & {
		variant: Variant;
		label: string;
		tooltip?: string;
		agent?: Agent;
		against?: string;
		status?: Status;
		/** Never run yet, so a done status means ready rather than finished. */
		fresh?: boolean;
		active?: boolean;
		dirty?: boolean;
		deleted?: boolean;
		ended?: boolean;
		closing?: boolean;
		/** Hangs from the content above, as on a phone's bottom strip, instead of rising into the content below. */
		bottom?: boolean;
		onclose?: () => void;
	};

	let {
		variant,
		label,
		tooltip,
		agent = 'claude-code',
		against,
		status,
		fresh = false,
		active = false,
		dirty = false,
		deleted = false,
		ended = false,
		closing = false,
		bottom = false,
		onclose,
		...rest
	}: Props = $props();


	// Measuring waits for the frame, so it reuses that frame's layout instead of forcing one mid-update.
	function reveal(tab: HTMLElement) {
		const frame = requestAnimationFrame(() => {
			const list = tab.closest<HTMLElement>('[role="tablist"]');
			if (!list) return;
			const shown = list.getBoundingClientRect();
			const { left, right } = tab.getBoundingClientRect();
			if (left < shown.left) list.scrollLeft -= shown.left - left;
			else if (right > shown.right) list.scrollLeft += right - shown.right;
		});
		return () => cancelAnimationFrame(frame);
	}
</script>

{#snippet select(props: HTMLButtonAttributes)}
	<button
		role="tab"
		aria-selected={active}
		aria-disabled={closing || undefined}
		title={closing ? 'Closing…' : undefined}
		class="select {variant} {status ?? ''}"
		class:active
		class:deleted
		class:ended
		class:closing
		{...props}
	>
		<TabIcon {variant} {agent} {label} />
		<span class="label">{label}</span>
		{#if variant === 'diff' && against}<span class="vs">· {against}</span>{/if}
		<TabMarks {status} {dirty} {fresh} />
	</button>
{/snippet}

<div class="tab {status ?? ''}" class:bottom class:closable={active && !!onclose} {@attach active && reveal}>
	{#if tooltip}
		<Tooltip text={tooltip} delayDuration={300} ignoreNonKeyboardFocus>
			{#snippet children(trigger)}
				{@render select(mergeProps(trigger, rest))}
			{/snippet}
		</Tooltip>
	{:else}
		{@render select(rest)}
	{/if}
	{#if active && onclose}
		<span class="close">
			<IconButton
				type="button"
				label="Close {label}"
				title="Close {label} (Ctrl/Cmd+W)"
				onpointerdown={(event) => event.stopPropagation()}
				onclick={(event) => {
					event.stopPropagation();
					onclose?.();
				}}
			><X /></IconButton>
		</span>
	{/if}
</div>

<style>
	.tab {
		position: relative;
		display: inline-flex;
		flex: 0 1 auto;
		min-width: 5rem;
		max-width: 13rem;
		border-radius: var(--radius-lg) var(--radius-lg) 0 0;
	}

	.tab.closable {
		flex-shrink: 0;
	}

	.bottom {
		min-width: 8rem;
		border-radius: 0 0 var(--radius-lg) var(--radius-lg);
	}

	.select {
		position: relative;
		display: inline-flex;
		flex: 1;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		min-height: calc(var(--target-min) + var(--space-2));
		margin-bottom: calc(-1 * var(--border-width));
		padding: 0 var(--space-3);
		border: var(--border-width) solid transparent;
		border-bottom: none;
		border-radius: inherit;
		background: none;
		color: var(--muted-fg);
		font: inherit;
		white-space: nowrap;
		cursor: pointer;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	.select:not(.active, .closing):hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.bottom .select {
		font-size: var(--font-size-sm);
		margin-top: calc(-1 * var(--border-width));
		margin-bottom: 0;
		border: var(--border-width) solid transparent;
		border-top: none;
	}

	.bottom .active {
		border-color: transparent;
		background: none;
	}

	.bottom .active::before {
		content: '';
		position: absolute;
		top: 0;
		left: 50%;
		width: calc(2 * var(--tile));
		height: calc(2 * var(--border-width));
		border-radius: var(--radius-pill);
		background: var(--accent);
		translate: -50%;
	}

	.closable .select {
		padding-right: calc(var(--target-min) + var(--space-3));
	}

	/* Puts a status or unsaved mark in the close button's slot and swaps them on hover. */
	@media (hover: hover) {
		.closable:not(.bottom):has(:global(:is(.mark, .dirty))) .select {
			padding-right: var(--space-3);
		}

		.closable:not(.bottom):has(:global(:is(.mark, .dirty))):not(:hover, :focus-within) .close {
			opacity: 0;
		}

		.closable:not(.bottom):is(:hover, :focus-within) .select :global(:is(.mark, .dirty)) {
			opacity: 0;
		}
	}

	.select > :global(svg) {
		flex: none;
	}

	.close {
		position: absolute;
		top: 50%;
		right: var(--space-1);
		display: inline-flex;
		transform: translateY(-50%);
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.active {
		border-color: var(--border);
		background: var(--color-bg);
		color: var(--color-fg);
	}

	.deleted .label {
		text-decoration: line-through;
	}

	.ended {
		color: var(--muted-fg);
		font-weight: 400;
		font-style: italic;
	}

	.closing {
		color: color-mix(in oklab, var(--muted-fg) 55%, var(--color-bg));
		font-weight: 400;
		cursor: default;
	}

	.closing :global(svg) {
		color: inherit;
	}

	.terminal .label {
		font-family: var(--font-mono);
	}

	.vs {
		color: var(--muted-fg);
		font-weight: 400;
	}

	.waiting {
		background: color-mix(in oklab, var(--status-waiting) 16%, transparent);
	}
</style>
