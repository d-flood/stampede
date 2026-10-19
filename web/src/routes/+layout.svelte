<script lang="ts">
	import { hostName } from '#lib/config.svelte.ts';
	import '#lib/tokens.css';

	let { children } = $props();
</script>

<svelte:head><title>{hostName() ?? 'Stampede'}</title></svelte:head>

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
