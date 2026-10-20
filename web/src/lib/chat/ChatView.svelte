<script lang="ts">
	import ChatComposer from '#lib/chat/ChatComposer.svelte';
	import Activity from '#lib/chat/Activity.svelte';
	import { same, subagents as flatten } from '#lib/chat/agents.ts';
	import Button from '#lib/ui/Button.svelte';
	import ContextBar from '#lib/chat/ContextBar.svelte';
	import Meters from '#lib/chat/Meters.svelte';
	import Queued from '#lib/chat/Queued.svelte';
	import RequestCard from '#lib/chat/RequestCard.svelte';
	import Shells from '#lib/chat/Shells.svelte';
	import Todos from '#lib/chat/Todos.svelte';
	import Chips from '#lib/chat/Chips.svelte';
	import Dock from '#lib/chat/Dock.svelte';
	import Prompt from '#lib/chat/Prompt.svelte';
	import SubagentHeader from '#lib/chat/SubagentHeader.svelte';
	import Text from '#lib/chat/Text.svelte';
	import Transcript from '#lib/chat/Transcript.svelte';
	import { modes } from '#lib/config.svelte.ts';
	import type { Answer, Builtin, Capabilities, Chat, Device, Item, Request, ServerMessage } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import { onMount, tick } from 'svelte';
	import { on } from 'svelte/events';

	let { device, chat, chats, onchat, items, opened, onopen, active, focused = false, ondiff }: { device: Device; chat: Chat; chats: Chat[]; onchat(chat: Chat): void; items: Item[]; opened: string[]; onopen(path: string[]): void; active: boolean; focused?: boolean; ondiff(path: string): void } =
		$props();

	const subagents = $derived(flatten(items));
	const selected = $derived(subagents.find(({ path }) => same(path, opened))?.subagent);
	const trail = $derived(subagents.filter(({ path }) => path.length < opened.length && same(path, opened.slice(0, path.length))));
	const current = $derived(selected ?? chat);
	const source = $derived(chats.find(({ id }) => id === chat.handoffFrom));
	const handoffs = $derived(chats.filter(({ handoffFrom }) => handoffFrom === chat.id));
	let requests = $state<Request[]>([]);
	let attached = $state(false);
	let attending = $state<string>();
	const request = $derived(requests.find(({ subagent }) => subagent && subagent === attending) ?? requests[0]);
	const asker = $derived(request?.subagent && subagents.find(({ subagent }) => subagent.id === request.subagent)?.subagent.task);
	let card = $state<RequestCard>();
	let composer = $state<ChatComposer>();
	const history = $derived([...new Set(items.flatMap((item) => (item.kind === 'user' && item.text ? [item.text] : item.kind === 'shell' ? [`!${item.command}`] : [])).reverse())].reverse());
	const shelling = $derived(items.some((item) => item.kind === 'shell' && item.status === 'running'));
	const live = $derived(device.connection === 'connected');
	const agent = $derived(chat.agent === 'codex' ? 'Codex' : 'Claude');
	let rewinding = $state(false);
	let capabilities = $state<Capabilities>({ models: [], efforts: [], modes: [], commands: [], rewindsFiles: false });
	const choices = $derived({
		model: capabilities.models.map((value) => ({ value, label: value })),
		effort: capabilities.efforts.map((value) => ({ value, label: value })),
		mode: capabilities.modes.filter((choice) => !modes(chat.agent).some((mode) => mode.value === choice.value && !mode.available))
	});

	$effect(() => {
		if (active && attached) painted(chat.id);
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
				attached = false;
				device.send({ type: 'chat.attach', chatId: chat.id });
				break;
			case 'chat.items':
				if (message.chatId !== chat.id) break;
				attached = true;
				requests = [];
				if (opened.length) expand(opened, message.items);
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

	function expand(path: string[], list = items) {
		const index = list.findIndex((item) => item.kind === 'subagent' && item.id === path[0]);
		if (index >= 0) device.send({ type: 'chat.expand', chatId: chat.id, index });
	}

	function open(path: string[]) {
		if (path.length) expand(path);
		onopen(path);
	}

	async function attend(subagent: string) {
		onopen([]);
		attending = subagent;
		await tick();
		card?.focus();
	}

	function run(command: Builtin, value?: string | number) {
		if (command === 'rewind' && value === undefined) { rewinding = true; return; }
		rewinding = false;
		device.send({ type: 'chat.run', chatId: chat.id, command, value });
	}

	function dragover(event: DragEvent) {
		if (event.dataTransfer?.types.includes('Files')) event.preventDefault();
	}

	function drop(event: DragEvent) {
		const files = [...(event.dataTransfer?.files ?? [])];
		if (!files.length) return;
		event.preventDefault();
		void composer?.attach(files);
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

<!-- svelte-ignore a11y_interactive_supports_focus (Dropped images have the Attach image button as their keyboard path.) -->
<div class="panel" class:phone={form() === 'phone'} role="tabpanel" hidden={!active} ondragover={dragover} ondrop={drop}>
	{#if !selected}
		<Chips agent={chat.agent} settings={chat.settings} {choices} onrun={run}>
			{#if form() === 'phone' && chat.context}
				<ContextBar context={chat.context} compact />
			{/if}
		</Chips>
		{#if source || handoffs.length}
			<div class="handoffs" aria-label="Chat handoffs">
				{#if source}<Button onclick={() => onchat(source)}>Source Chat · {source.rig}</Button>{/if}
				{#each handoffs as target (target.id)}<Button onclick={() => onchat(target)}>New Chat · {target.rig}</Button>{/each}
			</div>
		{/if}
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
		<Todos todos={chat.todos} />
	{:else}
		<SubagentHeader subagent={selected} {trail} onopen={open} />
	{/if}
	<div class="transcript" hidden={!!selected} {@attach follow}>
		<div class="column">
			<Transcript {items} answered={attached ? chat.answeredQuestions : []} activity={chat.activity} onopen={open} onattend={attend} {ondiff} onexpand={(index) => device.send({ type: 'chat.expand', chatId: chat.id, index })} />
			<Queued queued={chat.queued} />
		</div>
	</div>
	{#if selected}
		{#key opened.join('/')}
			<div class="transcript" {@attach follow}>
				<div class="column">
					{#if selected.prompt}<Prompt prompt={selected.prompt} />{/if}
					<Transcript items={selected.items} activity={selected.activity} path={opened} onopen={open} onattend={attend} {ondiff} />
					{#if selected.report}
						<section class="report" aria-label="Report">
							<span class="label">Report</span>
							<Text item={{ kind: 'text', text: selected.report, streaming: false, phase: 'final' }} />
						</section>
					{/if}
				</div>
			</div>
		{/key}
	{/if}
	<div class="foot">
		<Shells {items} />
		<Activity status={current.status} activity={current.activity} />
		{#if subagents.length}
			<Dock {subagents} onopen={open} onattend={attend} />
		{/if}
		{#if request}
			{#key request.id}
				<RequestCard bind:this={card} {request} {agent} {asker} disabled={!live} onanswer={(chosen) => answer(request, chosen)} />
			{/key}
		{/if}
		{#if !selected}
			<ChatComposer bind:this={composer} {device} {chat} {history} {shelling} {live} {request} {capabilities} {choices} handoffAvailable={attached && items.some((item) => item.kind === 'user')} {focused} onrun={run} />
			{#if form() === 'desktop'}
				<Meters {chat} />
			{/if}
		{:else}
			<div class="subagent"><span>Subagent</span><Button onclick={() => onopen([])}>Back to Main</Button></div>
		{/if}
	</div>
</div>

<style>
	.handoffs { display: flex; flex: none; flex-wrap: wrap; align-items: center; gap: var(--space-2); padding: var(--space-2) var(--space-4); border-bottom: var(--border-width) solid var(--border); font-size: var(--font-size-sm); }
	.subagent { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); padding: var(--space-3) var(--space-4); font-size: var(--font-size-sm); }
	.subagent span, .report .label { color: var(--muted-fg); }
	.report { display: flex; flex-direction: column; gap: var(--space-1); padding: var(--space-3) var(--space-4); border: var(--border-width) solid var(--border); border-radius: var(--radius-lg); }
	.report .label { font-size: var(--font-size-sm); }
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
