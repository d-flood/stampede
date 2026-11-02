<svg viewBox="-45 -53 340 340" aria-hidden="true">
	{#each [0, -0.2, -0.4] as delay (delay)}<circle class="puff" cx="70" cy="240" r="18" style:animation-delay="{delay}s" />{/each}
	<g class="body">
		<path class="far" d="M182 143 L217 208 L244 212 L244 230 L196 230 L158 157 Z" />
		<path class="tail" d="M100 78 C60 76 18 70 -18 56 C4 98 50 140 100 160 Z" />
		<ellipse cx="135" cy="115" rx="78" ry="52" transform="rotate(-12 135 115)" />
		<path d="M176 92 C190 70 196 50 203 30 L246 38 C238 62 226 92 200 116 Z" />
		<path d="M203 30 C203 10 223 2 245 8 L284 22 C294 26 292 42 282 44 L245 52 C223 56 203 50 203 30 Z" />
		<path class="near" d="M106 144 L134 156 L108 222 L136 226 L136 244 L78 244 L80 222 Z" />
	</g>
</svg>

<style>
	svg { flex: none; width: 1.2em; height: 1.2em; fill: currentColor; }
	.puff { opacity: 0; transform-origin: 70px 240px; animation: puff 0.6s ease-out var(--pulse); }
	.body { transform-origin: 137px 130px; animation: bob 0.15s ease-in-out var(--pulse) alternate; }
	.far { opacity: 0.55; transform-origin: 170px 150px; animation: far 0.6s ease-in-out var(--pulse); }
	.near { transform-origin: 120px 150px; animation: near 0.6s ease-in-out var(--pulse); }
	.tail { transform-origin: 110px 115px; animation: tail 0.3s ease-in-out var(--pulse) alternate; }
	@keyframes puff { from { transform: scale(0.25); opacity: 0.55; } to { transform: translate(-70px, -12px) scale(1.1); opacity: 0; } }
	@keyframes bob { from { transform: translateY(5px); } to { transform: translateY(-9px) rotate(-1.5deg); } }
	@keyframes far { 0%, 100% { transform: rotate(-6deg); } 50% { transform: rotate(58deg); } }
	@keyframes near { 0%, 100% { transform: rotate(22deg); } 50% { transform: rotate(-42deg); } }
	@keyframes tail { from { transform: rotate(-3deg); } to { transform: rotate(4deg); } }
</style>
