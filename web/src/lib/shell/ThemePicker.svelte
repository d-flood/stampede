<script lang="ts">
	import { selectedTheme, themeLabel } from '#lib/config.svelte.ts';
	import { other, ThemeChoice, themeBar, type Slot } from '#lib/themes.svelte.ts';
	import { onDestroy, tick } from 'svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { Command } from 'bits-ui';
	import Check from '~icons/ph/check';
	import CaretLeft from '~icons/ph/caret-left';
	import CaretRight from '~icons/ph/caret-right';
	import Moon from '~icons/ph/moon';
	import Sun from '~icons/ph/sun';

	let { onselect, onpreview }: { onselect(): void; onpreview(choice: ThemeChoice): void } = $props();

	const schemes = [
		{ scheme: 'light', label: 'Light', Icon: Sun },
		{ scheme: 'dark', label: 'Dark', Icon: Moon }
	] as const;
	const choice = themeBar.choice ?? new ThemeChoice();
	themeBar.choice = undefined;
	let docked = false;
	onDestroy(() => docked || choice.end());

	function describe(slot: Slot) {
		return choice.chosen[slot] ?? (choice.chosen[other(slot)] ? `Same as ${other(slot)}` : 'Default');
	}

	async function apply() {
		if (await choice.apply()) onselect();
	}

	function dock() {
		docked = true;
		onpreview(choice);
	}
</script>

<p class="current">Saved: {themeLabel()}</p>
<div class="schemes" role="group" aria-label="Color scheme">
	{#each schemes as { scheme: slot, label, Icon } (slot)}
		<button type="button" aria-pressed={choice.scheme === slot} disabled={choice.saving} onclick={() => choice.pick(slot)}>
			<Icon aria-hidden="true" />
			<span>{label}</span>
			<strong>{describe(slot)}</strong>
		</button>
	{/each}
</div>
<div class="cycler">
	<IconButton label="Previous theme" disabled={choice.saving || choice.matches.length === 0} onclick={() => choice.cycle(-1)}><CaretLeft /></IconButton>
	<div class="preview" role="status" aria-live="polite">
		<span>{choice.pending ? 'Loading preview…' : 'Preview'}</span>
		<strong>{choice.candidate ?? 'Default'}</strong>
	</div>
	<IconButton label="Next theme" disabled={choice.saving || choice.matches.length === 0} onclick={() => choice.cycle(1)}><CaretRight /></IconButton>
</div>
<Command.Root shouldFilter={false} loop>
	<Command.Input placeholder="Search {choice.all ? 'all' : choice.scheme} themes" aria-label="Search themes" autocomplete="off" spellcheck={false}>
		{#snippet child({ props })}
			<TextField {...props} bind:value={choice.query} {@attach (element) => { tick().then(() => element.focus()); }} />
		{/snippet}
	</Command.Input>
	<Command.List aria-label="Ghostty themes" aria-busy={choice.loading || choice.saving}>
		{#snippet child({ props })}
			<div {...props} class="list">
				<Command.Viewport>
					{#each choice.matches as name (name)}
						<Command.Item value={name} disabled={choice.saving} onSelect={() => choice.choose(name)}>
						{#snippet child({ props })}
							<div {...props} class="item">
								<span>{name}</span>
								{#if name === selectedTheme[choice.scheme]}<Check aria-label="Current theme" />{/if}
							</div>
						{/snippet}
						</Command.Item>
					{:else}
						<p class="status">{choice.loading ? 'Loading themes…' : 'No matching themes'}</p>
					{/each}
				</Command.Viewport>
			</div>
		{/snippet}
	</Command.List>
</Command.Root>
<label class="all"><input type="checkbox" bind:checked={choice.all} disabled={choice.saving} /> Show all themes</label>
<div class="actions">
	<Button variant="ghost" disabled={choice.saving} onclick={dock}>Preview in app</Button>
	<Button onclick={onselect}>Cancel</Button>
	<Button variant="primary" disabled={!choice.ready || choice.spec === selectedTheme.name} onclick={apply}>Apply theme</Button>
</div>
{#if choice.saving}<p class="status" role="status">Applying theme…</p>{/if}
{#if choice.error}
	<p class="error" role="alert">{choice.error}</p>
	{#if choice.names.length === 0}<Button onclick={() => choice.load()}>Retry</Button>{/if}
{/if}

<style>
	.schemes {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
	}

	.schemes button {
		display: grid;
		grid-template-columns: auto 1fr;
		align-items: center;
		column-gap: var(--space-2);
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		color: var(--muted-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.schemes button:hover:not(:disabled) {
		background: var(--hover);
	}

	.schemes button:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	.schemes button[aria-pressed='true'] {
		border-color: var(--status-working);
		background: var(--surface-3);
		color: var(--color-fg);
	}

	.schemes strong {
		grid-column: 1 / -1;
		overflow: hidden;
		color: var(--color-fg);
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.cycler {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
	}

	.preview {
		flex: 1;
		min-width: 0;
		text-align: center;
		overflow-wrap: anywhere;
	}

	.preview span {
		display: block;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-2);
		margin-top: var(--space-3);
	}

	.actions > :global(:first-child) {
		margin-inline-end: auto;
	}

	.current,
	.status {
		margin: 0 0 var(--space-3);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}

	.list {
		max-height: min(24rem, 50dvh);
		margin-top: var(--space-2);
		overflow-y: auto;
	}

	.all {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		margin-top: var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		cursor: pointer;
	}

	.item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.item span {
		overflow-wrap: anywhere;
	}

	.item[data-selected] {
		background: var(--hover);
	}

	.item[data-disabled] {
		opacity: 0.5;
		cursor: wait;
	}

	.error {
		color: var(--ansi-1);
		overflow-wrap: anywhere;
	}
</style>
