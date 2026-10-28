<script lang="ts">
	import type { Status } from '#lib/device.svelte.ts';
	import { cubicOut } from 'svelte/easing';
	import { prefersReducedMotion } from 'svelte/motion';
	import { fade, fly } from 'svelte/transition';
	import ShellMark from '#lib/ui/ShellMark.svelte';
	import CheckBold from '~icons/ph/check-bold';
	import HandPalmFill from '~icons/ph/hand-palm-fill';

	let { status, activity, shell }: { status: Status; activity?: string | null; shell?: string } = $props();

	const background = $derived(status === 'done' && !!shell);
	const label = $derived(status === 'waiting' ? 'Waiting for your response' : background ? shell : activity || 'Working…');
	const rise = $derived({ y: prefersReducedMotion.current ? 0 : 4, duration: 200 });
	/** How far the comet's head runs ahead of its tail's dash start, as a share of the path, per the dash lengths below. */
	const HEAD = 0.155;
	let ring = $state<SVGSVGElement>();

	/** Rides a mark `lead` of the way along its `layer` image on the comet's head while the comet passes behind `node`. */
	function ride(lead: number, layer: 'background' | 'mask', at: (x: number, progress: number) => Keyframe) {
		return (node: HTMLElement) => {
			const tail = ring?.querySelector<SVGRectElement>('.tail');
			if (!ring || !tail) return;
			const field = ring;
			let animation: Animation | undefined;
			const place = () => {
				const comet = status !== 'waiting' && tail.getAnimations()[0];
				if (!comet) return;
				return comet.ready.then(() => {
					if (comet !== tail.getAnimations()[0]) return;
					animation?.cancel();
					const box = field.getBoundingClientRect();
					const rect = node.getBoundingClientRect();
					const radius = parseFloat(getComputedStyle(tail).rx);
					const [left, straight] = [box.left + 0.5 + radius, box.width - 1 - 2 * radius];
					const perimeter = 2 * (straight + box.height - 1 - 2 * radius) + 2 * Math.PI * radius;
					const corner = (radius * Math.PI) / 2;
					const across = (distance: number) => left + (distance < 0 ? -radius * Math.sin(Math.min(-distance, corner) / radius) : distance > straight ? straight + radius * Math.sin(Math.min(distance - straight, corner) / radius) : distance);
					const along = (x: number) => (x < left ? -radius * Math.asin(Math.min(1, (left - x) / radius)) : x > left + straight ? straight + radius * Math.asin(Math.min(1, (x - left - straight) / radius)) : x - left);
					const orbit = Number(comet.effect?.getComputedTiming().duration);
					const size = parseFloat(getComputedStyle(node)[`${layer}Size`]);
					const [from, to] = [along(rect.left + (lead - 1) * size), along(rect.right + lead * size)];
					const arcs = Array.from({ length: 12 }, (_, step) => ((step + 1) / 12) * corner);
					const marks = [from, (from + to) / 2, to, 0, straight, ...arcs.map((arc) => -arc), ...arcs.map((arc) => straight + arc)].filter((mark) => mark >= from && mark <= to).sort((a, b) => a - b);
					const frames: Keyframe[] = marks.map((mark) => ({ offset: (mark - from) / perimeter, ...at(across(mark) - rect.left - lead * size, (mark - from) / (to - from)) }));
					const gone = { offset: (marks[marks.length - 1] - from) / perimeter, ...at(rect.width, 1), easing: 'steps(1, end)' };
					animation = node.animate([...frames, gone, { offset: 1, ...at(-size, 0) }], { duration: orbit, iterations: Infinity });
					animation.startTime = Number(comet.startTime) + ((((from / perimeter - HEAD) % 1) + 1) % 1) * orbit - orbit;
				}, () => {});
			};
			place();
			const observer = new ResizeObserver(place);
			observer.observe(field);
			return () => {
				observer.disconnect();
				animation?.cancel();
			};
		};
	}

	function sweep(_: Element) {
		const drawn = (u: number) => (prefersReducedMotion.current ? 1 : cubicOut(Math.min(1, u * 2)));
		return { duration: 1400, css: (_: number, u: number) => `stroke-dashoffset: ${100 * (1 - drawn(u))}; opacity: ${Math.min(1, (1 - u) * 2.5)}` };
	}

	function flash(_: Element) {
		return { duration: 1400, css: (_: number, u: number) => `visibility: visible; opacity: ${Math.min(1, u * 6, (1 - u) * 2.5)}` };
	}
</script>

{#if status !== 'done' || background}
	<svg bind:this={ring} class="ring" class:waiting={status === 'waiting'} class:background aria-hidden="true">
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
	<span class="legend" class:waiting={status === 'waiting'} class:background role="status" aria-label="Current activity" in:fly={rise} out:fade={{ duration: 150 }}>
		{#if status === 'waiting'}<HandPalmFill />{:else if background}<ShellMark />{/if}
		{#key label}
			<span class="trace" aria-hidden="true" {@attach ride(1, 'background', (x) => ({ backgroundPosition: `${x}px 50%` }))}></span>
			<span class="lit" in:fly={rise}>
				<span class="label" {@attach ride(0.92, 'background', (x, progress) => ({ backgroundPosition: `${x}px 0, 0 0`, filter: `brightness(${1 + 0.3 * Math.sin(Math.PI * progress)})` }))}>{label}</span>
				<span class="bloom" aria-hidden="true" {@attach ride(0.5, 'mask', (x) => ({ maskPosition: `${x}px 0` }))}>{label}</span>
			</span>
		{/key}
		{#if background}<span class="after">· resumes after</span>{/if}
	</span>
	<span class="legend done" aria-hidden="true" out:flash><CheckBold /> Done</span>
{/if}

<style>
	.ring { --orbit: 2.6s; position: absolute; inset: 0; width: 100%; height: 100%; overflow: visible; pointer-events: none; }
	rect { x: 0.5px; y: 0.5px; width: calc(100% - 1px); height: calc(100% - 1px); rx: calc(var(--radius-xl) - 0.5px); fill: none; stroke: var(--status-working); stroke-linecap: round; transition: opacity var(--dur-med); }
	.track { opacity: 0.35; }
	.waiting .track { stroke: var(--status-waiting); opacity: 1; }
	.ring.background { --orbit: 8s; }
	.background .track { stroke: var(--status-background); opacity: 0.25; }
	.background .comet { opacity: 0.6; }
	.background .comet rect { stroke: var(--status-background); }
	.background .comet .body, .background .comet .glow { display: none; }
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
	.sweep { stroke: var(--status-done); stroke-width: 2; stroke-linecap: butt; stroke-dasharray: 100 100; stroke-dashoffset: 100; opacity: 0; }
	.legend { --tone: var(--status-working); position: absolute; top: calc(var(--border-width) / 2); left: 50%; z-index: 1; display: flex; align-items: center; gap: var(--space-1); max-width: calc(100% - 4 * var(--radius-xl)); padding: 0 var(--space-4); translate: -50% -50%; color: var(--status-working); font-size: var(--font-size-sm); line-height: 1.4; white-space: nowrap; pointer-events: none; }
	.legend :global(svg) { flex: none; }
	.legend::before, .trace { --feather: linear-gradient(90deg, transparent, #000 var(--space-3), #000 calc(100% - var(--space-3)), transparent); position: absolute; inset: 0; mask: var(--feather); pointer-events: none; }
	.legend::before { content: ''; z-index: -1; background: linear-gradient(var(--color-bg) 50%, var(--surface-1) 0); }
	.trace { background: linear-gradient(90deg, transparent, color-mix(in oklab, var(--tone) 60%, transparent)) -5em 50% / 5em 1.5px no-repeat; }
	.lit { position: relative; display: flex; min-width: 0; }
	.label { position: relative; min-width: 0; overflow: hidden; text-overflow: ellipsis; background: linear-gradient(90deg, transparent, color-mix(in oklab, var(--tone), var(--color-fg) 35%) 60%, color-mix(in oklab, var(--tone), var(--color-fg) 85%) 92%, transparent) -7em 0 / 7em 100% no-repeat, var(--tone); background-clip: text; color: transparent; }
	.bloom { position: absolute; inset: -0.6em 0; padding-block: 0.6em; overflow: hidden; text-overflow: ellipsis; color: color-mix(in oklab, var(--tone), var(--color-fg) 75%); text-shadow: 0 0 4px var(--tone), 0 0 10px var(--tone); mask: linear-gradient(90deg, transparent, #000 45%, #000 55%, transparent) -5em 0 / 5em 100% no-repeat; pointer-events: none; }
	.legend.waiting { color: var(--status-waiting); }
	.legend.background { --tone: var(--status-background); color: var(--status-background); }
	.waiting .label { background: none; color: inherit; }
	.waiting .bloom, .waiting .trace { display: none; }
	.after { flex: none; color: var(--muted-fg); }
	.done { color: var(--status-done); visibility: hidden; opacity: 0; }
	@keyframes orbit { to { stroke-dashoffset: -100; } }
	@keyframes breathe { 50% { opacity: 0.5; } }
	@media (prefers-reduced-motion: reduce) {
		.comet { display: none; }
		.track { opacity: 1; }
	}
</style>
