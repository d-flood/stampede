<script lang="ts">
	import ChatComposer from '#lib/chat/ChatComposer.svelte';
	import EmptyChat from '#lib/chat/EmptyChat.svelte';
	import FilePreview from '#lib/chat/FilePreview.svelte';
	import Activity from '#lib/chat/Activity.svelte';
	import { same, subagents as flatten } from '#lib/chat/agents.ts';
	import Button from '#lib/ui/Button.svelte';
	import Queued from '#lib/chat/Queued.svelte';
	import Rewind from '#lib/chat/Rewind.svelte';
	import MessageNav from '#lib/chat/MessageNav.svelte';
	import { clearDraft } from '#lib/chat/questions.ts';
	import RequestCard from '#lib/chat/RequestCard.svelte';
	import { commands } from '#lib/chat/shells.ts';
	import Todos from '#lib/chat/Todos.svelte';
	import Tray from '#lib/chat/Tray.svelte';
	import { canvases as written } from '#lib/canvas.svelte.ts';
	import Handoff from '#lib/chat/Handoff.svelte';
	import MoveToWorktree from '#lib/chat/MoveToWorktree.svelte';
	import Prompt from '#lib/chat/Prompt.svelte';
	import SubagentHeader from '#lib/chat/SubagentHeader.svelte';
	import Text from '#lib/chat/Text.svelte';
	import Transcript from '#lib/chat/Transcript.svelte';
	import { modes } from '#lib/config.svelte.ts';
	import type { Answer, Builtin, Capabilities, Chat, Device, Item, Request, ResetCredits, ServerMessage } from '#lib/device.svelte.ts';
	import type { Cursor } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { fileLink, relative } from '#lib/links.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import { onMount, tick } from 'svelte';
	import { on } from 'svelte/events';
	import GitBranch from '~icons/ph/git-branch';
	import GitFork from '~icons/ph/git-fork';

	let { device, chat, chats, onchat, items, saved, opened, onopen, active, focused = false, project, root, branch, codexResets = null, ondiff, onfile, oncanvas }: { device: Device; chat: Chat; chats: Chat[]; onchat(chat: Chat): void; items: Item[]; saved: string[]; opened: string[]; onopen(path: string[]): void; active: boolean; focused?: boolean; project?: string; root?: string; branch?: string; codexResets?: ResetCredits | null; ondiff(path: string): void; onfile(path: string, cursor?: Cursor): void; oncanvas(canvas: string): void } =
		$props();

	const subagents = $derived(flatten(items));
	const canvases = $derived([...new Set([...written(items), ...saved])]);
	const shell = $derived(chat.background ? commands(items).findLast(({ item }) => item.status === 'running')?.item.command : undefined);
	const selected = $derived(subagents.find(({ path }) => same(path, opened))?.subagent);
	const trail = $derived(subagents.filter(({ path }) => path.length < opened.length && same(path, opened.slice(0, path.length))));
	const source = $derived(chats.find(({ id }) => id === chat.handoffFrom));
	const handoffs = $derived(chats.filter(({ handoffFrom }) => handoffFrom === chat.id));
	let requests = $state<Request[]>([]);
	let attached = $state(false);
	let attending = $state<string>();
	const request = $derived(requests.find(({ subagent }) => subagent && subagent === attending) ?? requests[0]);
	const asker = $derived(request?.subagent && subagents.find(({ subagent }) => subagent.id === request.subagent)?.subagent.task);
	let card = $state<RequestCard>();
	let composer = $state<ChatComposer>();
	let handoff = $state<Handoff>();
	let mover = $state<MoveToWorktree>();
	const onhandoff = $derived(handoff?.enabled() ? () => handoff?.show() : undefined);
	const branches = $derived([
		{ label: 'New Chat from here…', icon: GitFork, disabled: !handoff?.enabled(), onselect: () => handoff?.show() },
		{ label: 'Move to new worktree…', icon: GitBranch, disabled: !mover?.enabled(), onselect: () => mover?.show() }
	]);
	const history = $derived([...new Set(items.flatMap((item) => (item.kind === 'user' && item.text ? [item.text] : item.kind === 'shell' ? [`!${item.command}`] : [])).reverse())].reverse());
	const shelling = $derived(items.some((item) => item.kind === 'shell' && item.status === 'running'));
	const live = $derived(device.connection === 'connected');
	const agent = $derived(chat.agent === 'codex' ? 'Codex' : 'Claude');
	let rewinding = $state(false);
	let preview = $state<{ path: string; line?: number; anchor: DOMRect }>();
	let rewindTarget = $state<number>();
	let scroller = $state<HTMLElement>();
	let messageNavHeight = $state(0);
	const rewindUnavailable = $derived.by(() => {
		if (!live) return 'Reconnect to rewind.';
		if (!attached) return 'Loading the conversation…';
		if (chat.queued.length) return 'Cancel queued messages before rewinding.';
		if (chat.status !== 'done') return 'Wait for the current turn to finish, or interrupt first.';
		if (shelling) return 'Wait for running commands to finish, or interrupt first.';
		if (subagents.some(({ subagent }) => subagent.status !== 'done')) return 'Wait for subagents to finish, or interrupt first.';
	});
	let capabilities = $state<Capabilities>({ models: [], efforts: [], speeds: [], ultracode: false, modes: [], commands: [], rewindsFiles: false });
	const choices = $derived({
		model: capabilities.models.map((value) => ({ value, label: value })),
		effort: capabilities.efforts.map((value) => ({ value, label: value })),
		speed: capabilities.speeds,
		ultracode: capabilities.ultracode ? ['off', 'on'].map((value) => ({ value, label: value })) : [],
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
				clearDraft(message.requestId);
				if (message.chatId === chat.id) requests = requests.filter(({ id }) => id !== message.requestId);
				break;
			case 'chat.capabilities':
				if (message.chatId === chat.id) capabilities = message.capabilities;
				break;
			case 'chat.unqueued':
				if (message.chatId === chat.id) composer?.restore(message.text, message.images);
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
		if (command === 'rewind') { rewindTarget = typeof value === 'number' ? value : undefined; rewinding = true; return; }
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

	function links(panel: HTMLElement) {
		let timer: ReturnType<typeof setTimeout> | undefined;
		const linked = (event: Event) => {
			const anchor = (event.target as Element).closest<HTMLElement>('.text a.file');
			const file = anchor && fileLink(anchor.getAttribute('href')!);
			return file && { anchor, path: root && relative(file.path, root), cursor: file.cursor };
		};
		const hide = () => {
			clearTimeout(timer);
			preview = undefined;
		};
		const show = (event: PointerEvent | FocusEvent, delay: number) => {
			const link = linked(event);
			const path = link?.path;
			if (!path || link.anchor.contains(event.relatedTarget as Node)) return;
			clearTimeout(timer);
			timer = setTimeout(() => (preview = { path, line: link.cursor?.line, anchor: link.anchor.getBoundingClientRect() }), delay);
		};
		const leave = (event: PointerEvent | FocusEvent) => {
			const link = linked(event);
			if (link && !link.anchor.contains(event.relatedTarget as Node)) hide();
		};
		const offs = [
			on(panel, 'click', (event) => {
				const link = linked(event);
				if (!link) return;
				event.preventDefault();
				hide();
				if (link.path) onfile(link.path, link.cursor);
			}),
			on(panel, 'pointerover', (event) => event.pointerType === 'mouse' && show(event, 400)),
			on(panel, 'pointerout', leave),
			on(panel, 'focusin', (event) => (event.target as Element).matches(':focus-visible') && show(event, 0)),
			on(panel, 'focusout', leave),
			on(panel, 'scroll', hide, { capture: true })
		];
		return () => {
			hide();
			for (const off of offs) off();
		};
	}

	function follow(scroller: HTMLElement) {
		let pinned = true;
		let top = 0;
		let visible = false;
		// Scroll anchoring nudges a pinned transcript off the bottom when web fonts swap in.
		const pin = (bottom: boolean) => {
			pinned = bottom;
			scroller.style.overflowAnchor = bottom ? 'none' : '';
		};
		pin(true);
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
			if (next < top || bottom) pin(bottom);
			top = scroller.scrollTop;
		});
		return () => {
			off();
			observer.disconnect();
		};
	}
</script>

<!-- svelte-ignore a11y_interactive_supports_focus (Dropped images have the Attach image button as their keyboard path.) -->
<div class="panel" class:phone={form() === 'phone'} role="tabpanel" hidden={!active} ondragover={dragover} ondrop={drop} {@attach links}>
	{#if !selected}
		{#if source || handoffs.length}
			<div class="handoffs" aria-label="Chat handoffs">
				{#if source}<Button onclick={() => onchat(source)}>Source Chat · {source.rig}</Button>{/if}
				{#each handoffs as target (target.id)}<Button onclick={() => onchat(target)}>New Chat · {target.rig}</Button>{/each}
			</div>
		{/if}
		<Todos todos={chat.todos} />
	{:else}
		<SubagentHeader subagent={selected} path={opened} {trail} main={items} onopen={open} />
	{/if}
	<div class="conversation" hidden={!!selected}>
		<div class="transcript" bind:this={scroller} style:padding-top="max(var(--space-6), {form() === 'phone' ? 0 : messageNavHeight}px)" {@attach follow}>
			<div class="column">
				{#if attached && !items.length && !chat.queued.length}<EmptyChat rig={chat.rig} agent={chat.agent} settings={chat.settings} {choices} {project} {branch} />{/if}
				<Transcript {items} {active} answered={attached ? chat.answeredQuestions : []} activity={chat.activity} agent={chat.agent} resets={codexResets} {onhandoff} onopen={open} onattend={attend} {ondiff} {oncanvas} onrewind={(index) => run('rewind', index)} onexpand={(index) => device.send({ type: 'chat.expand', chatId: chat.id, index })} />
				<Queued queued={chat.queued} disabled={!live} oncancel={(index) => device.send({ type: 'chat.unqueue', chatId: chat.id, index, revision: chat.queueRevision })} />
			</div>
		</div>
		<MessageNav {items} {scroller} bind:height={messageNavHeight} />
	</div>
	{#if selected}
		{#key opened.join('/')}
			<div class="transcript" {@attach follow}>
				<div class="column">
					{#if selected.prompt}<Prompt prompt={selected.prompt} />{/if}
					<Transcript items={selected.items} main={items} {active} activity={selected.activity} path={opened} agent={selected.agent ?? chat.agent} resets={codexResets} {onhandoff} onopen={open} onattend={attend} {ondiff} {oncanvas} />
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
		{#if selected}<Activity status={selected.status} activity={selected.activity} />{/if}
		<Tray {items} {canvases} onopen={open} onattend={attend} {oncanvas} />
		{#if request}
			{#key request.id}
				<RequestCard bind:this={card} {request} {agent} {asker} disabled={!live} onanswer={(chosen) => answer(request, chosen)} />
			{/key}
		{/if}
		{#if !selected}
			<ChatComposer bind:this={composer} {device} {chat} {history} {shelling} {shell} {live} {request} {capabilities} {choices} {branches} {focused} onrun={run} />
		{:else}
			<div class="subagent"><span>Subagent</span><Button onclick={() => onopen([])}>Back to Main</Button></div>
		{/if}
	</div>
</div>
{#if preview && active}<FilePreview projectId={chat.projectId} {...preview} />{/if}
<Handoff bind:this={handoff} {device} {chat} available={attached && items.some((item) => item.kind === 'user')} />
<MoveToWorktree bind:this={mover} {device} {chat} />
<Rewind bind:open={rewinding} bind:target={rewindTarget} {chat} {items} {capabilities} unavailable={rewindUnavailable} onrewind={(index) => device.send({ type: 'chat.run', chatId: chat.id, command: 'rewind', value: index })} />

<style>
	.handoffs { display: flex; flex: none; flex-wrap: wrap; align-items: center; gap: var(--space-2); padding: var(--space-2) var(--space-4); border-bottom: var(--border-width) solid var(--border); font-size: var(--font-size-sm); }
	.subagent { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); padding: var(--space-3) var(--space-4); font-size: var(--font-size-sm); }
	.subagent span, .report .label { color: var(--muted-fg); }
	.report { display: flex; flex-direction: column; gap: var(--space-1); padding: var(--space-3) var(--space-4); border: var(--border-width) solid var(--border); border-radius: var(--radius-lg); }
	.report .label { font-size: var(--font-size-sm); }
	.conversation { position: relative; display: flex; flex: 1; min-height: 0; overflow: hidden; }
	.conversation[hidden] { display: none; }
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
		min-width: 0;
		min-height: 0;
		overflow-y: auto;
		padding: var(--space-6) var(--space-6) var(--space-4);
	}

	.column {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		max-width: 760px;
		min-height: 100%;
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
		font-size: var(--font-size-chat);
		line-height: 1.5;
	}
</style>
