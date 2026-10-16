<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const bits = lazy(() => import('#lib/ui/BitsDialog.svelte'));
</script>

<script lang="ts">
	import type BitsDialog from '#lib/ui/BitsDialog.svelte';
	import type { ComponentProps } from 'svelte';

	let { open = $bindable(false), ...rest }: ComponentProps<typeof BitsDialog> = $props();

	const Popup = $derived(bits());

	$effect(() => {
		if (open) warmUp();
	});
</script>

{#if Popup}
	<Popup bind:open {...rest} />
{/if}
