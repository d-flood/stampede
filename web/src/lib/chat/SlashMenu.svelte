<script lang="ts">
	import { agentNames, templates, type Agent } from '#lib/config.svelte.ts';
	import { chosen, type Builtin, type Capabilities, type Choice, type Setting, type Settings } from '#lib/device.svelte.ts';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Suggestions from '#lib/ui/Suggestions.svelte';

	type Props = {
		text: string;
		agent: Agent;
		settings: Settings;
		capabilities: Capabilities;
		choices: Record<Setting, Choice[]>;
		working: boolean;
		replace(text: string): void;
		onrun(command: Builtin, value?: string | number): void;
	};

	type Row = {
		label: string;
		detail?: string;
		choices?: Choice[];
		current?: string;
		disabled?: boolean;
		choose(): void;
	};

	let { text, agent, settings, capabilities, choices, working, replace: replaceText, onrun }: Props = $props();
	let browsing = $state<string>();
	const query = $derived(browsing === text ? '/' : text);
	const typed = $derived(query.match(/^\/(\S*)$/)?.[1]);

	function replace(next: string) {
		browsing = undefined;
		replaceText(next);
	}

	const builtins = ['effort', 'model', 'speed', 'ultracode', 'mode', 'compact', 'rewind'];

	function run(command: Builtin, value?: string | number) {
		onrun(command, value);
		replace('');
	}

	function now(command: 'compact' | 'rewind', detail: string): Row {
		const later = working ? 'after this turn' : detail;
		return { label: `/${command}`, detail: later, disabled: working, choose: () => run(command) };
	}

	const groups = $derived.by((): { heading: string; rows: Row[] }[] => {
		const picking = query.match(/^\/(model|effort|speed|ultracode|mode) (\S*)$/);
		if (picking) {
			const setting = picking[1] as Setting;
			const current = chosen(setting, settings, choices);
			const matching = choices[setting].filter((choice) => choice.label.startsWith(picking[2]));
			const rows = matching.map((choice) => ({
				label: choice.label,
				detail: choice.value !== current ? (choice.detail ?? '') : settings.pending?.includes(setting) ? 'next turn' : 'current',
				choose: () => run(setting, choice.value)
			}));
			return [{ heading: `/${setting}`, rows }];
		}
		if (typed === undefined) return [];
		const model = choices.model.find((choice) => choice.value === settings.model);
		const stampede: Row[] = [
			{ label: '/effort', choices: choices.effort, current: settings.effort, choose: () => replace('/effort ') },
			{ label: '/model', detail: model?.label ?? settings.model ?? '', choose: () => replace('/model ') },
			...(['speed', 'ultracode'] as const).filter((setting) => choices[setting].length).map((setting) => ({
				label: `/${setting}`,
				choices: choices[setting],
				current: chosen(setting, settings, choices),
				choose: () => replace(`/${setting} `)
			})),
			{ label: '/mode', choices: choices.mode, current: settings.mode, choose: () => replace('/mode ') },
			now('compact', 'free up context'),
			now('rewind', capabilities.rewindsFiles ? 'conversation and tracked files' : 'conversation only; files unchanged'),
			...templates().map(({ name, description }) => ({ label: `/${name}`, detail: description, choose: () => replace(`/${name} `) }))
		];
		const taken = [...builtins, ...templates().map((template) => template.name)];
		const commands = capabilities.commands.filter((command) => !taken.includes(command.name));
		const own = commands.map((command) => ({
			label: `/${command.name}`,
			detail: command.hint || command.description,
			choose: () => replace(`/${command.name} `)
		}));
		const matching = (rows: Row[]) => {
			const prefixed = rows.filter((row) => row.label.startsWith(`/${typed}`));
			return [...prefixed, ...rows.filter((row) => !prefixed.includes(row) && row.label.includes(typed))];
		};
		return [
			{ heading: 'Stampede', rows: matching(stampede) },
			{ heading: `From ${agentNames[agent]}`, rows: matching(own) }
		].filter((group) => group.rows.length);
	});

	const rows: Row[] = $derived(groups.flatMap((group) => group.rows));
	let highlighted = $state<string>();
	const highlight = $derived.by(() => {
		const hovered = rows.findIndex((row) => row.label === highlighted);
		return hovered >= 0 ? hovered : Math.max(0, rows.findIndex((row) => row.label.startsWith(`/${typed}`)));
	});
	let dismissed = $state<string>();
	const open = $derived(rows.length > 0 && dismissed !== text);

	export function isOpen() {
		return open;
	}

	export function toggle() {
		if (open) dismissed = text;
		else {
			browsing = text;
			dismissed = undefined;
		}
	}

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
	<Suggestions label="Commands">
		{#each groups as group, index (group.heading)}
			{#if index}<div class="separator"></div>{/if}
			<ListLabel>{group.heading}</ListLabel>
			{#each group.rows as row}
				<ListItem
					element="button"
					highlighted={rows.indexOf(row) === highlight}
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
				</ListItem>
			{/each}
		{/each}
	</Suggestions>
{/if}

<style>

	.separator {
		margin: var(--space-1) 0;
		border-top: var(--border-width) solid var(--border);
	}

	.label {
		flex: none;
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
	}

	.detail {
		min-width: 0;
		margin-left: auto;
		padding-left: var(--space-2);
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
