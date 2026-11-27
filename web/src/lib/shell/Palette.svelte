<script lang="ts" module>
	import type { Component } from 'svelte';
	export type Scope = 'all' | 'files' | 'sessions' | 'tabs';
	export type Action = { label: string; run(): void; group?: string; icon?: Component; hint?: string; shortcut?: string };
</script>

<script lang="ts">
	import { ago, logos } from '#lib/chat/sessions.ts';
	import { rigs, runsOn, type Rig } from '#lib/config.svelte.ts';
	import type { Chat, Project, Session } from '#lib/device.svelte.ts';
	import { listing } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { split } from '#lib/git.ts';
	import ChatItem from '#lib/shell/ChatItem.svelte';
	import FileIcon from '#lib/ui/FileIcon.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import Shortcut from '#lib/ui/Shortcut.svelte';
	import type { Status } from '#lib/ui/TabMarks.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { Command } from 'bits-ui';
	import fuzzysort, { type Key } from 'fuzzysort';
	import { SvelteSet } from 'svelte/reactivity';
	import CommandIcon from '~icons/ph/command';
	import FolderSimple from '~icons/ph/folder-simple';

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
	type Entry = { value: string; label: string; parent?: string; icon?: Component; projectIcon?: string; hint?: string; shortcut?: string; onselect(): void };

	let { scope, projects, labels, focused, actions = [], chats = [], status, sessions = {}, loadingSessions = false, onproject, onrig, onfile, onchat, onsession }: Props = $props();

	let query = $state('');
	let more = $state(false);
	const brokenIcons = new SvelteSet<string>();

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
			groups.get(heading)!.push({ value: `tab:${index}`, label: action.label, icon: action.icon ?? CommandIcon, hint: action.hint, shortcut: action.shortcut, onselect: action.run });
		}
		return Array.from(groups, ([heading, entries]) => ({ heading, entries }));
	});
	const groups: { heading: string; entries: Entry[] }[] = $derived(scope === 'tabs' ? tabGroups : [
		{
			heading: 'Projects',
			entries: find(all ? projects : [], (project) => labels.get(project.id)).map((project) => ({
				value: `project:${project.id}`,
				label: labels.get(project.id) ?? project.path,
				icon: FolderSimple,
				projectIcon: project.icon && !brokenIcons.has(project.icon) ? project.icon : undefined,
				onselect: () => onproject(project.id)
			}))
		},
		{
			heading: 'Rigs',
			entries: find(all && focused ? rigs().filter((rig) => runsOn(focused.host, rig.agent)) : [], (rig) => rig.name)
				.map((rig) => {
					const index = rigs().indexOf(rig);
					return {
						value: `rig:${rig.name}`,
						label: rig.name,
						icon: logos[rig.agent],
						shortcut: index < 9 ? `Ctrl+Shift+${index + 1}` : undefined,
						hint: index < 9 ? undefined : `${index + 1}`,
						onselect: () => onrig(rig)
					};
				})
		},
		{ heading: 'Files', entries: files },
		{
			heading: 'Sessions',
			entries: find(scope === 'sessions' && focused ? sessions[focused.id] ?? [] : [], (session) => session.title).map((session) => ({
				value: `session:${session.key.agent}:${session.key.store}:${session.key.sessionId}`,
				label: session.title,
				icon: logos[session.key.agent],
				hint: ago(session.updatedAt),
				onselect: () => focused && onsession(focused.id, session)
			}))
		},
		{
			heading: 'Commands',
			entries: find(all ? actions : [], (action) => action.label).map((action) => ({
				value: `command:${action.label}`,
				label: action.label,
				icon: action.icon ?? CommandIcon,
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

	function trackScroll(node: HTMLElement) {
		const update = () => {
			more = node.scrollHeight - node.scrollTop - node.clientHeight > 2;
		};
		const observer = new ResizeObserver(update);
		observer.observe(node);
		if (node.firstElementChild) observer.observe(node.firstElementChild);
		node.addEventListener('scroll', update, { passive: true });
		update();
		return () => {
			observer.disconnect();
			node.removeEventListener('scroll', update);
		};
	}
</script>

{#snippet heading(text: string)}
	<Command.GroupHeading>
		{#snippet child({ props })}
			<ListLabel {...props}>{text}</ListLabel>
		{/snippet}
	</Command.GroupHeading>
{/snippet}

<div class="palette">
	<Command.Root label={{ all: 'Command Palette', files: 'Open File', sessions: 'Past Sessions', tabs: 'New Tab' }[scope]} shouldFilter={false} loop>
		<Command.Input
			value={query}
			placeholder={{ all: 'Find a Chat, Project, Rig or file', files: 'Find a file', sessions: 'Find a session by title', tabs: 'Choose what to open' }[scope]}
			autocomplete="off"
			spellcheck={false}
		>
			{#snippet child({ props })}
				<TextField {...props} bind:value={query} search bare data-autofocus />
			{/snippet}
		</Command.Input>
		{#if reading}
			<div class="loading"><Loading label="Reading sessions…" /></div>
		{/if}
		<Command.List>
			{#snippet child({ props })}
				<div {...props} class="list" class:more {@attach trackScroll}>
					<Command.Viewport>
						{#if open.length}
							<Command.Group class="group">
								{@render heading('Open Chats')}
								<Command.GroupItems>
									{#each open as chat (chat.id)}
										<ChatItem value="chat:{chat.id}" agent={chat.agent} title={chat.title ?? chat.rig} time={chat.created / 1e6} project={labels.get(chat.projectId)} status={status?.(chat)} fresh={!chat.title} onselect={() => onchat?.(chat)} />
									{/each}
								</Command.GroupItems>
							</Command.Group>
						{/if}
						{#if past.length}
							<Command.Group class="group">
								{@render heading('Recent sessions')}
								<Command.GroupItems>
									{#each past as { session, projectId } (`${projectId}:${session.key.agent}:${session.key.store}:${session.key.sessionId}`)}
										<ChatItem value="session:{projectId}:{session.key.agent}:{session.key.store}:{session.key.sessionId}" agent={session.key.agent} title={session.title} time={session.updatedAt} project={labels.get(projectId)} onselect={() => onsession(projectId, session)} />
									{/each}
								</Command.GroupItems>
							</Command.Group>
						{/if}
						{#each groups as { heading: text, entries } (text)}
							{#if entries.length}
								<Command.Group class="group">
									{@render heading(text)}
									<Command.GroupItems>
										{#each entries as { value, label, parent, icon: Icon, projectIcon, hint, shortcut, onselect } (value)}
											<Command.Item {value} onSelect={onselect}>
												{#snippet child({ props })}
													<ListItem {...props}>
														{#if parent === undefined}
															<span class="slot" aria-hidden="true">
																{#if projectIcon}
																	<img class="project-icon" src="/api/project-icons/{projectIcon}" alt="" onerror={() => brokenIcons.add(projectIcon)} />
																{:else if Icon}
																	<Icon />
																{/if}
															</span>
															<span class="label" title={label}>{label}</span>
															{#if hint}<span class="hint" aria-hidden="true">{hint}</span>{/if}
															{#if shortcut && form() !== 'phone'}<span class="keycap" aria-hidden="true"><Shortcut keys={shortcut} /></span>{/if}
														{:else}
															<span class="slot" aria-hidden="true"><FileIcon path={label} /></span>
															<span class="path" title={parent ? `${parent}${label}` : label}><span class="parent">{parent}</span><span class="name">{label}</span></span>
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
</div>

<style>
	.palette {
		display: flex;
		flex-direction: column;
		min-height: 0;
		--bare-inset: calc(var(--space-2) + var(--space-2));
		--bare-gap: var(--space-2);
	}

	.palette :global([data-command-root]) {
		display: flex;
		flex-direction: column;
		min-height: 0;
	}

	.list {
		max-height: min(26rem, 55dvh);
		padding: var(--space-2);
		overflow-y: auto;
	}

	.list.more {
		mask-image: linear-gradient(to bottom, black calc(100% - var(--space-3)), transparent);
	}

	.palette :global(.group + .group) {
		margin-top: var(--space-4);
	}

	.palette :global(.group .caption) {
		min-height: var(--control-min);
		padding-bottom: var(--space-1);
		font-weight: 500;
	}

	.loading {
		padding: var(--space-2) var(--bare-inset) 0;
	}

	.slot {
		display: inline-grid;
		flex: none;
		place-items: center;
		width: 1.25em;
		height: 1.25em;
		color: var(--muted-fg);
	}

	.project-icon {
		width: 100%;
		height: 100%;
		object-fit: contain;
	}

	.label {
		flex: 1;
		min-width: 0;
		margin-inline-end: var(--space-3);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.hint {
		flex: none;
		color: var(--muted-fg);
		font-size: calc(var(--font-size-sm) - 1px);
		white-space: nowrap;
	}

	.keycap {
		display: inline-flex;
		flex: none;
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
