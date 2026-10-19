<script lang="ts" module>
	import type { Component } from 'svelte';
	export type Scope = 'all' | 'files' | 'sessions' | 'tabs';
	export type Action = { label: string; run(): void; group?: string; icon?: Component; hint?: string };
</script>

<script lang="ts">
	import { ago, logos } from '#lib/chat/sessions.ts';
	import { rigs, type Rig } from '#lib/config.svelte.ts';
	import type { Project, Session } from '#lib/device.svelte.ts';
	import { listing } from '#lib/files.svelte.ts';
	import { split } from '#lib/git.ts';
	import Loading from '#lib/ui/Loading.svelte';
	import { Command } from 'bits-ui';
	import fuzzysort, { type Key } from 'fuzzysort';

	type Props = {
		scope: Scope;
		projects: Project[];
		labels: Map<string, string>;
		focused?: Project;
		actions?: Action[];
		sessions?: Session[];
		loadingSessions?: boolean;
		onproject(projectId: string): void;
		onrig(rig: Rig): void;
		onfile(path: string): void;
		onsession(session: Session): void;
	};
	type Entry = { value: string; label: string; parent?: string; icon?: Component; hint?: string; onselect(): void };

	let { scope, projects, labels, focused, actions = [], sessions = [], loadingSessions = false, onproject, onrig, onfile, onsession }: Props = $props();

	let query = $state('');

	const all = $derived(scope === 'all');
	const reading = $derived(loadingSessions && (scope === 'tabs' || scope === 'sessions'));
	const files = $derived.by(() => {
		const snapshot = focused && listing(focused.id).snapshot;
		if (!snapshot || scope === 'tabs' || (all && !query)) return [];
		return fuzzysort.go(query, snapshot, { limit: 50, threshold: 0 }).map(({ target: path }) => {
			const { name, dir } = split(path);
			return { value: `file:${path}`, parent: dir, label: name, onselect: () => onfile(path) };
		});
	});
	const tabGroups = $derived.by(() => {
		const groups = new Map<string, Entry[]>();
		for (const [index, action] of find(actions, (action) => action.label).entries()) {
			const heading = action.group ?? 'Tabs';
			if (!groups.has(heading)) groups.set(heading, []);
			groups.get(heading)!.push({ value: `tab:${index}`, label: action.label, icon: action.icon, hint: action.hint, onselect: action.run });
		}
		return Array.from(groups, ([heading, entries]) => ({ heading, entries }));
	});
	const groups: { heading: string; entries: Entry[] }[] = $derived(scope === 'tabs' ? tabGroups : [
		{
			heading: 'Projects',
			entries: find(all ? projects : [], (project) => labels.get(project.id)).map((project) => ({
				value: `project:${project.id}`,
				label: labels.get(project.id) ?? project.path,
				onselect: () => onproject(project.id)
			}))
		},
		{
			heading: 'Rigs',
			entries: find(all && focused ? rigs() : [], (rig) => rig.name)
				.map((rig) => ({
					value: `rig:${rig.name}`,
					label: rig.name,
					hint: rigs().indexOf(rig) < 9 ? `Ctrl+Shift+${rigs().indexOf(rig) + 1}` : `${rigs().indexOf(rig) + 1}`,
					onselect: () => onrig(rig)
				}))
		},
		{ heading: 'Files', entries: files },
		{
			heading: 'Sessions',
			entries: find(scope === 'sessions' ? sessions : [], (session) => session.firstPrompt).map((session) => ({
				value: `session:${session.sessionId}`,
				label: session.firstPrompt,
				icon: logos[session.agent],
				hint: ago(session.updatedAt),
				onselect: () => onsession(session)
			}))
		},
		{
			heading: 'Commands',
			entries: find(all ? actions : [], (action) => action.label).map((action) => ({
				value: `command:${action.label}`,
				label: action.label,
				onselect: action.run
			}))
		}
	]);

	function find<T>(items: T[], key: Key<T>): T[] {
		return fuzzysort.go(query, items, { key, limit: 0, threshold: 0 }).map((result) => result.obj);
	}
</script>

<Command.Root label={{ all: 'Command Palette', files: 'Open File', sessions: 'Past Sessions', tabs: 'New Tab' }[scope]} shouldFilter={false} loop>
	<Command.Input
		placeholder={{ all: 'Switch Project, launch a Rig, or open a file', files: 'Find a file', sessions: 'Find a session by its first prompt', tabs: 'Choose what to open' }[scope]}
		autocomplete="off"
		spellcheck={false}
	>
		{#snippet child({ props })}
			<input {...props} class="query" bind:value={query} {@attach (element) => element.focus()} />
		{/snippet}
	</Command.Input>
	{#if reading}
		<div class="loading"><Loading label="Reading sessions…" /></div>
	{/if}
	<Command.List>
		{#snippet child({ props })}
			<div {...props} class="list">
				<Command.Viewport>
					{#each groups as { heading, entries } (heading)}
						{#if entries.length}
							<Command.Group>
								<Command.GroupHeading>
									{#snippet child({ props })}
										<div {...props} class="heading">{heading}</div>
									{/snippet}
								</Command.GroupHeading>
								<Command.GroupItems>
									{#each entries as { value, label, parent, icon: Icon, hint, onselect } (value)}
										<Command.Item {value} onSelect={onselect}>
											{#snippet child({ props })}
												<div {...props} class="item">
													{#if Icon}<Icon />{/if}
													{#if parent === undefined}
														<span class="label">{label}</span>
														{#if hint}<span class="hint" aria-hidden="true">{hint}</span>{/if}
													{:else}
														<span class="path"><span class="parent">{parent}</span><span class="name">{label}</span></span>
													{/if}
												</div>
											{/snippet}
										</Command.Item>
									{/each}
								</Command.GroupItems>
							</Command.Group>
						{/if}
					{/each}
					{#if !reading && groups.every(({ entries }) => !entries.length)}
						<p class="empty">{query ? 'No matches' : scope === 'sessions' ? 'No past sessions' : 'Type to find a file'}</p>
					{/if}
				</Command.Viewport>
			</div>
		{/snippet}
	</Command.List>
</Command.Root>

<style>
	.query {
		box-sizing: border-box;
		width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		outline: none;
	}

	.query:focus-visible {
		border-color: var(--status-working);
	}

	.list {
		max-height: min(24rem, 50dvh);
		margin-top: var(--space-2);
		overflow-y: auto;
	}

	.loading {
		margin-top: var(--space-2);
	}

	.heading {
		display: flex;
		align-items: center;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.item {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		cursor: pointer;
		white-space: nowrap;
	}

	.item :global(svg) {
		flex: none;
	}

	.label {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.hint {
		flex: none;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.item[data-selected] {
		background: var(--hover);
	}

	.path {
		display: flex;
		min-width: 0;
		overflow: hidden;
		font-family: var(--font-mono);
	}

	.parent {
		overflow: hidden;
		color: var(--muted-fg);
		text-overflow: ellipsis;
	}

	.name {
		flex: none;
	}

	.empty {
		margin: 0;
		padding: var(--space-2);
		color: var(--muted-fg);
	}
</style>
