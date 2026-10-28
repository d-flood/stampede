<script lang="ts">
	import { selectedTheme } from '#lib/config.svelte.ts';
	import { form } from '#lib/form.ts';
	import { closeThemeBar, type ThemeChoice } from '#lib/themes.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
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

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (Arrow keys cycle themes from any control in the bar.) -->
<section class="bar" class:docked={form() === 'phone'} aria-label="Theme preview" onkeydown={keydown}>
	<div class="schemes" role="group" aria-label="Color scheme">
		{#each schemes as { scheme, label, Icon } (scheme)}
			<IconButton {label} title={label} aria-pressed={choice.scheme === scheme} disabled={choice.saving} onclick={() => choice.pick(scheme)}><Icon /></IconButton>
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
</section>

<style>
	.bar {
		position: fixed;
		bottom: var(--space-6);
		left: 50%;
		z-index: 45;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		box-sizing: border-box;
		width: min(38rem, calc(100vw - 2 * var(--space-4)));
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-family: var(--font-ui);
		translate: -50%;
	}

	.docked {
		position: static;
		flex: none;
		width: auto;
		border-width: var(--border-width) 0 0;
		border-radius: 0;
		box-shadow: none;
		translate: none;
	}

	.schemes,
	.actions {
		display: flex;
		flex: none;
		gap: var(--space-1);
	}

	.schemes :global([aria-pressed='true']) {
		background: var(--surface-3);
		color: var(--color-fg);
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
