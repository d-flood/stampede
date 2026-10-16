<script lang="ts" module>
	export type Scope = 'all' | 'files';
	export type Action = { label: string; run(): void };
</script>

<script lang="ts">
	import { rigs, type Rig } from '#lib/config.svelte.ts';
	import type { Project } from '#lib/device.ts';
	import { listing } from '#lib/files.svelte.ts';
	import { Command } from 'bits-ui';
	import fuzzysort, { type Key } from 'fuzzysort';

	type Props = {
		scope: Scope;
		projects: Project[];
		labels: Map<string, string>;
		focused?: Project;
		actions?: Action[];
		onproject(projectId: string): void;
		onrig(rig: Rig): void;
		onfile(path: string): void;
	};
	type Entry = { value: string; label: string; parent?: string; onselect(): void };

	let { scope, projects, labels, focused, actions = [], onproject, onrig, onfile }: Props = $props();

	let query = $state('');

	const all = $derived(scope === 'all');
	const files = $derived.by(() => {
		const snapshot = focused && listing(focused.id).snapshot;
		if (!snapshot || (all && !query)) return [];
		return fuzzysort.go(query, snapshot, { limit: 50, threshold: 0 }).map(({ target: path }) => {
			const cut = path.lastIndexOf('/') + 1;
			return { value: `file:${path}`, parent: path.slice(0, cut), label: path.slice(cut), onselect: () => onfile(path) };
		});
	});
	const groups: { heading: string; entries: Entry[] }[] = $derived([
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
			entries: find(all && focused ? rigs() : [], (rig) => rig.name).map((rig) => ({
				value: `rig:${rig.name}`,
				label: rig.name,
				onselect: () => onrig(rig)
			}))
		},
		{ heading: 'Files', entries: files },
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

<Command.Root label={all ? 'Command Palette' : 'Open File'} shouldFilter={false} loop>
	<Command.Input
		placeholder={all ? 'Switch Project, launch a Rig, or open a file' : 'Find a file'}
		autocomplete="off"
		spellcheck={false}
	>
		{#snippet child({ props })}
			<input {...props} class="query" bind:value={query} />
		{/snippet}
	</Command.Input>
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
									{#each entries as { value, label, parent, onselect } (value)}
										<Command.Item {value} onSelect={onselect}>
											{#snippet child({ props })}
												<div {...props} class="item">
													{#if parent === undefined}
														{label}
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
					{#if groups.every(({ entries }) => !entries.length)}
						<p class="empty">{query ? 'No matches' : 'Type to find a file'}</p>
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
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		cursor: pointer;
		white-space: nowrap;
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
