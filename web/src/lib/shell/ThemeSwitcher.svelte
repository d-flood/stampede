<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const picker = lazy(() => import('#lib/shell/ThemePicker.svelte'));
</script>

<script lang="ts">
	import { themeLabel } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Palette from '~icons/ph/palette';

	let open = $state(false);
	const Picker = $derived(picker());
</script>

<IconButton
	label="Change theme"
	title="Theme: {themeLabel()}"
	aria-haspopup="dialog"
	aria-expanded={open}
	onclick={() => ((open = true), warmUp())}
><Palette /></IconButton>
<Dialog
	bind:open
	full={form() === 'phone'}
	title="Choose a theme"
	description="Pick one theme for light mode and one for dark. Apply to save on every device; closing restores your saved themes."
>
	{#if Picker}<Picker onselect={() => (open = false)} />{/if}
</Dialog>
