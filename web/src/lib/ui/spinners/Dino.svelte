<svelte:options namespace="svg" />

<script lang="ts">
	let { delay = 0 }: { delay?: number } = $props();
</script>

<g class="body" style:--delay="{delay}s">
	<path class="far" d="M182 143 L217 208 L244 212 L244 230 L196 230 L158 157 Z" />
	<path class="tail" d="M100 78 C60 76 18 70 -18 56 C4 98 50 140 100 160 Z" />
	<ellipse cx="135" cy="115" rx="78" ry="52" transform="rotate(-12 135 115)" />
	<path d="M176 92 C190 70 196 50 203 30 L246 38 C238 62 226 92 200 116 Z" />
	<path d="M203 30 C203 10 223 2 245 8 L284 22 C294 26 292 42 282 44 L245 52 C223 56 203 50 203 30 Z" />
	<path class="near" d="M106 144 L134 156 L108 222 L136 226 L136 244 L78 244 L80 222 Z" />
</g>

<style>
	.body { transform-origin: 137px 130px; animation: bob 0.5s ease-in-out var(--delay, 0s) var(--pulse); }
	.near { transform-origin: 120px 150px; animation: near 0.5s var(--delay, 0s) var(--pulse); }
	.far { opacity: 0.55; transform-origin: 170px 150px; animation: far 0.5s calc(var(--delay, 0s) - 0.25s) var(--pulse); }
	.tail { transform-origin: 110px 115px; animation: tail 0.25s ease-in-out var(--delay, 0s) var(--pulse) alternate; }
	@keyframes bob { 0%, 50%, 100% { transform: translateY(-12px) rotate(-2deg); } 20%, 70% { transform: translateY(10px) rotate(1.5deg); } 45%, 95% { transform: translateY(-16px) rotate(-2.5deg); } }
	/* Sweep the planted foot back slowly, then lift it and swing it forward fast, so the stride reads as running forward. */
	@keyframes near {
		0% { transform: translateY(0) rotate(-43deg); animation-timing-function: linear; }
		40% { transform: translateY(0) rotate(22deg); animation-timing-function: ease-in-out; }
		70% { transform: translateY(-22px) rotate(-10deg); animation-timing-function: ease-in-out; }
		100% { transform: translateY(0) rotate(-43deg); }
	}
	@keyframes far {
		0% { transform: translateY(0) rotate(0deg); animation-timing-function: linear; }
		40% { transform: translateY(0) rotate(65deg); animation-timing-function: ease-in-out; }
		70% { transform: translateY(-22px) rotate(33deg); animation-timing-function: ease-in-out; }
		100% { transform: translateY(0) rotate(0deg); }
	}
	@keyframes tail { from { transform: rotate(-5deg); } to { transform: rotate(6deg); } }
</style>
