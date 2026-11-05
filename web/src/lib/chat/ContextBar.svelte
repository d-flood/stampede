<script lang="ts">
	import { cold, describe, warmth } from '#lib/chat/cache.ts';
	import { count } from '#lib/chat/count.ts';
	import type { Context, Tokens } from '#lib/device.svelte.ts';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import { cubicOut } from 'svelte/easing';
	import { prefersReducedMotion, Tween } from 'svelte/motion';

	let { context, tokens }: { context: Context; tokens?: Tokens | null } = $props();

	const percent = $derived(Math.round((100 * context.used) / context.window));
	const shown = Tween.of(() => percent, { duration: () => (prefersReducedMotion.current ? 0 : 800), easing: cubicOut });
	const figure = $derived(`${count(context.used)} / ${count(context.window)}`);
	const state = $derived(context.cache && warmth(context.cache));
	const tip = $derived([`Context ${figure} (${percent}%)`, tokens && `${count(tokens.input)} in · ${count(tokens.output)} out`, context.cache && describe(context.cache)].filter(Boolean).join('\n'));
</script>

<Tooltip text={tip} delayDuration={0}>
	{#snippet children(props)}
		<span
			{...props}
			class="context"
			class:high={percent >= 90}
			class:cold={cold(state)}
			role="meter"
			aria-label="Context"
			aria-valuemin={0}
			aria-valuemax={context.window}
			aria-valuenow={Math.min(context.used, context.window)}
			aria-valuetext="{figure} ({percent}%){cold(state) ? `, cache ${state?.replace('-', ' ')}` : ''}"
		>
			<svg viewBox="0 0 40 40" aria-hidden="true">
				<circle class="track" cx="20" cy="20" r="17" />
				<circle class="fill" cx="20" cy="20" r="17" pathLength="100" stroke-dasharray="{Math.min(shown.current, 100)} 100" transform="rotate(-90 20 20)" />
			</svg>
			<span class="percent">{Math.round(shown.current)}%</span>
		</span>
	{/snippet}
</Tooltip>

<style>
	.context {
		position: relative;
		display: grid;
		flex: none;
		place-items: center;
		width: 30px;
		height: 30px;
		color: var(--muted-fg);
	}

	svg,
	.percent {
		grid-area: 1 / 1;
	}

	svg {
		width: 100%;
		height: 100%;
		fill: none;
		stroke-width: 3;
	}

	.track {
		stroke: var(--border);
	}

	.fill {
		stroke: color-mix(in oklab, var(--color-fg) 75%, var(--color-bg));
	}

	.percent {
		font-family: var(--font-mono);
		font-size: 9px;
		font-variant-numeric: tabular-nums;
	}

	.high .fill {
		stroke: var(--ansi-3);
	}

	.high .percent {
		color: var(--ansi-3);
	}

	.cold .fill {
		stroke: var(--ansi-1);
	}

	.cold .percent {
		color: var(--ansi-1);
	}

	.cold::after {
		position: absolute;
		top: 0;
		right: 0;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--ansi-1);
		content: '';
	}
</style>
