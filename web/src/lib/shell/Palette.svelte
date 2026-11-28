<script lang="ts" module>
	import type { Component } from 'svelte';
	export type Scope = 'all' | 'files' | 'sessions' | 'tabs';
	export type Action = { label: string; run(): void; group?: string; icon?: Component; hint?: string; shortcut?: string };
</script>

<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import { rigs, runsOn, type Rig } from '#lib/config.svelte.ts';
	import type { Chat, Coverage, FoundSession, Project, Session } from '#lib/device.svelte.ts';
	import { formatSize } from '#lib/size.ts';
	import { listing } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { split } from '#lib/git.ts';
	import ChatItem from '#lib/shell/ChatItem.svelte';
	import SessionPreview from '#lib/shell/SessionPreview.svelte';
	import { grouped, outOfDate, PANE_MIN, projectName, SessionSearch, sessionKey, type Searching } from '#lib/shell/session-search.svelte.ts';
	import FileIcon from '#lib/ui/FileIcon.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import Shortcut from '#lib/ui/Shortcut.svelte';
	import type { Status } from '#lib/ui/TabMarks.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { Command } from 'bits-ui';
	import fuzzysort, { type Key } from 'fuzzysort';
	import { tick } from 'svelte';
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
		/** Where a typed query searches what was said in Sessions. */
		search?: Searching;
		/** The focused Project's Host's index coverage, as its latest Session list reported it. */
		coverage?: Coverage;
		/** Whether a found Session can be opened from here. */
		selectable?(session: Session): boolean;
		onproject(projectId: string): void;
		onrig(rig: Rig): void;
		onfile(path: string): void;
		onchat?(chat: Chat): void;
		/** `query` is set when a typed search found the Session. */
		onsession(session: Session | FoundSession, query?: string): void;
		/** Whether the dialog should widen to fit a preview pane beside the results. */
		wide?: boolean;
	};
	type Entry = { value: string; label: string; parent?: string; icon?: Component; projectIcon?: string; hint?: string; shortcut?: string; onselect(): void };

	let { scope, projects, labels, focused, actions = [], chats = [], status, sessions = {}, loadingSessions = false, search, coverage, selectable, onproject, onrig, onfile, onchat, onsession, wide = $bindable(false) }: Props = $props();

	let query = $state('');
	let command: Command.Root | undefined;
	const host = new SessionSearch(
		() => (scope === 'all' || scope === 'sessions' ? search : undefined),
		async () => {
			await tick();
			command?.updateSelectedToIndex(0);
		}
	);
	let more = $state(false);
	const brokenIcons = new SvelteSet<string>();

	const all = $derived(scope === 'all');
	const open = $derived(all ? ranked(chats.toSorted((a, b) => b.created - a.created), (chat) => chat.title ?? chat.rig, 5) : []);
	const found = $derived(query && search && (all || scope === 'sessions') ? grouped(host.found, projects.filter((project) => project.host === focused?.host).flatMap((project) => sessions[project.id] ?? []), query, search.focus) : undefined);
	let width = $state(0);
	let highlighted = $state('');
	const covered = $derived(found ? host.coverage : coverage);
	const paned = $derived(!!found && width >= PANE_MIN && found.here.length + found.other.length > 0);
	const previewed = $derived(paned && found ? [...found.here, ...found.other].find((session) => `session:${session.project.id}:${sessionKey(session)}` === highlighted) : undefined);

	$effect(() => {
		wide = !!found;
		return () => (wide = false);
	});

	const past = $derived.by(() => {
		if (found) return [];
		if (scope === 'sessions') return focused ? sessions[focused.id] ?? [] : [];
		if (!all) return [];
		return projects.flatMap((project) => sessions[project.id] ?? []).toSorted((a, b) => b.updatedAt - a.updatedAt).slice(0, 3);
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

{#snippet session(session: Session | FoundSession, project?: string)}
	{@const disabled = selectable?.(session) === false}
	<ChatItem
		value="session:{session.project.id}:{sessionKey(session)}"
		agent={session.key.agent}
		title={session.title}
		time={session.updatedAt}
		{project}
		excerpt={'excerpt' in session && session.excerpt ? { ...session.excerpt, matches: session.matches } : undefined}
		ended={!!session.ended}
		{disabled}
		onselect={disabled ? undefined : () => onsession(session, query || undefined)}
		ondismiss={search && (() => search.device.send({ type: 'sessions.dismiss', key: session.key }))}
	/>
{/snippet}

{#snippet sessionGroup(text: string, listed: (Session | FoundSession)[], labelled: boolean)}
	{#if listed.length}
		<Command.Group class="group">
			{@render heading(text)}
			<Command.GroupItems>
				{#each listed as item (`${item.project.id}:${sessionKey(item)}`)}
					{@render session(item, labelled ? projectName(item, labels) : undefined)}
				{/each}
			</Command.GroupItems>
		</Command.Group>
	{/if}
{/snippet}

<div class="palette" bind:clientWidth={width}>
	<Command.Root bind:this={command} label={{ all: 'Command Palette', files: 'Open File', sessions: 'Past Sessions', tabs: 'New Tab' }[scope]} shouldFilter={false} loop bind:value={highlighted}>
		<Command.Input
			value={query}
			placeholder={{ all: 'Find a Chat, Project, Rig or file', files: 'Find a file', sessions: 'Find a session', tabs: 'Choose what to open' }[scope]}
			autocomplete="off"
			spellcheck={false}
		>
			{#snippet child({ props })}
				<TextField {...props} bind:value={() => query, (value) => ((query = value), host.set(value))} search bare data-autofocus />
			{/snippet}
		</Command.Input>
		{#if reading}
			<div class="loading"><Loading label="Reading sessions…" /></div>
		{/if}
		<div class="results" class:paned>
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
							{#if found}
								{@render sessionGroup('Sessions', found.here, false)}
								{@render sessionGroup('Other Projects', found.other, true)}
							{:else}
								{@render sessionGroup(all ? 'Recent sessions' : 'Sessions', past, all)}
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
							{#if !reading && !open.length && !past.length && !found?.here.length && !found?.other.length && !(found && (host.pending || host.indexing)) && groups.every(({ entries }) => !entries.length)}
								<ListLabel element="p">{query ? 'No matches' : scope === 'sessions' ? 'No past sessions' : 'Type to find a Chat or file'}</ListLabel>
							{/if}
						</Command.Viewport>
					</div>
				{/snippet}
			</Command.List>
			{#if paned && search}<div class="pane"><SessionPreview device={search.device} session={previewed} /></div>{/if}
		</div>
		{#if covered && (found || scope === 'sessions')}
			{@const indexing = covered.indexed < covered.total}
			{@const stale = outOfDate(covered)}
			{#if indexing || stale || scope === 'sessions'}
				<div class="coverage">
					{#if indexing}
						<Loading label="Indexing {covered.indexed.toLocaleString()} of {covered.total.toLocaleString()} Sessions" />
					{:else if scope === 'sessions'}
						<p>{covered.ended.toLocaleString()} Ended {covered.ended === 1 ? 'Session' : 'Sessions'} · {formatSize(covered.bytes.index + covered.bytes.archive)}</p>
					{/if}
					{#if stale}<p>{stale}</p>{/if}
				</div>
			{/if}
		{/if}
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

	.paned {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1.15fr);
		height: min(26rem, 55dvh);
	}

	.paned .list {
		max-height: none;
	}

	.pane {
		display: flex;
		min-height: 0;
		border-left: var(--border-width) solid var(--border);
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

	.coverage {
		display: grid;
		gap: var(--space-1);
		padding: var(--space-2) var(--bare-inset);
		border-top: var(--border-width) solid var(--border);
	}

	.coverage p {
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
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
