<script lang="ts">
	import Transcript from '#lib/chat/Transcript.svelte';
	import type { Chat, Device, Item, ServerMessage } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Composer from '#lib/shell/Composer.svelte';
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

	function send(text: string) {
		if (!live || !text.trim()) return false;
		device.send({ type: 'chat.send', chatId: chat.id, text });
		return true;
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
	<div class="transcript" {@attach follow}>
		<div class="column">
			<Transcript {items} />
		</div>
	</div>
	<Composer id={chat.id} disabled={!live} insert={false} placeholder="Message {chat.rig}" onsend={send} />
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

	.phone .transcript {
		padding: var(--space-4);
	}

	.phone .column {
		font-size: var(--font-size-lg);
		line-height: 1.5;
	}
</style>
