<script lang="ts">
	import { selectedTheme } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import { closeThemeBar, type ThemeChoice } from '#lib/themes.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import FloatingBar from '#lib/ui/FloatingBar.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';
	import Moon from '~icons/ph/moon';
	import Sun from '~icons/ph/sun';

	let { choice }: { choice: ThemeChoice } = $props();

	const schemes = [
		{ scheme: 'light', label: 'Light theme', Icon: Sun },
		{ scheme: 'dark', label: 'Dark theme', Icon: Moon }
	] as const;
	const index = $derived(choice.matches.indexOf(choice.chosen[choice.scheme] ?? ''));

	function keydown(event: KeyboardEvent) {
		if (event.key === 'Escape') closeThemeBar();
		else if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
			event.preventDefault();
			choice.cycle(event.key === 'ArrowLeft' ? -1 : 1);
		}
	}

	async function apply() {
		if (await choice.apply()) closeThemeBar();
	}
</script>

<FloatingBar role="region" aria-label="Theme preview" docked={form() === 'phone'} belowDialogs>
	<!-- svelte-ignore a11y_no_static_element_interactions (Arrow keys cycle themes from any control in the bar.) -->
	<div class="content" class:docked={form() === 'phone'} onkeydown={keydown}>
		<div class="schemes" role="group" aria-label="Color scheme">
			{#each schemes as { scheme, label, Icon } (scheme)}
				<IconButton {label} title={label} pressed={choice.scheme === scheme} disabled={choice.saving} onclick={() => choice.pick(scheme)}><Icon /></IconButton>
			{/each}
		</div>
		<div class="cycler">
			<IconButton label="Previous theme" disabled={choice.saving} onclick={() => choice.cycle(-1)}><CaretLeft /></IconButton>
			<div class="name" role="status" aria-live="polite">
				<strong>{choice.candidate ?? 'Default'}</strong>
				<span>{choice.loading || choice.pending ? 'Loading…' : index < 0 ? `${choice.matches.length} themes` : `${index + 1} of ${choice.matches.length}`}</span>
			</div>
			<IconButton label="Next theme" disabled={choice.saving} onclick={() => choice.cycle(1)} {@attach (element) => element.focus()}><CaretRight /></IconButton>
		</div>
		<div class="actions">
			<Button variant="ghost" onclick={closeThemeBar}>Cancel</Button>
			<Button variant="primary" disabled={!choice.ready || choice.spec === selectedTheme.name} onclick={apply}>Apply</Button>
		</div>
		{#if choice.error}<p role="alert">{choice.error}</p>{/if}
	</div>
</FloatingBar>

<style>
	.content {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		width: min(36rem, 100vw - 4 * var(--space-4));
	}

	.content.docked {
		width: 100%;
	}

	.schemes,
	.actions {
		display: flex;
		flex: none;
		gap: var(--space-1);
	}

	.cycler {
		display: flex;
		flex: 1;
		align-items: center;
		min-width: 0;
	}

	.docked .cycler {
		order: -1;
		flex-basis: 100%;
	}

	.docked .actions {
		margin-inline-start: auto;
	}

	.name {
		flex: 1;
		min-width: 0;
		text-align: center;
	}

	.name strong {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.name span {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
	}

	p {
		flex-basis: 100%;
		margin: 0;
		color: var(--ansi-1);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}
</style>
