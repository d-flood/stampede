<svg viewBox="0 0 24 24" aria-hidden="true">
	<g class="hand">
		<path d="M5.4 14.2 H18.6 V16.4 C18.6 20.2 15.8 23 12 23 C8.2 23 5.4 20.2 5.4 16.4 Z" />
		<g transform="translate(7 15) rotate(-20)"><rect x="-1.6" y="-3" width="3.2" height="5" rx="1.28" /><path d="M-1.6 -2.7 C-1.6 -7.9 0.72 -10 1.8 -9.8 C0.72 -6.85 1.6 -5.8 1.6 -2.7 Z" /></g>
		<g transform="translate(12 14.4)"><rect x="-1.65" y="-3.6" width="3.3" height="5.6" rx="1.32" /><path d="M-1.65 -3.3 C-1.65 -8.92 0.64 -11.2 1.6 -11 C0.64 -7.78 1.65 -6.64 1.65 -3.3 Z" /></g>
		<g transform="translate(17 15) rotate(20) scale(-1 1)"><rect x="-1.6" y="-3" width="3.2" height="5" rx="1.28" /><path d="M-1.6 -2.7 C-1.6 -7.9 0.72 -10 1.8 -9.8 C0.72 -6.85 1.6 -5.8 1.6 -2.7 Z" /></g>
	</g>
</svg>

<style>
	svg { flex: none; width: 1.2em; height: 1.2em; fill: currentColor; }
	.hand { transform-origin: 12px 23px; animation: wave 1.8s ease-in-out var(--pulse); }
	@keyframes wave { 0%, 55%, 100% { transform: none; } 10% { transform: rotate(-14deg); } 22% { transform: rotate(12deg); } 34% { transform: rotate(-9deg); } 45% { transform: rotate(5deg); } }
</style>
