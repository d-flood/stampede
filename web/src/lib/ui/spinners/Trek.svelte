<script lang="ts">
	const tracks = [0, 1, 2, 3].map((k) => ({ x: 7.8 + 13.8 * k, y: k % 2 ? 14.5 : 9.5, turn: k % 2 ? 100 : 80 }));
</script>

<svg viewBox="0 0 43.2 24" aria-hidden="true">
	<g class="walk">
		{#each tracks as { x, y, turn }, k (k)}
			<g class:first={k === 2} class:second={k === 3} style:transform-origin="{x + 1.2}px {y}px">
				<g transform="translate({x} {y}) rotate({turn}) scale(0.92)">
					<ellipse cy="2" rx="3.2" ry="2.7" />
					<path d="M0 0 L0 -6 M-1 .6 L-4.6 -4.2 M1 .6 L4.6 -4.2" />
				</g>
			</g>
		{/each}
	</g>
</svg>

<style>
	svg { flex: none; width: 2.16em; height: 1.2em; fill: currentColor; mask-image: linear-gradient(90deg, transparent, #000 45%); }
	path { fill: none; stroke: currentColor; stroke-width: 2.6; stroke-linecap: round; }
	/* Slide two strides per loop so the last two prints land where the first two began, hiding the wrap. */
	.walk { animation: walk 1s ease-in-out var(--pulse); }
	.first { animation: stamp-first 1s ease-out var(--pulse); }
	.second { animation: stamp-second 1s ease-out var(--pulse); }
	@keyframes walk { 50% { transform: translateX(-13.8px); } 100% { transform: translateX(-27.6px); } }
	@keyframes stamp-first { 0% { opacity: 0; transform: scale(1.5); } 12%, 100% { opacity: 1; transform: none; } }
	@keyframes stamp-second { 0%, 50% { opacity: 0; transform: scale(1.5); } 62%, 100% { opacity: 1; transform: none; } }
</style>
