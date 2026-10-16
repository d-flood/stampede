<script lang="ts">
	import '#lib/tokens.css';

	let { children } = $props();
</script>

{@render children()}

<style>
	:global(html) {
		overscroll-behavior-y: none;
	}

	:global(body) {
		margin: 0;
		background: var(--color-bg);
		color: var(--color-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-md);
	}
</style>
