<script lang="ts" module>
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const dropdown = lazy(() => import('#lib/shell/AttentionMenu.svelte'));
</script>

<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import { tabOf, type Attention, type Chat } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { disable, enable, notifications, type Notifications } from '#lib/push.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Article from '~icons/ph/article';
	import Bell from '~icons/ph/bell';
	import CheckBold from '~icons/ph/check-bold';
	import CircleNotchBold from '~icons/ph/circle-notch-bold';
	import HandPalmFill from '~icons/ph/hand-palm-fill';

	type Props = { items: Attention[]; projects: Map<string, string>; chats: Chat[]; onselect(item: Attention): void };

	let { items, projects, chats, onselect }: Props = $props();

	const marks = { working: CircleNotchBold, waiting: HandPalmFill, done: CheckBold };

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
	let sheet = $state(false);
	let open = $state(false);
	const Dropdown = $derived(dropdown());
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
		<IconButton {...props} label="Attention list"><Bell /></IconButton>
		{#if items.length}<span class="count"><Badge tone="waiting">{items.length}</Badge></span>{/if}
	</span>
{/snippet}

{#snippet entry(item: Attention)}
	{@const Icon = icon(item)}
	{@const Mark = marks[item.status]}
	<span class="icon">{#if Icon}<Icon />{/if}</span>
	<span class="label">
		<strong>{item.label}</strong>
		<span class="project">in {projects.get(item.projectId)}</span>
	</span>
	<span class="mark {item.status}" role="img" aria-label={item.status} title={item.status}><Mark /></span>
{/snippet}

{#if form() === 'phone'}
	{@render trigger({ onclick: () => (sheet = true) })}
	<Dialog bind:open={sheet} full title="Needs attention">
		<ul class="sheet">
			{#each items as item (tabOf(item))}
				<li>
					<button class="item" onclick={() => ((sheet = false), onselect(item))}>{@render entry(item)}</button>
				</li>
			{:else}
				<li class="empty">Nothing needs attention</li>
			{/each}
		</ul>
		{#if push !== 'unsupported'}
			<Button disabled={pushDisabled} onclick={notify}>{pushLabel}</Button>
		{/if}
	</Dialog>
{:else if Dropdown}
	<Dropdown
		{items}
		push={push === 'unsupported' ? undefined : { label: pushLabel, disabled: pushDisabled, onselect: notify }}
		bind:open
		{trigger}
		{entry}
		{onselect}
	/>
{:else}
	{@render trigger({ onclick: () => ((open = true), warmUp()) })}
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

	.empty {
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
		gap: var(--space-3);
		min-height: var(--target-min);
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-sm);
		cursor: pointer;
		outline: none;
	}

	.sheet {
		margin: 0 0 var(--space-4);
		padding: 0;
		list-style: none;
	}

	.sheet .item {
		box-sizing: border-box;
		width: 100%;
		border: none;
		background: none;
		color: inherit;
		font: inherit;
		text-align: start;
	}

	.sheet .item:hover {
		background: var(--hover);
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
