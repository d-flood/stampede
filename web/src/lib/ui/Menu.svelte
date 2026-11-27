<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const bits = lazy(() => import('#lib/ui/BitsMenu.svelte'));
</script>

<script lang="ts">
	import type BitsMenu from '#lib/ui/BitsMenu.svelte';
	import type { ComponentProps } from 'svelte';

	type Props = Omit<ComponentProps<typeof BitsMenu>, 'open'> & { onopen?: () => void };

	let { items, context = false, trigger, onopen, ...rest }: Props = $props();

	let open = $state(false);
	let engaged = $state(false);
	const Popup = $derived(bits());

	function toggle(value: boolean) {
		open = value;
		if (!open) return;
		engaged = true;
		onopen?.();
	}
</script>

<!-- Dropdowns mount their popup on first open; a context menu needs the opening event's position. -->
{#if Popup && (context || engaged)}
	<Popup {items} {context} {trigger} {...rest} bind:open={() => open, toggle} />
{:else if context}
	{@render trigger({ onclick: warmUp })}
{:else}
	{@render trigger({
		'aria-haspopup': 'menu',
		'aria-expanded': false,
		onclick: () => (toggle(true), warmUp()),
		onkeydown: (event: KeyboardEvent) => event.key === 'ArrowDown' && (event.preventDefault(), toggle(true), warmUp())
	})}
{/if}
