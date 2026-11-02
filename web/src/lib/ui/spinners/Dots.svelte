<span class="dots" aria-hidden="true"><span></span><span></span><span></span></span>

<style>
	.dots {
		display: flex;
		gap: 3px;
	}

	.dots span {
		width: 3px;
		height: 3px;
		border-radius: 50%;
		background: currentColor;
		animation: pulse 1.2s ease-in-out var(--pulse);
	}

	.dots span:nth-child(2) { animation-delay: 150ms; }
	.dots span:nth-child(3) { animation-delay: 300ms; }

	@keyframes pulse {
		0%, 80%, 100% { opacity: 0.3; }
		40% { opacity: 1; }
	}
</style>
