<script lang="ts">
	import ChatComposer from '#lib/chat/ChatComposer.svelte';
	import ContextBar from '#lib/chat/ContextBar.svelte';
	import Meters from '#lib/chat/Meters.svelte';
	import Queued from '#lib/chat/Queued.svelte';
	import Transcript from '#lib/chat/Transcript.svelte';
	import type { Chat, Device, Item, ServerMessage } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { onMount } from 'svelte';
	import { on } from 'svelte/events';

	let { device, chat, active }: { device: Device; chat: Chat; active: boolean } = $props();

	let items = $state<Item[]>([]);
	const live = $derived(device.connection === 'connected');

	onMount(() => {
		device.send({ type: 'chat.attach', chatId: chat.id });
		const unlisten = device.listen({ message });
		return () => {
			unlisten();
			device.send({ type: 'chat.detach', chatId: chat.id });
		};
	});

	function message(message: ServerMessage) {
		switch (message.type) {
			case 'hello':
				device.send({ type: 'chat.attach', chatId: chat.id });
				break;
			case 'chat.items':
				if (message.chatId === chat.id) items = message.items;
				break;
			case 'chat.item':
				if (message.chatId === chat.id) items[message.index] = message.item;
				break;
		}
	}

	function follow(scroller: HTMLElement) {
		let pinned = true;
		const observer = new ResizeObserver(() => {
			if (pinned) scroller.scrollTop = scroller.scrollHeight;
		});
		observer.observe(scroller);
		observer.observe(scroller.firstElementChild!);
		const off = on(scroller, 'scroll', () => {
			pinned = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 2;
		});
		return () => {
			off();
			observer.disconnect();
		};
	}
</script>

<div class="panel" class:phone={form() === 'phone'} role="tabpanel" hidden={!active}>
	{#if form() === 'phone' && chat.context}
		<div class="chips">
			<ContextBar context={chat.context} compact />
		</div>
	{/if}
	<div class="transcript" {@attach follow}>
		<div class="column">
			<Transcript {items} />
			<Queued queued={chat.queued} />
		</div>
	</div>
	<div class="foot">
		<ChatComposer {device} {chat} {live} />
		{#if form() === 'desktop'}
			<Meters {chat} />
		{/if}
	</div>
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.panel[hidden] {
		display: none;
	}

	.chips {
		display: flex;
		flex: none;
		align-items: center;
		justify-content: flex-end;
		padding: var(--space-2) var(--space-4);
		border-bottom: var(--border-width) solid var(--border);
	}

	.transcript {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding: var(--space-6) var(--space-6) var(--space-4);
	}

	.column {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		max-width: 760px;
		margin: 0 auto;
		line-height: 1.55;
	}

	.foot {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-2);
		width: min(760px, 100% - 2 * var(--space-6));
		margin: 0 auto var(--space-4);
	}

	.phone .transcript {
		padding: var(--space-4);
	}

	.phone .foot {
		width: auto;
		margin: 0 var(--space-3) var(--space-3);
	}

	.phone .column {
		font-size: var(--font-size-lg);
		line-height: 1.5;
	}
</style>
