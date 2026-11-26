<script lang="ts">
	import type { Chat, Session } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import ChatFinder from '#lib/shell/ChatFinder.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Popover from '#lib/ui/Popover.svelte';
	import type { Status } from '#lib/ui/TabMarks.svelte';
	import ListMagnifyingGlass from '~icons/ph/list-magnifying-glass';

	type Props = {
		chats: Chat[];
		status(chat: Chat): Status;
		/** Chats that need attention. */
		waiting: Set<string>;
		sessions: Session[];
		reading: boolean;
		onopen(): void;
		onchat(chat: Chat): void;
		onsession(session: Session): void;
	};

	let { chats, status, waiting, sessions, reading, onopen, onchat, onsession }: Props = $props();

	let open = $state(false);
	const attention = $derived(chats.some((chat) => waiting.has(chat.id)));
	const title = $derived(attention ? 'Chats and recent sessions, with a Chat waiting' : 'Chats and recent sessions');
	let chose = false;

	function toggle(value: boolean) {
		open = value;
		if (value) onopen();
	}

	function pick(run: () => void) {
		chose = true;
		open = false;
		run();
	}

	/** Leaves focus with what was chosen instead of returning it to the trigger. */
	function release(event: Event) {
		if (chose) event.preventDefault();
		chose = false;
	}
</script>

{#snippet trigger(props: Record<string, unknown>)}
	<!-- Serves every pane, so focusing it mustn't make its own pane the current one. -->
	<span class="trigger" class:phone={form() === 'phone'}>
		{#if form() === 'phone'}
			<IconButton {...props} label="Chats" {title} onfocusin={(event: FocusEvent) => event.stopPropagation()}><ListMagnifyingGlass />{#if attention}<span class="dot"></span>{/if}</IconButton>
		{:else}
			<Button {...props} variant="ghost" size="sm" {title} onfocusin={(event: FocusEvent) => event.stopPropagation()}><ListMagnifyingGlass />Chats{#if attention}<span class="dot"></span>{/if}</Button>
		{/if}
	</span>
{/snippet}

{#snippet finder()}
	<ChatFinder
		label="Find a Chat or session"
		{chats}
		{status}
		{waiting}
		{sessions}
		{reading}
		flush
		onchat={(chat) => pick(() => onchat(chat))}
		onsession={(session) => pick(() => onsession(session))}
	/>
{/snippet}

{#if form() === 'phone'}
	{@render trigger({ 'aria-haspopup': 'dialog', 'aria-expanded': open, onclick: () => toggle(true) })}
	<Dialog bind:open={() => open, toggle} sheet title="Chats" oncloseautofocus={release}><div class="sheet">{@render finder()}</div></Dialog>
{:else}
	<Popover bind:open={() => open, toggle} label="Chats" align="end" width="30rem" flush {trigger} oncloseautofocus={release}>{@render finder()}</Popover>
{/if}

<style>
	.trigger {
		display: inline-flex;
	}

	.trigger :global(button) {
		position: relative;
	}

	.trigger:not(.phone) :global(button) {
		padding-right: var(--space-3);
	}

	.dot {
		position: absolute;
		top: calc(2 * var(--border-width));
		right: calc(2 * var(--border-width));
		width: var(--space-2);
		height: var(--space-2);
		border-radius: 50%;
		background: var(--status-waiting);
		pointer-events: none;
	}

	.phone .dot {
		top: var(--space-2);
		right: var(--space-2);
	}

	.sheet {
		margin: 0 calc(-1 * var(--space-4)) calc(-1 * var(--space-4));
	}
</style>
