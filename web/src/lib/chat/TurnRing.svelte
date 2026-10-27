<script lang="ts">
	import type { Status } from '#lib/device.svelte.ts';
	import { cubicOut } from 'svelte/easing';
	import { prefersReducedMotion } from 'svelte/motion';
	import { fade, fly } from 'svelte/transition';
	import CheckBold from '~icons/ph/check-bold';
	import HandPalmFill from '~icons/ph/hand-palm-fill';

	let { status, activity }: { status: Status; activity?: string | null } = $props();

	const label = $derived(status === 'waiting' ? 'Waiting for your response' : activity || 'Working…');
	const rise = $derived({ y: prefersReducedMotion.current ? 0 : 4, duration: 200 });

	function sweep(_: Element) {
		const drawn = (u: number) => (prefersReducedMotion.current ? 1 : cubicOut(Math.min(1, u * 2)));
		return { duration: 1400, css: (_: number, u: number) => `stroke-dashoffset: ${100 * (1 - drawn(u))}; opacity: ${Math.min(1, (1 - u) * 2.5)}` };
	}

	function flash(_: Element) {
		return { duration: 1400, css: (_: number, u: number) => `visibility: visible; opacity: ${Math.min(1, u * 6, (1 - u) * 2.5)}` };
	}
</script>

{#if status !== 'done'}
	<svg class="ring" class:waiting={status === 'waiting'} aria-hidden="true">
		<rect class="track" out:fade />
		<rect class="halo" out:fade />
		<g class="comet" out:fade>
			<rect class="tail" pathLength="100" />
			<rect class="body" pathLength="100" />
			<rect class="glow" pathLength="100" />
			<rect class="head" pathLength="100" />
		</g>
		<rect class="sweep" pathLength="100" out:sweep />
	</svg>
	<span class="legend" class:waiting={status === 'waiting'} role="status" aria-label="Current activity" in:fly={rise} out:fade={{ duration: 150 }}>
		{#if status === 'waiting'}<HandPalmFill />{/if}
		{#key label}<span class="label" in:fly={rise}>{label}</span>{/key}
	</span>
	<span class="legend done" aria-hidden="true" out:flash><CheckBold /> Done</span>
{/if}

<style>
	.ring { --orbit: 2.6s; position: absolute; inset: 0; width: 100%; height: 100%; overflow: visible; pointer-events: none; }
	rect { x: 0.5px; y: 0.5px; width: calc(100% - 1px); height: calc(100% - 1px); rx: calc(var(--radius-xl) - 0.5px); fill: none; stroke: var(--status-working); stroke-linecap: round; transition: opacity var(--dur-med); }
	.track { opacity: 0.35; }
	.waiting .track { stroke: var(--status-waiting); opacity: 1; }
	.halo { stroke: var(--status-waiting); stroke-width: 6; filter: blur(3px); opacity: 0; }
	.waiting .halo { animation: breathe 2.4s ease-in-out var(--pulse); }
	.comet { transition: opacity var(--dur-med); }
	.waiting .comet { opacity: 0; }
	/* Negative delays push each shorter dash ahead so all heads align into one tapered comet. */
	.comet rect { animation: orbit var(--orbit) linear var(--pulse); }
	.comet .tail { stroke-dasharray: 18 82; stroke-width: 1.5; opacity: 0.25; }
	.comet .body { stroke-dasharray: 11 89; stroke-width: 1.5; opacity: 0.55; animation-delay: calc(var(--orbit) * -0.07); }
	.comet .glow { stroke-dasharray: 6 94; stroke-width: 6; filter: blur(3px); opacity: 0.8; animation-delay: calc(var(--orbit) * -0.12); }
	.comet .head { stroke: color-mix(in oklab, var(--status-working), var(--color-fg) 40%); stroke-dasharray: 5 95; stroke-width: 2; animation-delay: calc(var(--orbit) * -0.13); }
	.sweep { stroke: var(--status-done); stroke-width: 2; stroke-linecap: butt; stroke-dasharray: 100 100; stroke-dashoffset: 100; }
	.legend { position: absolute; top: calc(var(--border-width) / 2); left: 50%; z-index: 1; display: flex; align-items: center; gap: var(--space-1); max-width: calc(100% - 4 * var(--radius-xl)); padding: 0 var(--space-2); translate: -50% -50%; background: linear-gradient(var(--color-bg) 50%, var(--surface-1) 0); color: var(--status-working); font-size: var(--font-size-sm); line-height: 1.4; white-space: nowrap; pointer-events: none; }
	.legend :global(svg) { flex: none; }
	.label { min-width: 0; overflow: hidden; text-overflow: ellipsis; background: linear-gradient(90deg, var(--status-working) 35%, color-mix(in oklab, var(--status-working), var(--color-fg) 70%) 50%, var(--status-working) 65%) 100% 0 / 250% 100%; background-clip: text; color: transparent; animation: shimmer 2.2s linear var(--pulse); }
	.legend.waiting { color: var(--status-waiting); }
	.waiting .label { background: none; color: inherit; animation: none; }
	.done { color: var(--status-done); visibility: hidden; opacity: 0; }
	@keyframes orbit { to { stroke-dashoffset: -100; } }
	@keyframes breathe { 50% { opacity: 0.5; } }
	@keyframes shimmer { to { background-position: 0 0; } }
	@media (prefers-reduced-motion: reduce) {
		.comet { display: none; }
		.track { opacity: 1; }
	}
</style>
