<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import { tabOf, type Attention, type Chat } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { disable, enable, notifications, type Notifications } from '#lib/push.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import { WaitingMark, WorkingMark } from '#lib/ui/spinners.ts';
	import Article from '~icons/ph/article';
	import ChatsCircle from '~icons/ph/chats-circle';
	import CheckBold from '~icons/ph/check-bold';

	type Props = { items: Attention[]; projects: Map<string, string>; chats: Chat[]; onselect(item: Attention): void };

	let { items, projects, chats, onselect }: Props = $props();

	const marks = { working: WorkingMark, waiting: WaitingMark, done: CheckBold };

	const listed = $derived(new Set(items.map(tabOf)));
	/** Every open Chat that doesn't already need attention, grouped by Project in rail order. */
	const others = $derived.by(() => {
		const groups = new Map<string, Attention[]>();
		for (const chat of chats) {
			if (listed.has(chat.id)) continue;
			const group = groups.get(chat.projectId) ?? [];
			group.push({ projectId: chat.projectId, chatId: chat.id, label: chat.title ?? chat.rig, status: chat.status });
			groups.set(chat.projectId, group);
		}
		return [...groups].map(([projectId, entries]) => ({ heading: projects.get(projectId) ?? projectId, entries }));
	});

	function icon(item: Attention) {
		if (!('chatId' in item)) return Article;
		const chat = chats.find(({ id }) => id === item.chatId);
		return chat && logos[chat.agent];
	}

	const labels = { off: 'Enable notifications', on: 'Disable notifications', blocked: 'Notifications blocked' };

	let push = $state<Notifications>('unsupported');
	let pending = $state(false);
	let failed = $state(false);
	const pushDisabled = $derived(pending || push === 'blocked');
	const pushLabel = $derived(failed ? `Retry ${push === 'on' ? 'disabling' : 'enabling'} notifications` : push === 'unsupported' ? '' : labels[push]);
	const menu = $derived([
		{ heading: 'Needs attention', empty: 'Nothing needs attention', items: items.map((item) => ({ label: item.label, data: item, onselect: () => onselect(item) })) },
		...others.map(({ heading, entries }) => ({ heading, items: entries.map((item) => ({ label: item.label, data: item, onselect: () => onselect(item) })) })),
		...(push === 'unsupported' ? [] : [{ label: pushLabel, disabled: pushDisabled, onselect: notify }])
	]);
	let sheet = $state(false);
	notifications().then((state) => (push = state));

	async function notify() {
		if (pending) return;
		pending = true;
		failed = false;
		try {
			push = await (push === 'on' ? disable() : enable());
		} catch {
			failed = true;
		} finally {
			pending = false;
		}
	}
</script>

{#snippet trigger(props: Record<string, unknown>)}
	<span class="trigger">
		<IconButton {...props} label="Attention list" title="Chats"><ChatsCircle /></IconButton>
		{#if items.length}<span class="count"><Badge tone="waiting">{items.length}</Badge></span>{/if}
	</span>
{/snippet}

{#snippet entry(data: unknown)}
	{@const item = data as Attention}
	{@const Icon = icon(item)}
	{@const Mark = marks[item.status]}
	<span class="icon">{#if Icon}<Icon />{/if}</span>
	<span class="label">
		<strong>{item.label}</strong>
		{#if listed.has(tabOf(item))}<span class="project">in {projects.get(item.projectId)}</span>{/if}
	</span>
	<span class="mark {item.status}" role="img" aria-label={item.status} title={item.status}><Mark /></span>
{/snippet}

{#if form() === 'phone'}
	{@render trigger({ onclick: () => (sheet = true) })}
	<Dialog bind:open={sheet} full title="Chats">
		<ul class="sheet" aria-label="Needs attention">
			<ListLabel element="li">Needs attention</ListLabel>
			{#each items as item (tabOf(item))}
				<li>
					<ListItem element="button" onclick={() => ((sheet = false), onselect(item))}>{@render entry(item)}</ListItem>
				</li>
			{:else}
				<ListLabel element="li">Nothing needs attention</ListLabel>
			{/each}
		</ul>
		{#each others as { heading, entries } (heading)}
			<ul class="sheet" aria-label={heading}>
				<ListLabel element="li">{heading}</ListLabel>
				{#each entries as item (tabOf(item))}
					<li>
						<ListItem element="button" onclick={() => ((sheet = false), onselect(item))}>{@render entry(item)}</ListItem>
					</li>
				{/each}
			</ul>
		{/each}
		{#if push !== 'unsupported'}
			<Button disabled={pushDisabled} onclick={notify}>{pushLabel}</Button>
		{/if}
	</Dialog>
{:else}
	<Menu items={menu} row={entry} {trigger} side="right" align="end" />
{/if}

<style>
	.trigger {
		position: relative;
		display: inline-grid;
	}

	.count {
		position: absolute;
		top: calc(-1 * var(--space-1));
		right: calc(-1 * var(--space-1));
		pointer-events: none;
	}


	.sheet {
		margin: 0 0 var(--space-4);
		padding: 0;
		list-style: none;
	}

	.label {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.project {
		color: var(--muted-fg);
	}

	.icon,
	.mark {
		display: inline-flex;
		flex: none;
	}

	.icon {
		width: 1em;
	}

	.working {
		color: var(--status-working);
	}

	.waiting {
		color: var(--status-waiting);
	}

	.done {
		color: var(--status-done);
	}
</style>
