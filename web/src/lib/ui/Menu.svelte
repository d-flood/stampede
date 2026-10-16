<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const bits = lazy(() => import('#lib/ui/BitsMenu.svelte'));
</script>

<script lang="ts">
	import type BitsMenu from '#lib/ui/BitsMenu.svelte';
	import type { ComponentProps } from 'svelte';

	let { items, context = false, trigger }: Omit<ComponentProps<typeof BitsMenu>, 'open'> = $props();

	let open = $state(false);
	const Popup = $derived(bits());
</script>

{#if Popup}
	<Popup {items} {context} {trigger} bind:open />
{:else}
	{@render trigger({ onclick: () => ((open = !context), warmUp()) })}
{/if}
