<script lang="ts">
	let { active }: { active: boolean } = $props();
</script>

<span class="line" class:active aria-hidden="true"></span>

<style>
	.line {
		position: absolute;
		inset: auto 0 0;
		height: calc(2 * var(--border-width));
		overflow: hidden;
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--dur-med);
	}

	.active {
		opacity: 1;
		transition-delay: var(--dur-fast);
	}

	.line::before {
		position: absolute;
		inset: 0 auto 0 0;
		width: 40%;
		border-radius: var(--radius-pill);
		background: linear-gradient(90deg, transparent, var(--accent), transparent);
		content: '';
		translate: -100% 0;
	}

	.active::before {
		animation: sweep 1.1s var(--ease-out) var(--pulse);
	}

	@media (prefers-reduced-motion: reduce) {
		.line::before {
			width: 100%;
			translate: none;
		}
	}

	@keyframes sweep {
		to {
			translate: 250% 0;
		}
	}
</style>
