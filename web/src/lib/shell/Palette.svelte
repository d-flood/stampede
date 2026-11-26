<script lang="ts" module>
	import type { Component } from 'svelte';
	export type Scope = 'all' | 'files' | 'sessions' | 'tabs';
	export type Action = { label: string; run(): void; group?: string; icon?: Component; hint?: string };
</script>

<script lang="ts">
	import { ago, logos } from '#lib/chat/sessions.ts';
	import { rigs, type Rig } from '#lib/config.svelte.ts';
	import type { Chat, Project, Session } from '#lib/device.svelte.ts';
	import { listing } from '#lib/files.svelte.ts';
	import { split } from '#lib/git.ts';
	import ChatItem from '#lib/shell/ChatItem.svelte';
	import FileIcon from '#lib/ui/FileIcon.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import type { Status } from '#lib/ui/TabMarks.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { Command } from 'bits-ui';
	import fuzzysort, { type Key } from 'fuzzysort';

	type Props = {
		scope: Scope;
		projects: Project[];
		labels: Map<string, string>;
		focused?: Project;
		actions?: Action[];
		/** Open Chats across every open Project. */
		chats?: Chat[];
		status?(chat: Chat): Status;
		/** Past sessions read so far, by Project. */
		sessions?: Record<string, Session[]>;
		loadingSessions?: boolean;
		onproject(projectId: string): void;
		onrig(rig: Rig): void;
		onfile(path: string): void;
		onchat?(chat: Chat): void;
		onsession(projectId: string, session: Session): void;
	};
	type Entry = { value: string; label: string; parent?: string; icon?: Component; hint?: string; onselect(): void };

	let { scope, projects, labels, focused, actions = [], chats = [], status, sessions = {}, loadingSessions = false, onproject, onrig, onfile, onchat, onsession }: Props = $props();

	let query = $state('');

	const all = $derived(scope === 'all');
	const open = $derived(all ? ranked(chats.toSorted((a, b) => b.created - a.created), (chat) => chat.title ?? chat.rig, 5) : []);
	const past = $derived.by(() => {
		if (!all) return [];
		const listed = projects.flatMap((project) => (sessions[project.id] ?? []).map((session) => ({ session, projectId: project.id })));
		return ranked(listed.toSorted((a, b) => b.session.updatedAt - a.session.updatedAt), ({ session }) => session.title, 3);
	});
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
			entries: find(scope === 'sessions' && focused ? sessions[focused.id] ?? [] : [], (session) => session.title).map((session) => ({
				value: `session:${session.sessionId}`,
				label: session.title,
				icon: logos[session.agent],
				hint: ago(session.updatedAt),
				onselect: () => focused && onsession(focused.id, session)
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

	/** Keeps the newest few until a query searches them all. */
	function ranked<T>(items: T[], key: (item: T) => string, newest: number): T[] {
		return query ? find(items, key).slice(0, 50) : items.slice(0, newest);
	}
</script>

{#snippet heading(text: string)}
	<Command.GroupHeading>
		{#snippet child({ props })}
			<ListLabel {...props}>{text}</ListLabel>
		{/snippet}
	</Command.GroupHeading>
{/snippet}

<Command.Root label={{ all: 'Command Palette', files: 'Open File', sessions: 'Past Sessions', tabs: 'New Tab' }[scope]} shouldFilter={false} loop>
	<Command.Input
		value={query}
		placeholder={{ all: 'Find a Chat, Project, Rig or file', files: 'Find a file', sessions: 'Find a session by title', tabs: 'Choose what to open' }[scope]}
		autocomplete="off"
		spellcheck={false}
	>
		{#snippet child({ props })}
			<TextField {...props} bind:value={query} {@attach (element) => element.focus()} />
		{/snippet}
	</Command.Input>
	{#if reading}
		<div class="loading"><Loading label="Reading sessions…" /></div>
	{/if}
	<Command.List>
		{#snippet child({ props })}
			<div {...props} class="list">
				<Command.Viewport>
					{#if open.length}
						<Command.Group>
							{@render heading('Open Chats')}
							<Command.GroupItems>
								{#each open as chat (chat.id)}
									<ChatItem value="chat:{chat.id}" agent={chat.agent} title={chat.title ?? chat.rig} time={chat.created / 1e6} project={labels.get(chat.projectId)} status={status?.(chat)} fresh={!chat.title} onselect={() => onchat?.(chat)} />
								{/each}
							</Command.GroupItems>
						</Command.Group>
					{/if}
					{#if past.length}
						<Command.Group>
							{@render heading('Recent sessions')}
							<Command.GroupItems>
								{#each past as { session, projectId } (`${projectId}:${session.agent}:${session.sessionId}`)}
									<ChatItem value="session:{projectId}:{session.agent}:{session.sessionId}" agent={session.agent} title={session.title} time={session.updatedAt} project={labels.get(projectId)} onselect={() => onsession(projectId, session)} />
								{/each}
							</Command.GroupItems>
						</Command.Group>
					{/if}
					{#each groups as { heading: text, entries } (text)}
						{#if entries.length}
							<Command.Group>
								{@render heading(text)}
								<Command.GroupItems>
									{#each entries as { value, label, parent, icon: Icon, hint, onselect } (value)}
										<Command.Item {value} onSelect={onselect}>
											{#snippet child({ props })}
												<ListItem {...props}>
													{#if Icon}<Icon />{/if}
													{#if parent === undefined}
														<span class="label">{label}</span>
														{#if hint}<span class="hint" aria-hidden="true">{hint}</span>{/if}
													{:else}
														<FileIcon path={label} />
														<span class="path"><span class="parent">{parent}</span><span class="name">{label}</span></span>
													{/if}
												</ListItem>
											{/snippet}
										</Command.Item>
									{/each}
								</Command.GroupItems>
							</Command.Group>
						{/if}
					{/each}
					{#if !reading && !open.length && !past.length && groups.every(({ entries }) => !entries.length)}
						<ListLabel element="p">{query ? 'No matches' : scope === 'sessions' ? 'No past sessions' : 'Type to find a Chat or file'}</ListLabel>
					{/if}
				</Command.Viewport>
			</div>
		{/snippet}
	</Command.List>
</Command.Root>

<style>
	.list {
		max-height: min(24rem, 50dvh);
		margin-top: var(--space-2);
		overflow-y: auto;
	}

	.loading {
		margin-top: var(--space-2);
	}


	.label {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.hint {
		flex: none;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		white-space: nowrap;
	}

	.path {
		display: flex;
		min-width: 0;
		overflow: hidden;
		white-space: nowrap;
	}

	.parent {
		overflow: hidden;
		color: var(--muted-fg);
		text-overflow: ellipsis;
	}

	.name {
		flex: none;
	}

</style>
