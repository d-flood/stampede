<script lang="ts">
	import { colorScheme, selectScheme, type Scheme } from '#lib/scheme.svelte.ts';
	import IconButton from '#lib/ui/IconButton.svelte';
	import CircleHalf from '~icons/ph/circle-half';
	import Moon from '~icons/ph/moon';
	import Sun from '~icons/ph/sun';

	const order: Scheme[] = ['light', 'dark', 'system'];
	const names: Record<Scheme, string> = { light: 'Light', dark: 'Dark', system: 'System' };
	const next = $derived(order[(order.indexOf(colorScheme.choice) + 1) % order.length]);
</script>

<IconButton
	label="Color scheme: {names[colorScheme.choice]}"
	title="Color scheme: {names[colorScheme.choice]}. Click for {names[next]}."
	onclick={() => selectScheme(next)}
>
	{#if colorScheme.choice === 'light'}<Sun />{:else if colorScheme.choice === 'dark'}<Moon />{:else}<CircleHalf />{/if}
</IconButton>
