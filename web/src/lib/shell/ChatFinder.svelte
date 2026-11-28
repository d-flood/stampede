<script lang="ts">
	import type { Chat, FoundSession, Session } from '#lib/device.svelte.ts';
	import ChatItem from '#lib/shell/ChatItem.svelte';
	import SessionPreview from '#lib/shell/SessionPreview.svelte';
	import { RECENT } from '#lib/shell/new-tab.ts';
	import { grouped, PANE_MIN, projectName, SessionSearch, sessionKey, type Searching } from '#lib/shell/session-search.svelte.ts';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import type { Status } from '#lib/ui/TabMarks.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { Command } from 'bits-ui';
	import fuzzysort from 'fuzzysort';

	type Props = {
		label: string;
		/** The focused Project's recent Sessions. */
		sessions: Session[];
		reading: boolean;
		search: Searching;
		labels?: Map<string, string>;
		/** Whether a found Session can be opened from here. */
		selectable?(session: Session): boolean;
		/** Open Chats, listed under their own heading before the sessions. */
		chats?: Chat[];
		status?(chat: Chat): Status;
		/** Open Chats that need attention, which lead the list under their own heading. */
		waiting?: Set<string>;
		/** Renders the search field as a flush top row with a hairline below. */
		flush?: boolean;
		/** Holds the list at the recent sessions' height, or room for a few found rows, while searching, so little around it moves. */
		steady?: boolean;
		onchat?(chat: Chat): void;
		/** `query` is set when a typed search found the Session. */
		onsession(session: Session | FoundSession, query?: string): void;
	};

	let { label, sessions, reading, search, labels, selectable, chats, status, waiting, flush = false, steady = false, onchat, onsession }: Props = $props();

	let query = $state('');
	const host = new SessionSearch(() => search);
	const found = $derived(query ? grouped(host.found, sessions, query, search.focus) : undefined);
	let height = $state(0);
	const open = $derived(find(chats ?? [], (chat) => chat.title ?? chat.rig));
	const groups = $derived.by(() => {
		if (query) return [{ name: 'Open Chats', chats: open }];
		const attention = open.filter((chat) => waiting?.has(chat.id));
		const running = open.filter((chat) => !waiting?.has(chat.id) && (status?.(chat) === 'working' || status?.(chat) === 'background'));
		const rest = open.filter((chat) => !attention.includes(chat) && !running.includes(chat));
		const shown = [{ name: 'Waiting', chats: attention }, { name: 'Running', chats: running }, { name: 'Open', chats: rest }].filter((group) => group.chats.length);
		return shown.length > 1 ? shown : [{ name: 'Open Chats', chats: open }];
	});
	const past = $derived<(Session | FoundSession)[]>(found ? [...found.here, ...found.other] : sessions.slice(0, RECENT));
	let width = $state(0);
	let highlighted = $state('');
	const paned = $derived(!!query && past.length > 0 && width >= PANE_MIN);
	const previewed = $derived(paned ? past.find((session) => `session:${sessionKey(session)}` === highlighted) : undefined);
	const days = $derived.by(() => {
		const midnight = new Date().setHours(0, 0, 0, 0);
		return [
			{ day: 'Today', sessions: past.filter((session) => session.updatedAt >= midnight) },
			{ day: 'Earlier', sessions: past.filter((session) => session.updatedAt < midnight) }
		];
	});
	const dated = $derived(days.every(({ sessions }) => sessions.length));

	function find<T>(items: T[], key: (item: T) => string): T[] {
		return query ? fuzzysort.go(query, items, { key, limit: 0, threshold: 0 }).map((result) => result.obj) : items;
	}
</script>

{#snippet heading(text: string)}
	<Command.GroupHeading>
		{#snippet child({ props })}
			<ListLabel {...props} size="md">{text}</ListLabel>
		{/snippet}
	</Command.GroupHeading>
{/snippet}

{#snippet session(session: Session | FoundSession)}
	{@const here = !query || session.project.id === search.focus}
	{@const disabled = selectable?.(session) === false}
	<ChatItem
		value="session:{sessionKey(session)}"
		agent={session.key.agent}
		title={session.title}
		time={session.updatedAt}
		project={here ? undefined : projectName(session, labels)}
		excerpt={'excerpt' in session && session.excerpt ? { ...session.excerpt, matches: session.matches } : undefined}
		ended={!!session.ended}
		{disabled}
		size="md"
		onselect={disabled ? undefined : () => onsession(session, query || undefined)}
	/>
{/snippet}

<div class="finder" class:flush bind:clientWidth={width}>
	<Command.Root {label} shouldFilter={false} loop bind:value={highlighted}>
		<Command.Input value={query} placeholder={label} autocomplete="off" spellcheck={false} enterkeyhint="go">
			{#snippet child({ props })}
				<TextField {...props} bind:value={() => query, (value) => ((query = value), host.set(value))} search bare={flush} />
			{/snippet}
		</Command.Input>
		{#if reading}<div class="loading"><Loading label="Reading sessions…" /></div>{/if}
		<div class="results" class:paned>
			<Command.List>
				{#snippet child({ props })}
					<div {...props} class="list" style:height={steady && query && !paned ? `max(${height}px, ${RECENT} * (var(--target-min) + var(--space-2)))` : undefined} bind:clientHeight={null, (value: number) => { if (!query) height = value; }}>
						<Command.Viewport>
							{#each groups as group (group.name)}
								{#if group.chats.length}
									<Command.Group class="group">
										{@render heading(group.name)}
										<Command.GroupItems>
											{#each group.chats as chat (chat.id)}
												<ChatItem value="chat:{chat.id}" agent={chat.agent} title={chat.title ?? chat.rig} time={chat.created / 1e6} status={status?.(chat)} fresh={!chat.title} size="md" onselect={() => onchat?.(chat)} />
											{/each}
										</Command.GroupItems>
									</Command.Group>
								{/if}
							{/each}
							{#if found}
								{#if found.here.length}
									<Command.Group class={chats && open.length ? 'group history' : 'group'}>
										{#if chats}{@render heading('Sessions')}{/if}
										<Command.GroupItems>
											{#each found.here as hit (sessionKey(hit))}{@render session(hit)}{/each}
										</Command.GroupItems>
									</Command.Group>
								{/if}
								{#if found.other.length}
									<Command.Group class={(chats && open.length) || found.here.length ? 'group history' : 'group'}>
										{@render heading('Other Projects')}
										<Command.GroupItems>
											{#each found.other as hit (sessionKey(hit))}{@render session(hit)}{/each}
										</Command.GroupItems>
									</Command.Group>
								{/if}
							{:else if past.length}
								<Command.Group class={chats && open.length ? 'group history' : 'group'}>
									{#if chats}{@render heading('Recent sessions')}{/if}
									<Command.GroupItems>
										{#each days as { day, sessions: listed } (day)}
											{#if dated}<ListLabel sub aria-hidden="true">{day}</ListLabel>{/if}
											{#each listed as item (sessionKey(item))}{@render session(item)}{/each}
										{/each}
									</Command.GroupItems>
								</Command.Group>
							{/if}
							{#if !past.length && !open.length && !reading && !(query && (host.pending || host.indexing))}
								<div class="empty"><ListLabel element="p">{query ? 'No matches' : chats ? 'No Chats or sessions yet' : 'No past sessions'}</ListLabel></div>
							{/if}
						</Command.Viewport>
					</div>
				{/snippet}
			</Command.List>
			{#if paned}<div class="pane"><SessionPreview device={search.device} session={previewed} /></div>{/if}
		</div>
		{#if query && host.indexing && host.coverage}
			<div class="coverage"><Loading label="Indexing {host.coverage.indexed.toLocaleString()} of {host.coverage.total.toLocaleString()} Sessions" /></div>
		{/if}
	</Command.Root>
</div>

<style>
	.finder.flush {
		display: flex;
		flex-direction: column;
		min-height: 0;
		--bare-inset: calc(var(--space-2) + var(--space-3));
		--bare-gap: var(--space-3);
	}

	.flush :global([data-command-root]) {
		display: flex;
		flex-direction: column;
		min-height: 0;
	}

	.loading {
		margin-top: var(--space-2);
	}

	.flush .loading {
		padding-inline: var(--bare-inset);
	}

	.coverage {
		margin-top: var(--space-2);
	}

	.flush .coverage {
		margin: 0;
		padding: var(--space-2) var(--bare-inset);
		border-top: var(--border-width) solid var(--border);
	}

	.list {
		margin: var(--space-3) calc(-1 * var(--space-3)) 0;
		overflow-y: auto;
	}

	.paned {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1.15fr);
		gap: var(--space-4);
		height: min(26rem, 55dvh);
		margin-top: var(--space-3);
	}

	.paned .list {
		margin-top: 0;
	}

	.pane {
		display: flex;
		min-height: 0;
		overflow: hidden;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
	}

	.flush .list {
		margin: 0;
		padding: var(--space-2);
		max-height: min(26rem, 55dvh);
	}

	.finder :global(.group + .group) {
		margin-top: var(--space-2);
	}

	.finder :global(.group .caption.md) {
		min-height: var(--control-min);
		padding-bottom: var(--space-1);
	}

	.finder.flush :global(.group .sub:first-child) {
		margin-top: 0;
	}

	.finder :global(.group.history) {
		margin-top: var(--space-4);
		padding-top: var(--space-3);
		border-top: var(--border-width) solid var(--border);
	}

	.empty {
		padding-inline: var(--space-1);
	}

	/* The highlighted row is what Enter opens, so it only shows while the list is in use. */
	.finder:not(:focus-within, :hover) :global(.item[data-selected]) {
		background: none;
	}
</style>
