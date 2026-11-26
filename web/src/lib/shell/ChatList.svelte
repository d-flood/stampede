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
		sessions: Session[];
		reading: boolean;
		onopen(): void;
		onchat(chat: Chat): void;
		onsession(session: Session): void;
	};

	let { chats, status, sessions, reading, onopen, onchat, onsession }: Props = $props();

	let open = $state(false);
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
	{#if form() === 'phone'}
		<IconButton {...props} label="Chats" title="Chats and recent sessions" onfocusin={(event: FocusEvent) => event.stopPropagation()}><ListMagnifyingGlass /></IconButton>
	{:else}
		<Button {...props} variant="ghost" size="sm" title="Chats and recent sessions" onfocusin={(event: FocusEvent) => event.stopPropagation()}><ListMagnifyingGlass />Chats</Button>
	{/if}
{/snippet}

{#snippet finder()}
	<ChatFinder
		label="Find a Chat or session"
		{chats}
		{status}
		{sessions}
		{reading}
		full
		onchat={(chat) => pick(() => onchat(chat))}
		onsession={(session) => pick(() => onsession(session))}
	/>
{/snippet}

{#if form() === 'phone'}
	{@render trigger({ 'aria-haspopup': 'dialog', 'aria-expanded': open, onclick: () => toggle(true) })}
	<Dialog bind:open={() => open, toggle} sheet title="Chats" oncloseautofocus={release}>{@render finder()}</Dialog>
{:else}
	<Popover bind:open={() => open, toggle} label="Chats" align="end" width="30rem" {trigger} oncloseautofocus={release}>{@render finder()}</Popover>
{/if}
