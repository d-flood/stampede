<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const picker = lazy(() => import('#lib/shell/FontPicker.svelte'));
</script>

<script lang="ts">
	import { uiFont } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import TextAa from '~icons/ph/text-aa';

	let open = $state(false);
	const Picker = $derived(picker());
</script>

<IconButton
	label="Change font"
	title="Font: {uiFont.family ?? 'Default'}"
	aria-haspopup="dialog"
	aria-expanded={open}
	onclick={() => ((open = true), warmUp())}
><TextAa /></IconButton>
<Dialog
	bind:open
	full={form() === 'phone'}
	title="Choose a font"
	description="Fonts installed on this Host. Apply to save on every device; closing restores your saved font."
>
	{#if Picker}<Picker onselect={() => (open = false)} />{/if}
</Dialog>
