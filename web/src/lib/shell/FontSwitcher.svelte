<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const picker = lazy(() => import('#lib/shell/FontPicker.svelte'));
</script>

<script lang="ts">
	import { defaultFonts, fontNames, fontVariables, fonts, type FontRole } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import SelectTrigger from '#lib/ui/SelectTrigger.svelte';

	let { role }: { role: FontRole } = $props();
	let open = $state(false);
	const Picker = $derived(picker());
</script>

<SelectTrigger
	value={fonts[role] ?? defaultFonts[role]}
	caret="right"
	aria-label="Change {fontNames[role]}"
	title="{fontNames[role]}: {fonts[role] ?? 'Default'}"
	aria-haspopup="dialog"
	aria-expanded={open}
	onclick={() => ((open = true), warmUp())}
>
	{#snippet leading()}<span class="glyphs" aria-hidden="true" style:font-family="var({fontVariables[role]})">Aa</span>{/snippet}
</SelectTrigger>
<Dialog
	bind:open
	full={form() === 'phone'}
	title={fontNames[role]}
	description="Fonts installed on this Host. Apply to save on every device; closing restores your saved font."
>
	{#if Picker}<Picker {role} onselect={() => (open = false)} />{/if}
</Dialog>

<style>



	.glyphs {
		display: grid;
		flex: none;
		place-items: center;
		min-width: 28px;
		height: 22px;
		border-radius: var(--radius-sm);
		background: var(--surface-2);
		font-size: var(--font-size-md);
		font-weight: 600;
	}


</style>
