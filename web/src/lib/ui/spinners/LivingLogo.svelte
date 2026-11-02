<script lang="ts">
	import Dino from '#lib/ui/spinners/Dino.svelte';

	const herd = [
		{ at: '67 74', scale: 0.58, delay: -0.2, opacity: 0.55 },
		{ at: '261 82', scale: 0.6, delay: -0.4, opacity: 0.55 },
		{ at: '123 186', scale: 0.92, delay: 0, opacity: 1 }
	];
</script>

<svg viewBox="57 51 392 392" aria-hidden="true">
	{#each herd as { at, scale, delay, opacity } (at)}
		<g transform="translate({at}) scale({scale}) rotate(7 140 140)" {opacity}><Dino {delay} /></g>
	{/each}
</svg>

<style>
	svg { flex: none; width: 1.2em; height: 1.2em; fill: currentColor; }
</style>
