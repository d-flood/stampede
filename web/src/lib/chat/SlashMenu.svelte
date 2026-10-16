<script lang="ts">
	import { agentNames, type Agent } from '#lib/config.svelte.ts';
	import type { Builtin, Capabilities, Choice, Setting, Settings } from '#lib/device.svelte.ts';

	type Props = {
		text: string;
		agent: Agent;
		settings: Settings;
		capabilities: Capabilities;
		choices: Record<Setting, Choice[]>;
		working: boolean;
		replace(text: string): void;
		onrun(command: Builtin, value?: string): void;
	};

	type Row = {
		label: string;
		detail?: string;
		choices?: Choice[];
		current?: string;
		disabled?: boolean;
		choose(): void;
	};

	let { text, agent, settings, capabilities, choices, working, replace, onrun }: Props = $props();

	const builtins = ['effort', 'model', 'mode', 'compact', 'clear'];
	const templates: Row[] = [];

	function run(command: Builtin, value?: string) {
		onrun(command, value);
		replace('');
	}

	function now(command: 'compact' | 'clear', detail: string): Row {
		const later = working ? 'after this turn' : detail;
		return { label: `/${command}`, detail: later, disabled: working, choose: () => run(command) };
	}

	const groups = $derived.by((): { heading: string; rows: Row[] }[] => {
		const picking = text.match(/^\/(model|effort|mode) (\S*)$/);
		if (picking) {
			const setting = picking[1] as Setting;
			const current = settings[setting];
			const matching = choices[setting].filter((choice) => choice.label.startsWith(picking[2]));
			const rows = matching.map((choice) => ({
				label: choice.label,
				detail: choice.value !== current ? '' : settings.pending?.includes(setting) ? 'next turn' : 'current',
				choose: () => run(setting, choice.value)
			}));
			return [{ heading: `/${setting}`, rows }];
		}
		const typed = text.match(/^\/(\S*)$/)?.[1];
		if (typed === undefined) return [];
		const model = choices.model.find((choice) => choice.value === settings.model);
		const oikonomos: Row[] = [
			{ label: '/effort', choices: choices.effort, current: settings.effort, choose: () => replace('/effort ') },
			{ label: '/model', detail: model?.label ?? settings.model ?? '', choose: () => replace('/model ') },
			{ label: '/mode', choices: choices.mode, current: settings.mode, choose: () => replace('/mode ') },
			now('compact', 'free up context'),
			now('clear', 'new session')
		];
		const commands = capabilities.commands.filter((command) => !builtins.includes(command.name));
		const own = commands.map((command) => ({
			label: `/${command.name}`,
			detail: command.hint || command.description,
			choose: () => replace(`/${command.name} `)
		}));
		const matches = (row: Row) => row.label.startsWith(`/${typed}`);
		return [
			{ heading: 'Oikonomos', rows: oikonomos.filter(matches) },
			{ heading: 'Templates', rows: templates.filter(matches) },
			{ heading: `From ${agentNames[agent]}`, rows: own.filter(matches) }
		].filter((group) => group.rows.length);
	});

	const rows: Row[] = $derived(groups.flatMap((group) => group.rows));
	let highlighted = $state<string>();
	const highlight = $derived(Math.max(0, rows.findIndex((row) => row.label === highlighted)));
	let dismissed = $state<string>();
	const open = $derived(rows.length > 0 && dismissed !== text);

	export function key(event: KeyboardEvent) {
		if (!open) return;
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			highlighted = rows[(highlight + (event.key === 'ArrowDown' ? 1 : rows.length - 1)) % rows.length].label;
		} else if ((event.key === 'Enter' || event.key === 'Tab') && !event.shiftKey) {
			if (!rows[highlight].disabled) rows[highlight].choose();
		} else if (event.key === 'Escape') {
			dismissed = text;
		} else {
			return;
		}
		event.preventDefault();
	}
</script>

{#if open}
	<section aria-label="Commands" class="menu">
		{#each groups as group (group.heading)}
			<div class="heading">{group.heading}</div>
			{#each group.rows as row}
				<button
					class="row"
					class:highlighted={rows.indexOf(row) === highlight}
					disabled={row.disabled}
					onpointerenter={() => (highlighted = row.label)}
					onmousedown={(event) => event.preventDefault()}
					onclick={row.choose}
				>
					<span class="label">{row.label}</span>
					<span class="detail">
						{#if row.choices}
							{#each row.choices as choice, at (choice.value)}
								{#if at}·{/if}
								{#if choice.value === row.current}<strong>{choice.label}</strong>{:else}{choice.label}{/if}
							{/each}
						{:else}
							{row.detail}
						{/if}
					</span>
				</button>
			{/each}
		{/each}
	</section>
{/if}

<style>
	.menu {
		position: absolute;
		right: var(--space-2);
		bottom: calc(100% + var(--space-1));
		left: var(--space-2);
		z-index: 1;
		display: flex;
		flex-direction: column;
		max-height: min(24rem, 50vh);
		padding: var(--space-1) 0;
		overflow-y: auto;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
	}

	.heading {
		padding: var(--space-2) var(--space-4) var(--space-1);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.heading:not(:first-child) {
		margin-top: var(--space-1);
		border-top: var(--border-width) solid var(--border);
	}

	.row {
		display: flex;
		flex: none;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-4);
		border: 0;
		background: transparent;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.row.highlighted {
		background: var(--hover);
	}

	.row:disabled {
		color: var(--muted-fg);
		cursor: default;
	}

	.label {
		flex: none;
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
	}

	.detail {
		min-width: 0;
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.detail strong {
		color: var(--color-fg);
	}
</style>
