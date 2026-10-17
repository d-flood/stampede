<script lang="ts">
	import ChatComposer from '#lib/chat/ChatComposer.svelte';
	import Activity from '#lib/chat/Activity.svelte';
	import { agentSessions } from '#lib/chat/agents.ts';
	import Button from '#lib/ui/Button.svelte';
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

	let { device, chat, items, session, onsession, active, ondiff }: { device: Device; chat: Chat; items: Item[]; session: string; onsession(id: string): void; active: boolean; ondiff(path: string): void } =
		$props();

	const sessions = $derived(agentSessions(chat, items));
	const selected = $derived(sessions.find(({ id }) => id === session) ?? sessions[0]);
	let requests = $state<Request[]>([]);
	const request = $derived(requests[0]);
	const live = $derived(device.connection === 'connected');
	const agent = $derived(chat.agent === 'codex' ? 'Codex' : 'Claude');
	let rewinding = $state(false);
	let capabilities = $state<Capabilities>({ models: [], efforts: [], modes: [], commands: [], rewindsFiles: false });
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
				requests = [];
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

	function run(command: Builtin, value?: string | number) {
		if (command === 'rewind' && value === undefined) { rewinding = true; return; }
		rewinding = false;
		device.send({ type: 'chat.run', chatId: chat.id, command, value });
	}

	function follow(scroller: HTMLElement) {
		let pinned = true;
		let top = 0;
		let visible = false;
		const observer = new ResizeObserver(() => {
			if (!scroller.getClientRects().length) { visible = false; return; }
			if (pinned) scroller.scrollTop = scroller.scrollHeight;
			else if (!visible) scroller.scrollTop = top;
			top = scroller.scrollTop;
			visible = true;
		});
		observer.observe(scroller);
		observer.observe(scroller.firstElementChild!);
		const off = on(scroller, 'scroll', () => {
			if (!scroller.getClientRects().length) return;
			const next = scroller.scrollTop;
			const bottom = scroller.scrollHeight - next - scroller.clientHeight < 2;
			if (next < top || bottom) pinned = bottom;
			top = scroller.scrollTop;
		});
		return () => {
			off();
			observer.disconnect();
		};
	}
</script>

<div class="panel" class:phone={form() === 'phone'} role="tabpanel" hidden={!active}>
	{#if !selected.id}
		<Chips agent={chat.agent} settings={chat.settings} {choices} onrun={run}>
			{#if form() === 'phone' && chat.context}
				<ContextBar context={chat.context} compact />
			{/if}
		</Chips>
		{#if rewinding}
			<section class="rewind" aria-label="Rewind">
				<strong>Rewind to before a message</strong>
				<p>{capabilities.rewindsFiles ? 'Restores tracked edits. Bash and subagent changes are not restored.' : 'Conversation only. Files are left unchanged.'}</p>
				{#each items as item, index}
					{#if item.kind === 'user'}<button disabled={!live || chat.status !== 'done'} onclick={() => run('rewind', index)}>{item.text || `${item.images} images`}</button>{/if}
				{/each}
				<button onclick={() => (rewinding = false)}>Cancel</button>
			</section>
		{/if}
		<Todos todos={chat.todos} working={chat.status === 'working'} />
	{:else}
		<div class="session-heading"><strong>{selected.task}</strong><span>{selected.status}</span></div>
	{/if}
	{#each sessions as entry (entry.id)}
		<div class="transcript" hidden={entry.id !== selected.id} {@attach follow}>
			<div class="column">
				<Transcript items={entry.items} activity={entry.activity} {ondiff} />
				{#if !entry.id}<Queued queued={chat.queued} />{/if}
			</div>
		</div>
	{/each}
	<div class="foot">
		<Activity status={selected.status} activity={selected.activity} />
		{#if request}
			{#key request.id}
				<RequestCard {request} {agent} disabled={!live} onanswer={(chosen) => answer(request, chosen)} />
			{/key}
		{/if}
		{#if !selected.id}
			<ChatComposer {device} {chat} {live} {request} {capabilities} {choices} onrun={run} />
			{#if form() === 'desktop'}
				<Meters {chat} />
			{/if}
		{:else}
			<div class="session-footer"><span>Subagent session</span><Button onclick={() => onsession('')}>Return to Main</Button></div>
		{/if}
	</div>
</div>

<style>
	.session-heading, .session-footer { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); padding: var(--space-3) var(--space-4); font-size: var(--font-size-sm); }
	.session-heading { border-bottom: var(--border-width) solid var(--border); }
	.session-heading strong { min-width: 0; overflow-wrap: anywhere; }
	.session-heading span, .session-footer span { color: var(--muted-fg); }
	.transcript[hidden] { display: none; }
	.rewind { display: flex; flex-direction: column; flex: none; gap: var(--space-2); max-height: 45vh; padding: var(--space-3); overflow-y: auto; }
	.rewind p { margin: 0; color: var(--muted-fg); }
	.rewind button { min-height: var(--target-min); padding: var(--space-2); border: var(--border-width) solid var(--border); border-radius: var(--radius-md); background: var(--surface-2); color: var(--color-fg); font: inherit; text-align: left; overflow-wrap: anywhere; cursor: pointer; }
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
