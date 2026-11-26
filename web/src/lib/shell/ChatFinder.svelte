<script lang="ts">
	import type { Chat, Session } from '#lib/device.svelte.ts';
	import ChatItem from '#lib/shell/ChatItem.svelte';
	import { RECENT } from '#lib/shell/new-tab.ts';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Loading from '#lib/ui/Loading.svelte';
	import type { Status } from '#lib/ui/TabMarks.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { Command } from 'bits-ui';
	import fuzzysort from 'fuzzysort';

	type Props = {
		label: string;
		sessions: Session[];
		reading: boolean;
		/** Open Chats, listed under their own heading before the sessions. */
		chats?: Chat[];
		status?(chat: Chat): Status;
		/** Open Chats that need attention, which lead the list under their own heading. */
		waiting?: Set<string>;
		/** Wraps titles instead of truncating them. */
		full?: boolean;
		/** Holds the list at the recent sessions' height while searching, so nothing around it moves. */
		steady?: boolean;
		onchat?(chat: Chat): void;
		onsession(session: Session): void;
	};

	let { label, sessions, reading, chats, status, waiting, full = false, steady = false, onchat, onsession }: Props = $props();

	let query = $state('');
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
	const past = $derived(query ? find(sessions, (session) => session.title) : sessions.slice(0, RECENT));
	const days = $derived.by(() => {
		if (query) return [{ day: '', sessions: past }];
		const midnight = new Date().setHours(0, 0, 0, 0);
		return [
			{ day: 'Today', sessions: past.filter((session) => session.updatedAt >= midnight) },
			{ day: 'Earlier', sessions: past.filter((session) => session.updatedAt < midnight) }
		];
	});
	const dated = $derived(days.length > 1 && days.every(({ sessions }) => sessions.length));

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

<div class="finder" class:full>
	<Command.Root {label} shouldFilter={false} loop>
		<Command.Input value={query} placeholder={label} autocomplete="off" spellcheck={false} enterkeyhint="go">
			{#snippet child({ props })}
				<TextField {...props} bind:value={query} search />
			{/snippet}
		</Command.Input>
		{#if reading}<div class="loading"><Loading label="Reading sessions…" /></div>{/if}
		<Command.List>
			{#snippet child({ props })}
				<div {...props} class="list" style:height={steady && query ? `${height}px` : undefined} bind:clientHeight={null, (value: number) => { if (!query) height = value; }}>
					<Command.Viewport>
						{#each groups as group (group.name)}
							{#if group.chats.length}
								<Command.Group>
									{@render heading(group.name)}
									<Command.GroupItems>
										{#each group.chats as chat (chat.id)}
											<ChatItem value="chat:{chat.id}" agent={chat.agent} title={chat.title ?? chat.rig} time={chat.created / 1e6} status={status?.(chat)} fresh={!chat.title} size="md" {full} onselect={() => onchat?.(chat)} />
										{/each}
									</Command.GroupItems>
								</Command.Group>
							{/if}
						{/each}
						{#if past.length}
							<Command.Group>
								{#if chats}{@render heading('Recent sessions')}{/if}
								<Command.GroupItems>
									{#each days as { day, sessions: listed } (day)}
										<!-- Each row's time already says when, so the day labels are visual only. -->
										{#if dated}<ListLabel sub aria-hidden="true">{day}</ListLabel>{/if}
										{#each listed as session (`${session.agent}:${session.sessionId}`)}
											<ChatItem value="session:{session.agent}:{session.sessionId}" agent={session.agent} title={session.title} time={session.updatedAt} size="md" {full} onselect={() => onsession(session)} />
										{/each}
									{/each}
								</Command.GroupItems>
							</Command.Group>
						{:else if !reading && !open.length}
							<div class="empty"><ListLabel element="p">{query ? 'No matches' : chats ? 'No Chats or sessions yet' : 'No past sessions'}</ListLabel></div>
						{/if}
					</Command.Viewport>
				</div>
			{/snippet}
		</Command.List>
	</Command.Root>
</div>

<style>
	.loading {
		margin-top: var(--space-2);
	}

	.list {
		margin: var(--space-3) calc(-1 * var(--space-3)) 0;
		overflow-y: auto;
	}

	.full .list {
		max-height: min(24rem, 50dvh);
	}

	.empty {
		padding-inline: var(--space-1);
	}

	/* The highlighted row is what Enter opens, so it only shows while the list is in use. */
	.finder:not(:focus-within, :hover) :global(.item[data-selected]) {
		background: none;
	}
</style>
