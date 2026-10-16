<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const bits = lazy(() => import('#lib/ui/BitsMenu.svelte'));
</script>

<script lang="ts">
	import type BitsMenu from '#lib/ui/BitsMenu.svelte';
	import type { ComponentProps } from 'svelte';

	type Props = Omit<ComponentProps<typeof BitsMenu>, 'open'> & { onopen?: () => void };

	let { items, context = false, trigger, onopen }: Props = $props();

	let open = $state(false);
	const Popup = $derived(bits());

	function toggle(value: boolean) {
		open = value;
		if (open) onopen?.();
	}
</script>

{#if Popup}
	<Popup {items} {context} {trigger} bind:open={() => open, toggle} />
{:else}
	{@render trigger({ onclick: () => (toggle(!context), warmUp()) })}
{/if}
