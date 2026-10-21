<script lang="ts">
	import type { Chat, Device } from '#lib/device.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { onMount } from 'svelte';
	import GitBranch from '~icons/ph/git-branch';

	let { device, chat }: { device: Device; chat: Chat } = $props();
	let open = $state(false);
	let branch = $state('');
	let moving = $state(false);
	let error = $state('');
	const live = $derived(device.connection === 'connected');
	const ready = $derived(live && chat.status === 'done' && !chat.queued.length && !moving);

	onMount(() => device.listen({ message(message) {
		if (message.type === 'chat.move.error' && message.chatId === chat.id) {
			moving = false;
			error = message.error;
			open = true;
		} else if (message.type === 'chat.moved' && message.chatId === chat.id) {
			moving = false;
			open = false;
		} else if (message.type === 'hello') {
			moving = false;
		}
	} }));

	function show() {
		branch = '';
		error = '';
		open = true;
	}

	function move() {
		if (!ready || !branch.trim()) return;
		error = '';
		moving = true;
		device.send({ type: 'chat.move', chatId: chat.id, branch: branch.trim() });
	}
</script>

<Button aria-label="Move to new worktree…" title="Move to new worktree…" disabled={!ready} onclick={show}><GitBranch /></Button>

<Dialog bind:open title="Move to new worktree" description="Cut a new branch from the current one, carry the uncommitted changes over, and continue this Chat there.">
	<form id="move-{chat.id}" onsubmit={(event) => { event.preventDefault(); move(); }}>
		<label for="move-branch-{chat.id}">New branch</label>
		<input id="move-branch-{chat.id}" bind:value={branch} required autocomplete="off" autocapitalize="off" spellcheck="false" disabled={moving} {@attach (element) => element.focus()} />
		{#if moving}<p role="status">Moving {chat.rig}…</p>{/if}
		{#if error}<p role="alert">{error}</p>{/if}
	</form>
	{#snippet actions()}
		<Button disabled={moving} onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary" type="submit" form="move-{chat.id}" disabled={!ready || !branch.trim()}>Move</Button>
	{/snippet}
</Dialog>

<style>
	form { display: flex; flex-direction: column; gap: var(--space-2); }
	label { font-weight: 600; }
	p { margin: var(--space-2) 0 0; color: var(--muted-fg); overflow-wrap: anywhere; }
	input { box-sizing: border-box; width: 100%; min-height: var(--target-min); padding: var(--space-2); border: var(--border-width) solid var(--border); border-radius: var(--radius-md); background: var(--color-bg); color: var(--color-fg); font: inherit; }
	input:focus-visible { outline: var(--border-width) solid var(--status-working); outline-offset: var(--border-width); }
	[role='alert'] { color: var(--ansi-1); }
</style>
