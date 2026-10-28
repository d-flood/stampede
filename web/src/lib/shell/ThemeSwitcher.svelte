<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const picker = lazy(() => import('#lib/shell/ThemePicker.svelte'));
</script>

<script lang="ts">
	import { themeLabel } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import CaretRight from '~icons/ph/caret-right';

	let open = $state(false);
	const Picker = $derived(picker());
</script>

<button
	class="swatch"
	aria-label="Change theme"
	title="Theme: {themeLabel()}"
	aria-haspopup="dialog"
	aria-expanded={open}
	onclick={() => ((open = true), warmUp())}
>
	<span class="colors" aria-hidden="true">
		{#each [1, 2, 3, 4, 5, 6] as color (color)}<span style:background="var(--ansi-{color})"></span>{/each}
	</span>
	<span class="name">{themeLabel()}</span>
	<CaretRight />
</button>
<Dialog
	bind:open
	full={form() === 'phone'}
	title="Choose a theme"
	description="Pick one theme for light mode and one for dark. Apply to save on every device; closing restores your saved themes."
>
	{#if Picker}<Picker onselect={() => (open = false)} />{/if}
</Dialog>

<style>
	.swatch {
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

	.swatch:hover {
		border-color: color-mix(in oklab, var(--status-working) 60%, var(--border-strong));
	}

	.swatch:focus-visible {
		outline: 2px solid color-mix(in oklab, var(--status-working) 70%, transparent);
		outline-offset: 1px;
	}

	.colors {
		display: grid;
		flex: none;
		grid-template-columns: repeat(3, 8px);
		gap: 2px;
		padding: 3px;
		border-radius: var(--radius-sm);
		background: var(--surface-2);
	}

	.colors span {
		height: 8px;
		border-radius: 2px;
	}

	.name {
		flex: 1;
		min-width: 0;
		text-align: start;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.swatch > :global(svg) {
		flex: none;
		color: var(--muted-fg);
	}
</style>
