<svg viewBox="0 0 24 24" aria-hidden="true">
	<ellipse class="shadow" cx="12" cy="21.6" rx="5" ry="0.9" />
	<g class="egg">
		<path class="line" d="M12 3 C16.5 3 19 9.5 19 14 C19 18.5 16 21 12 21 C8 21 5 18.5 5 14 C5 9.5 7.5 3 12 3 Z" stroke-width="1.6" />
		<path class="line" d="M5.8 12.2 L8.6 13.8 L10.8 11.6 L13.4 13.8 L15.6 11.8 L18.2 13.2" stroke-width="1.3" />
		<circle cx="9.5" cy="8.5" r="0.9" />
		<circle cx="14.5" cy="17" r="1" />
		<circle cx="10" cy="17.6" r="0.7" />
	</g>
</svg>

<style>
	svg { flex: none; width: 1.2em; height: 1.2em; fill: currentColor; }
	.shadow { opacity: 0.25; }
	.line { fill: none; stroke: currentColor; stroke-linecap: round; stroke-linejoin: round; }
	.egg { transform-origin: 12px 21px; animation: wobble 1.6s ease-in-out var(--pulse); }
	@keyframes wobble { 0%, 40%, 72%, 100% { transform: none; } 46% { transform: rotate(-13deg); } 52% { transform: rotate(11deg); } 58% { transform: rotate(-7deg); } 64% { transform: rotate(4deg); } }
</style>
