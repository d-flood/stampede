<script lang="ts">
	import Dino from '#lib/ui/spinners/Dino.svelte';
</script>

<svg viewBox="-45 -53 340 340" aria-hidden="true">
	<path class="ground" d="M-45 258 H295" />
	{#each [[78, 0.2], [122, 0.45]] as [x, delay] (x)}<circle class="puff" cx={x} cy="244" r="20" style:transform-origin="{x}px 244px" style:animation-delay="{delay}s" />{/each}
	<Dino />
</svg>

<style>
	svg { flex: none; width: 1.2em; height: 1.2em; fill: currentColor; }
	/* Scroll the ground at the planted foot's speed so the dino reads as running forward. */
	.ground { fill: none; stroke: currentColor; stroke-width: 9; stroke-linecap: round; stroke-dasharray: 40 80; opacity: 0.4; animation: ground 0.24s linear var(--pulse); }
	.puff { opacity: 0; animation: puff 0.5s ease-out var(--pulse); }
	@keyframes ground { to { stroke-dashoffset: 120; } }
	@keyframes puff { from { transform: scale(0.3); opacity: 0.6; } to { transform: translate(-60px, -16px) scale(1.3); opacity: 0; } }
</style>
