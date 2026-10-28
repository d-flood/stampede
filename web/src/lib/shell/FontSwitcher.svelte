<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const picker = lazy(() => import('#lib/shell/FontPicker.svelte'));
</script>

<script lang="ts">
	import { defaultFonts, fontNames, fontVariables, fonts, type FontRole } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import CaretRight from '~icons/ph/caret-right';

	let { role }: { role: FontRole } = $props();
	let open = $state(false);
	const Picker = $derived(picker());
</script>

<button
	class="specimen"
	aria-label="Change {fontNames[role]}"
	title="{fontNames[role]}: {fonts[role] ?? 'Default'}"
	aria-haspopup="dialog"
	aria-expanded={open}
	onclick={() => ((open = true), warmUp())}
>
	<span class="glyphs" aria-hidden="true" style:font-family="var({fontVariables[role]})">Aa</span>
	<span class="name">{fonts[role] ?? defaultFonts[role]}</span>
	<CaretRight />
</button>
<Dialog
	bind:open
	full={form() === 'phone'}
	title={fontNames[role]}
	description="Fonts installed on this Host. Apply to save on every device; closing restores your saved font."
>
	{#if Picker}<Picker {role} onselect={() => (open = false)} />{/if}
</Dialog>

<style>
	.specimen {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		box-sizing: border-box;
		width: 100%;
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2) var(--space-1) var(--space-1);
		border: var(--border-width) solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		cursor: pointer;
		transition: border-color var(--dur-fast);
	}

	.specimen:hover {
		border-color: color-mix(in oklab, var(--status-working) 60%, var(--border-strong));
	}

	.specimen:focus-visible {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: 1px;
	}

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

	.name {
		flex: 1;
		min-width: 0;
		text-align: start;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.specimen > :global(svg) {
		flex: none;
		color: var(--muted-fg);
	}
</style>
