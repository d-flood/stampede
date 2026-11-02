<script lang="ts">
	import ClawShape from '#lib/ui/spinners/ClawShape.svelte';
</script>

<svg viewBox="0 0 24 24" aria-hidden="true">
	<g class="hand"><ClawShape /></g>
</svg>

<style>
	svg { flex: none; width: 1.2em; height: 1.2em; fill: currentColor; }
	.hand { transform-origin: 12px 23px; animation: wave 1.8s ease-in-out var(--pulse); }
	@keyframes wave { 0%, 55%, 100% { transform: none; } 10% { transform: rotate(-14deg); } 22% { transform: rotate(12deg); } 34% { transform: rotate(-9deg); } 45% { transform: rotate(5deg); } }
</style>
