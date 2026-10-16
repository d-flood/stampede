<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const picker = lazy(() => import('#lib/shell/ThemePicker.svelte'));
</script>

<script lang="ts">
	import { selectedTheme } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Palette from '~icons/ph/palette';

	let open = $state(false);
	const Picker = $derived(picker());
</script>

<IconButton
	label="Change theme"
	title="Theme: {selectedTheme.name ?? 'Default'}"
	aria-haspopup="dialog"
	aria-expanded={open}
	onclick={() => ((open = true), warmUp())}
><Palette /></IconButton>
<Dialog
	bind:open
	full={form() === 'phone'}
	title="Choose a theme"
	description="Applies to the UI, editor, and terminals on every device."
>
	{#if Picker}<Picker onselect={() => (open = false)} />{/if}
</Dialog>
