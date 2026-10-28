<script lang="ts">
	import '@fontsource-variable/crimson-pro/wght-italic.css';
	import '@fontsource-variable/crimson-pro/wght.css';
	import '@fontsource/fira-sans/400-italic.css';
	import '@fontsource/fira-sans/400.css';
	import '@fontsource/fira-sans/500.css';
	import '@fontsource/fira-sans/600.css';
	import '@fontsource/fira-sans/700.css';
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
