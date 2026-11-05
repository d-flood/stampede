<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import { agentNames, type Agent } from '#lib/config.svelte.ts';
	import { chosen, type Choice, type Setting, type Settings } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Menu from '#lib/ui/Menu.svelte';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import type { Snippet } from 'svelte';
	import CaretDown from '~icons/ph/caret-down';

	type Props = {
		agent: Agent;
		settings: Settings;
		choices: Record<Setting, Choice[]>;
		children?: Snippet;
		onrun(setting: Setting, value: string): void;
	};

	let { agent, settings, choices, children, onrun }: Props = $props();

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
		tuning.filter((setting) => choices[setting].length).map((setting) => ({ heading: pending(setting) ? `${names[setting]} · next turn` : names[setting], items: menu(setting) }))
	);
</script>

<div class="chips" class:phone>
	{#if !phone}
		<Tooltip text={agentNames[agent]} delayDuration={0}>
			{#snippet children(props)}
				<span {...props} class="agent" role="img" aria-label={agentNames[agent]}><Logo /></span>
			{/snippet}
		</Tooltip>
	{/if}
	{#if parts.length}
		<Menu items={sections}>
			{#snippet trigger(props)}
				<button
					{...props}
					class="chip"
					class:pending={later}
					aria-label="Model: {parts.join(' ')}{later ? ', next turn' : ''}"
					title={later ? 'Takes effect on the next turn' : undefined}
				>
					<span class="label"><span class="model">{parts[0]}</span>{parts.slice(1).map((part) => ` ${part}`).join('')}</span>
					{#if !phone}<CaretDown aria-hidden="true" />{/if}
				</button>
			{/snippet}
		</Menu>
	{/if}
	{#if settings.mode}
		<Menu items={menu('mode')}>
			{#snippet trigger(props)}
				<button
					{...props}
					class="chip"
					class:pending={pending('mode')}
					aria-label="Mode: {label('mode')}{pending('mode') ? ', next turn' : ''}"
					title={pending('mode') ? 'Takes effect on the next turn' : undefined}
				>
					<span class="label">{label('mode')}</span>
					{#if !phone}<CaretDown aria-hidden="true" />{/if}
				</button>
			{/snippet}
		</Menu>
	{/if}
	<span class="end">{@render children?.()}</span>
</div>

<style>
	.chips {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		padding-left: var(--space-2);
		font-size: var(--font-size-sm);
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
		margin-right: var(--space-1);
		color: var(--muted-fg);
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: 0.3em;
		min-width: 0;
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid transparent;
		border-radius: var(--radius-md);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		white-space: nowrap;
		cursor: pointer;
	}

	.label {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.model {
		color: var(--color-fg);
	}

	.chip :global(svg) {
		flex: none;
		align-self: center;
		width: 0.85em;
		height: 0.85em;
	}

	.chip:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.chip.pending {
		border-style: dashed;
		border-color: var(--accent);
	}

	.phone {
		padding-left: 0;
	}
</style>
