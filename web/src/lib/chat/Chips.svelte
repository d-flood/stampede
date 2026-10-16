<script lang="ts">
	import { agentNames, type Agent } from '#lib/config.svelte.ts';
	import type { Choice, Setting, Settings } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Menu from '#lib/ui/Menu.svelte';
	import type { Snippet } from 'svelte';

	type Props = {
		agent: Agent;
		settings: Settings;
		choices: Record<Setting, Choice[]>;
		children?: Snippet;
		onrun(setting: Setting, value: string): void;
	};

	let { agent, settings, choices, children, onrun }: Props = $props();

	const names: Record<Setting, string> = { model: 'Model', effort: 'Effort', mode: 'Mode' };
	const phone = $derived(form() === 'phone');
</script>

<div class="chips" class:phone>
	{#if !phone}
		<span class="agent">{agentNames[agent]}</span>
	{/if}
	{#each ['model', 'effort', 'mode'] as const as setting (setting)}
		{@const value = settings[setting]}
		{@const pending = !!settings.pending?.includes(setting)}
		{#if value}
			{@const label = choices[setting].find((choice) => choice.value === value)?.label ?? value}
			<Menu
				items={choices[setting].map((choice) => ({
					label: choice.label,
					checked: choice.value === value,
					onselect: () => onrun(setting, choice.value)
				}))}
			>
				{#snippet trigger(props)}
					<button
						{...props}
						class="chip"
						class:pending
						aria-label="{names[setting]}: {label}{pending ? ', next turn' : ''}"
						title={pending ? 'Takes effect on the next turn' : undefined}
					>
						{label}{setting === 'mode' && !phone ? ' mode' : ''}
					</button>
				{/snippet}
			</Menu>
		{/if}
	{/each}
	<span class="end">{@render children?.()}</span>
</div>

<style>
	.chips {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-6);
		border-bottom: var(--border-width) solid var(--border);
		font-size: var(--font-size-sm);
	}

	.end {
		display: flex;
		align-items: center;
		margin-left: auto;
	}

	.agent {
		margin-right: var(--space-1);
		color: var(--muted-fg);
	}

	.chip {
		padding: var(--space-1) var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: 999px;
		background: var(--surface-2);
		color: var(--color-fg);
		font: inherit;
		cursor: pointer;
	}

	.chip:hover {
		background: var(--hover);
	}

	.chip.pending {
		border-style: dashed;
		border-color: var(--status-working);
	}

	.phone {
		padding: var(--space-2) var(--space-4);
		font-size: var(--font-size-md);
	}
</style>
