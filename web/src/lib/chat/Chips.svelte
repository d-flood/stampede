<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import { agentNames, type Agent } from '#lib/config.svelte.ts';
	import { chosen, type Choice, type Setting, type Settings } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Menu from '#lib/ui/Menu.svelte';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import type { Snippet } from 'svelte';

	type Props = {
		agent: Agent;
		settings: Settings;
		choices: Record<Setting, Choice[]>;
		docked?: boolean;
		children?: Snippet;
		onrun(setting: Setting, value: string): void;
	};

	let { agent, settings, choices, docked = false, children, onrun }: Props = $props();

	const names: Record<Setting, string> = { model: 'Model', effort: 'Effort', speed: 'Speed', ultracode: 'Ultracode', mode: 'Mode' };
	const phone = $derived(form() === 'phone');
	const Logo = $derived(logos[agent]);
	const optional = (setting: Setting) => setting === 'speed' || setting === 'ultracode';
	const shown = $derived((['model', 'effort', 'speed', 'ultracode', 'mode'] as const).filter((setting) => (optional(setting) ? choices[setting].length : settings[setting])));
</script>

<div class="chips" class:phone class:docked>
	{#if docked}
		<Tooltip text={agentNames[agent]} delayDuration={0}>
			{#snippet children(props)}
				<span {...props} class="agent" role="img" aria-label={agentNames[agent]}><Logo /></span>
			{/snippet}
		</Tooltip>
	{:else if !phone}
		<span class="agent"><Logo /> {agentNames[agent]}</span>
	{/if}
	{#each shown as setting, index (setting)}
		{@const value = chosen(setting, settings, choices)}
		{@const pending = !!settings.pending?.includes(setting)}
		{@const quiet = optional(setting) && value === choices[setting][0].value}
		{@const label = choices[setting].find((choice) => choice.value === value)?.label ?? value}
		{#if index}<span class="dot" class:quiet aria-hidden="true">·</span>{/if}
		<Menu
			items={choices[setting].map((choice) => ({
				label: choice.label,
				hint: choice.detail,
				checked: choice.value === value,
				onselect: () => onrun(setting, choice.value)
			}))}
		>
			{#snippet trigger(props)}
				<button
					{...props}
					class="chip"
					class:pending
					class:quiet
					aria-label="{names[setting]}: {label}{pending ? ', next turn' : ''}"
					title={pending ? 'Takes effect on the next turn' : undefined}
				>
					{setting === 'ultracode' ? 'ultracode ' : ''}{label}{setting === 'mode' && !phone ? ' mode' : ''}
				</button>
			{/snippet}
		</Menu>
	{/each}
	<span class="end">{@render children?.()}</span>
</div>

<style>
	.chips {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-1);
		padding: var(--space-2) var(--space-6);
		border-bottom: var(--border-width) solid var(--border);
		font-size: var(--font-size-sm);
	}

	.end {
		display: flex;
		flex: none;
		align-items: center;
		margin-left: auto;
	}

	.agent {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		margin-right: var(--space-2);
		color: var(--muted-fg);
	}

	.dot {
		color: var(--muted-fg);
	}

	.chip {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid transparent;
		border-radius: var(--radius-md);
		background: none;
		color: var(--color-fg);
		font: inherit;
		cursor: pointer;
	}

	.chip:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.chip.pending {
		border-style: dashed;
		border-color: var(--status-working);
	}

	.phone {
		padding: 0 var(--space-3);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.phone .quiet {
		display: none;
	}

	.docked {
		flex: 1;
		min-width: 0;
		padding: 0 0 0 var(--space-2);
		border: none;
		container-type: inline-size;
	}

	@container (max-width: 30rem) {
		.quiet {
			display: none;
		}
	}

	.docked .agent {
		flex: none;
		margin-right: var(--space-1);
	}

	.docked .chip {
		color: var(--muted-fg);
	}

	.docked .end {
		margin-left: var(--space-1);
	}
</style>
