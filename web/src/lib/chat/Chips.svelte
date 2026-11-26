<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import { agentNames, type Agent } from '#lib/config.svelte.ts';
	import { chosen, type Choice, type Setting, type Settings } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Chip from '#lib/ui/Chip.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import type { Snippet } from 'svelte';
	import CaretDown from '~icons/ph/caret-down';

	type Props = {
		agent: Agent;
		settings: Settings;
		ready: boolean;
		choices: Record<Setting, Choice[]>;
		mark?: boolean;
		children?: Snippet;
		onrun(setting: Setting, value: string): void;
	};

	let { agent, settings, ready, choices, mark = true, children, onrun }: Props = $props();

	const names: Record<Setting, string> = { model: 'Model', effort: 'Effort', speed: 'Speed', ultracode: 'Ultracode', mode: 'Mode' };
	const phone = $derived(form() === 'phone');
	const Logo = $derived(logos[agent]);
	const tuning = ['model', 'effort', 'speed', 'ultracode'] as const;
	const optional = (setting: Setting) => setting === 'speed' || setting === 'ultracode';
	const value = (setting: Setting) => chosen(setting, settings, choices);
	const label = (setting: Setting) => choices[setting].find((choice) => choice.value === value(setting))?.label ?? value(setting);
	const pending = (setting: Setting) => !!settings.pending?.includes(setting);
	const shown = (setting: Setting) => (optional(setting) ? choices[setting].length > 0 && value(setting) !== choices[setting][0].value : !!settings[setting]);
	const parts = $derived(tuning.filter(shown).map((setting) => (setting === 'ultracode' ? `ultracode ${label(setting)}` : label(setting))));
	const later = $derived(tuning.some(pending));
	const menu = (setting: Setting) =>
		choices[setting].map((choice) => ({
			label: choice.label,
			hint: choice.detail,
			checked: choice.value === value(setting),
			onselect: () => onrun(setting, choice.value)
		}));
	const sections = $derived(
		tuning
			.filter((setting) => choices[setting].length || (setting === 'model' && ready))
			.map((setting) => ({ heading: pending(setting) ? `${names[setting]} · next turn` : names[setting], items: menu(setting), empty: 'No models available' }))
	);
</script>

<div class="chips" class:marked={mark} class:phone>
	{#if mark && !phone}
		<Tooltip text={agentNames[agent]} delayDuration={0}>
			{#snippet children(props)}
				<span {...props} class="agent" role="img" aria-label={agentNames[agent]}><Logo /></span>
			{/snippet}
		</Tooltip>
	{/if}
	{#if parts.length}
		<Menu side="top" items={sections} loading={ready || choices.model.length ? undefined : 'Loading models…'}>
			{#snippet trigger(props)}
				<Chip
					{...props}
					variant="ghost"
					pending={later}
					aria-label="Model: {parts.join(' ')}{later ? ', next turn' : ''}"
					title={later ? 'Takes effect on the next turn' : undefined}
				>
					<span class="label">{parts[0]}{#if parts.length > 1}<span class="detail">{parts.slice(1).map((part) => ` ${part}`).join('')}</span>{/if}</span>
					<span class="caret"><CaretDown aria-hidden="true" /></span>
				</Chip>
			{/snippet}
		</Menu>
	{/if}
	{#if settings.mode}
		<Menu side="top" items={menu('mode')} loading={choices.mode.length ? undefined : 'Loading modes…'}>
			{#snippet trigger(props)}
				<Chip
					{...props}
					variant="ghost"
					pending={pending('mode')}
					aria-label="Mode: {label('mode')}{pending('mode') ? ', next turn' : ''}"
					title={pending('mode') ? 'Takes effect on the next turn' : undefined}
				>
					<span class="label">{label('mode')}{#if !phone}<span class="suffix">{' '}mode</span>{/if}</span>
					<span class="caret"><CaretDown aria-hidden="true" /></span>
				</Chip>
			{/snippet}
		</Menu>
	{/if}
	{#if children}<span class="end">{@render children()}</span>{/if}
</div>

<style>
	.chips {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		font-size: var(--font-size-sm);
	}

	.chips.marked:not(.phone) {
		padding-left: var(--space-2);
	}

	.chips :global(.chip) {
		color: var(--color-fg);
	}

	.chips:not(.phone) :global(.chip) {
		flex: 0 1 auto;
	}

	.detail {
		color: var(--muted-fg);
	}

	.end {
		display: flex;
		flex: none;
		align-items: center;
		margin-left: var(--space-1);
	}

	.agent {
		display: inline-flex;
		flex: none;
		align-items: center;
		/* Pulls the next ghost Chip's padding in, so the glyph reads as its leading icon. */
		margin-right: calc(-1 * var(--space-1));
		color: var(--muted-fg);
	}

	.label {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.label::first-letter {
		text-transform: uppercase;
	}

	.caret {
		display: inline-flex;
	}

	.caret :global(svg) {
		width: 0.85em;
		height: 0.85em;
	}

	@container (width < 24rem) {
		.suffix {
			display: none;
		}
	}

	.phone {
		--target-min: var(--control-min);
		flex: 1;
		gap: var(--space-2);
		padding: 0 var(--space-1);
	}

	.phone .end {
		margin: 0 0 0 auto;
	}
</style>
