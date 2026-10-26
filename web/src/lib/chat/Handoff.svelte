<script lang="ts">
	import { agentNames, rigs } from '#lib/config.svelte.ts';
	import type { Chat, Device } from '#lib/device.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { onMount } from 'svelte';

	let { device, chat, available }: { device: Device; chat: Chat; available: boolean } = $props();
	let open = $state(false);
	let rig = $state('');
	let focus = $state('');
	let preparing = $state(false);
	let error = $state('');
	const live = $derived(device.connection === 'connected');
	const configured = $derived(rigs());
	const ready = $derived(live && available && chat.status === 'done' && !chat.queued.length && !preparing);

	onMount(() => device.listen({ message(message) {
		if (message.type === 'chat.handoff.error' && message.chatId === chat.id) {
			preparing = false;
			error = message.error;
			open = true;
		} else if (message.type === 'chat.created' && message.chat.handoffFrom === chat.id) {
			preparing = false;
			open = false;
		} else if (message.type === 'hello') {
			preparing = false;
		}
	} }));

	export function enabled() {
		return ready && configured.length > 0;
	}

	export function show() {
		rig = configured.some(({ name }) => name === chat.rig) ? chat.rig : (configured[0]?.name ?? '');
		focus = '';
		error = '';
		open = true;
	}

	function prepare() {
		if (!ready || !configured.some(({ name }) => name === rig)) return;
		error = '';
		preparing = true;
		device.send({ type: 'chat.handoff', chatId: chat.id, rig, focus: focus.trim() });
	}
</script>

<Dialog bind:open title="New Chat from here" description="Prepare a handoff, then review and edit it in the new Chat’s Composer before sending.">
	<form id="handoff-{chat.id}" onsubmit={(event) => { event.preventDefault(); prepare(); }}>
		<label for="handoff-rig-{chat.id}">Rig</label>
		<select id="handoff-rig-{chat.id}" bind:value={rig} disabled={preparing}>
			{#each configured as option (option.name)}
				<option value={option.name}>{option.name} · {agentNames[option.agent]}</option>
			{/each}
		</select>
		<label for="handoff-focus-{chat.id}">Focus <span>(optional)</span></label>
		<textarea id="handoff-focus-{chat.id}" bind:value={focus} disabled={preparing} placeholder="What should the new Chat focus on?" rows="3" {@attach (element) => { requestAnimationFrame(() => element.focus()); }}></textarea>
		{#if preparing}<p role="status">Preparing handoff in {chat.rig}…</p>{/if}
		{#if error}<p role="alert">{error}</p>{/if}
	</form>
	{#snippet actions()}
		{#if preparing}
			<Button disabled={!live} onclick={() => device.send({ type: 'chat.interrupt', chatId: chat.id })}>Stop</Button>
		{:else}
			<Button onclick={() => (open = false)}>Cancel</Button>
		{/if}
		<Button variant="primary" type="submit" form="handoff-{chat.id}" disabled={!ready || !configured.some(({ name }) => name === rig)}>Prepare handoff</Button>
	{/snippet}
</Dialog>

<style>
	form { display: flex; flex-direction: column; gap: var(--space-2); }
	label { font-weight: 600; }
	label:not(:first-child) { margin-top: var(--space-2); }
	span, p { color: var(--muted-fg); }
	span { font-weight: normal; }
	select, textarea { box-sizing: border-box; width: 100%; min-height: var(--target-min); padding: var(--space-2); border: var(--border-width) solid var(--border); border-radius: var(--radius-md); background: var(--color-bg); color: var(--color-fg); font: inherit; }
	textarea { resize: vertical; max-height: 30vh; }
	select:focus-visible, textarea:focus-visible { outline: var(--border-width) solid var(--status-working); outline-offset: var(--border-width); }
	p { margin: var(--space-2) 0 0; }
	[role='alert'] { color: var(--ansi-1); }
</style>
