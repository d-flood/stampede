<script lang="ts">
	import ChatComposer from '#lib/chat/ChatComposer.svelte';
	import ContextBar from '#lib/chat/ContextBar.svelte';
	import Meters from '#lib/chat/Meters.svelte';
	import Queued from '#lib/chat/Queued.svelte';
	import RequestCard from '#lib/chat/RequestCard.svelte';
	import Todos from '#lib/chat/Todos.svelte';
	import Chips from '#lib/chat/Chips.svelte';
	import Transcript from '#lib/chat/Transcript.svelte';
	import type { Answer, Builtin, Capabilities, Chat, Device, Item, Request, ServerMessage } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { onMount } from 'svelte';
	import { on } from 'svelte/events';

	let { device, chat, active, ondiff }: { device: Device; chat: Chat; active: boolean; ondiff(path: string): void } =
		$props();

	let items = $state<Item[]>([]);
	let requests = $state<Request[]>([]);
	const request = $derived(requests[0]);
	const live = $derived(device.connection === 'connected');
	const agent = $derived(chat.agent === 'codex' ? 'Codex' : 'Claude');
	let capabilities = $state<Capabilities>({ models: [], efforts: [], modes: [], commands: [] });
	const choices = $derived({
		model: capabilities.models.map((value) => ({ value, label: value })),
		effort: capabilities.efforts.map((value) => ({ value, label: value })),
		mode: capabilities.modes
	});

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
				if (message.chatId !== chat.id) break;
				items = message.items;
				requests = [];
				break;
			case 'chat.item':
				if (message.chatId === chat.id) items[message.index] = message.item;
				break;
			case 'chat.request':
				if (message.chatId !== chat.id) break;
				requests = [...requests.filter(({ id }) => id !== message.request.id), message.request];
				break;
			case 'chat.resolved':
				if (message.chatId === chat.id) requests = requests.filter(({ id }) => id !== message.requestId);
				break;
			case 'chat.capabilities':
				if (message.chatId === chat.id) capabilities = message.capabilities;
				break;
		}
	}

	function answer(request: Request, answer: Answer) {
		device.send({ type: 'chat.answer', chatId: chat.id, requestId: request.id, answer });
	}

	function run(command: Builtin, value?: string) {
		device.send({ type: 'chat.run', chatId: chat.id, command, value });
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
	<Chips agent={chat.agent} settings={chat.settings} {choices} onrun={run}>
		{#if form() === 'phone' && chat.context}
			<ContextBar context={chat.context} compact />
		{/if}
	</Chips>
	<Todos todos={chat.todos} working={chat.status === 'working'} />
	<div class="transcript" {@attach follow}>
		<div class="column">
			<Transcript {items} {ondiff} />
			<Queued queued={chat.queued} />
		</div>
	</div>
	<div class="foot">
		{#if request}
			{#key request.id}
				<RequestCard {request} {agent} disabled={!live} onanswer={(chosen) => answer(request, chosen)} />
			{/key}
		{/if}
		<ChatComposer {device} {chat} {live} {request} {capabilities} {choices} onrun={run} />
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
