<script lang="ts">
	import ChatComposer from '#lib/chat/ChatComposer.svelte';
	import EmptyChat from '#lib/chat/EmptyChat.svelte';
	import FilePreview from '#lib/chat/FilePreview.svelte';
	import Activity from '#lib/chat/Activity.svelte';
	import { same, subagents as flatten } from '#lib/chat/agents.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
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
	import { mark, matcher, says } from '#lib/chat/landing.ts';
	import { listOptions, type Options } from '#lib/chat/options.ts';
	import MoveToWorktree from '#lib/chat/MoveToWorktree.svelte';
	import Prompt from '#lib/chat/Prompt.svelte';
	import SubagentHeader from '#lib/chat/SubagentHeader.svelte';
	import Text from '#lib/chat/Text.svelte';
	import Transcript from '#lib/chat/Transcript.svelte';
	import { logos } from '#lib/chat/sessions.ts';
	import { agentNames, modes, rigs as configured, runsOn } from '#lib/config.svelte.ts';
	import type { Answer, Builtin, Capabilities, Chat, Choice, Device, Ended, Image, Item, Request, ResetCredits, ServerMessage, Setting, Settings } from '#lib/device.svelte.ts';
	import type { Cursor } from '#lib/files.svelte.ts';
	import { hostOf } from '#lib/host.ts';
	import { form } from '#lib/form.ts';
	import { fileLink, relative } from '#lib/links.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import { onMount, tick, untrack } from 'svelte';
	import { on } from 'svelte/events';
	import { prefersReducedMotion } from 'svelte/motion';
	import { fade } from 'svelte/transition';
	import ArrowDown from '~icons/ph/arrow-down';
	import GitBranch from '~icons/ph/git-branch';
	import GitFork from '~icons/ph/git-fork';

	let { device, chat, chats, movable = true, onchat, items, saved, opened, onopen, active, focused = false, project, root, branch, changes = 0, onchanges, codexResets = null, ondiff, onfile, oncanvas, scroller = $bindable() }: { device: Device; chat: Chat; chats: Chat[]; movable?: boolean; onchat(chat: Chat): void; items: Item[]; saved: string[]; opened: string[]; onopen(path: string[]): void; active: boolean; focused?: boolean; project?: string; root?: string; branch?: string; changes?: number; onchanges?(): void; codexResets?: ResetCredits | null; ondiff(path: string): void; onfile(path: string, cursor?: Cursor): void; oncanvas(canvas: string): void; scroller?: HTMLElement | null } =
		$props();

	const causes: Record<Ended, string> = {
		aged: "Claude Code deleted this Session's history for age",
		cloud: 'Its Cloud Host was destroyed',
		missing: "Its Agent no longer has this Session's history"
	};
	const host = $derived(hostOf(chat.projectId));
	const subagents = $derived(flatten(items));
	const canvases = $derived([...new Set([...written(items), ...saved])]);
	const shell = $derived(chat.background ? commands(items).findLast(({ item }) => item.status === 'running')?.item.command : undefined);
	const selected = $derived(subagents.find(({ path }) => same(path, opened))?.subagent);
	const trail = $derived(subagents.filter(({ path }) => path.length < opened.length && same(path, opened.slice(0, path.length))));
	const source = $derived(chats.find(({ id }) => id === (chat.handoffFrom ?? chat.forkedFrom)));
	const handoffs = $derived(chats.filter(({ handoffFrom }) => handoffFrom === chat.id));
	const forks = $derived(chats.filter(({ forkedFrom }) => forkedFrom === chat.id));
	let ended = $state(true);
	let forkError = $state('');
	let forkFailed = $state(false);
	let failedTitle = $state('Could not fork');
	let requests = $state<Request[]>([]);
	let attached = $state(false);
	let attending = $state<string>();
	const request = $derived(requests.find(({ subagent }) => subagent && subagent === attending) ?? requests[0]);
	const asker = $derived(request?.subagent && subagents.find(({ subagent }) => subagent.id === request.subagent)?.subagent.task);
	let card = $state<RequestCard>();
	let composer = $state<ChatComposer>();
	let transcript = $state<Transcript>();

	export function reveal(item: number, land: (node: HTMLElement) => void) {
		transcript?.reveal(item, land);
	}

	let landed = false;
	let unmark: (() => void) | undefined;
	let pinMain: ((bottom: boolean) => void) | undefined;

	$effect(() => {
		const landing = chat.landing;
		if (landed || !chat.dormant || landing?.item === undefined || !active || !attached || !items.length || !scroller) return;
		landed = true;
		untrack(() => land(landing.item!, landing.query));
	});

	$effect(() => {
		if (!chat.dormant) unmark?.();
	});

	$effect(() => () => unmark?.());

	/** Scrolls to the message a search found and highlights its terms, or to the first message that still has them. */
	function land(item: number, query?: string) {
		const pattern = query ? matcher(query) : undefined;
		const fallback = pattern && !says(items[item], pattern) ? items.findIndex((entry) => says(entry, pattern)) : -1;
		const target = fallback >= 0 ? fallback : item;
		if (target >= items.length) return;
		transcript?.reveal(target, (node) => {
			if (!scroller) return;
			const marked = pattern ? mark(node, pattern) : undefined;
			marked?.ranges[0]?.startContainer.parentElement?.closest('details')?.setAttribute('open', '');
			const view = scroller.getBoundingClientRect();
			const box = node.getBoundingClientRect();
			const first = marked?.ranges[0]?.getBoundingClientRect();
			pinMain?.(false);
			const anchor = box.height > view.height * 0.6 && first ? first.top - view.height / 3 : box.top - view.height / 6;
			scroller.scrollTop += anchor - view.top;
			if (!marked) return;
			const observer = new IntersectionObserver(([entry]) => !entry.isIntersecting && scroller?.getClientRects().length && unmark?.(), { root: scroller });
			observer.observe(node);
			unmark = () => {
				observer.disconnect();
				marked.clear();
				unmark = undefined;
			};
		});
	}
	let handoff = $state<Handoff>();
	let mover = $state<MoveToWorktree>();
	const onhandoff = $derived(handoff?.enabled() ? () => handoff?.show() : undefined);
	const branches = $derived([
		{ label: 'New Chat from here…', icon: GitFork, disabled: !handoff?.enabled(), onselect: () => handoff?.show() },
		...(movable ? [{ label: 'Move to new worktree…', icon: GitBranch, disabled: !mover?.enabled(), onselect: () => mover?.show() }] : [])
	]);
	const history = $derived([...new Set(items.flatMap((item) => (item.kind === 'user' && item.text ? [item.text] : item.kind === 'shell' ? [`!${item.command}`] : [])).reverse())].reverse());
	const shelling = $derived(items.some((item) => item.kind === 'shell' && item.status === 'running'));
	const live = $derived(device.connection === 'connected');
	const agent = $derived(chat.agent === 'claude-code' ? 'Claude' : agentNames[chat.agent]);
	let rewinding = $state(false);
	let preview = $state<{ path: string; line?: number; anchor: DOMRect }>();
	let rewindTarget = $state<number>();
	const rewindUnavailable = $derived.by(() => {
		if (!live) return 'Reconnect to rewind.';
		if (!attached) return 'Loading the conversation…';
		if (chat.queued.length) return 'Cancel queued messages before rewinding.';
		if (chat.status !== 'done') return 'Wait for the current turn to finish, or interrupt first.';
		if (shelling) return 'Wait for running commands to finish, or interrupt first.';
		if (subagents.some(({ subagent }) => subagent.status !== 'done')) return 'Wait for subagents to finish, or interrupt first.';
	});
	let capabilities = $state<Capabilities>({ ready: false, models: [], efforts: [], speeds: [], ultracode: false, modes: [], commands: [], rewindsFiles: false, forks: false, rewinds: false, compacts: false });
	const resumable = $derived(!!chat.dormant && !chat.ended && !chat.gone);
	const continuable = $derived(!!chat.dormant && (!!chat.ended || !!chat.gone));
	const targets = $derived(configured().filter(({ agent }) => runsOn(host, agent)));
	let chosenTarget = $state<string>();
	const target = $derived(targets.find(({ name }) => name === chosenTarget) ?? targets.find(({ agent }) => agent === chat.agent) ?? targets[0]);
	const picker = $derived(
		continuable
			? { rigs: targets.map(({ name, agent }) => ({ label: name, icon: logos[agent] })), current: target ? targets.indexOf(target) : -1, onpick: (index: number) => (chosenTarget = targets[index].name) }
			: resumable && chat.rigs
				? { rigs: chat.rigs.map((label) => ({ label, icon: logos[chat.agent] })), current: chat.rigs.indexOf(chat.rig), onpick: (index: number) => device.send({ type: 'chat.rig', chatId: chat.id, rig: chat.rigs![index] }) }
				: undefined
	);

	function continueWith(text: string, images: Image[], index?: number) {
		if (target) device.send({ type: 'chat.continue', chatId: chat.id, rig: target.name, index, text, images });
	}
	let listing = $state.raw<{ rig: string; options: Options }>();
	const options = $derived(listing?.rig === chat.rig ? listing.options : undefined);
	const settings = $derived<Settings>(resumable ? { ...chat.settings, model: chat.settings.model ?? options?.settings?.model ?? 'default', effort: chat.settings.effort ?? options?.settings?.effort } : chat.settings);
	const choices = $derived<Record<Setting, Choice[]>>(
		resumable
			? {
					model: options?.models.map(({ value, label }) => ({ value, label })) ?? [{ value: 'default', label: 'Default model' }],
					effort: (options?.models.find(({ value }) => value === settings.model)?.efforts ?? []).map((value) => ({ value, label: value })),
					speed: [],
					ultracode: [],
					mode: modes(chat.agent).filter(({ available }) => available).map(({ value, label }) => ({ value, label }))
				}
			: {
					model: capabilities.models.map((value) => ({ value, label: value })),
					effort: capabilities.efforts.map((value) => ({ value, label: value })),
					speed: capabilities.speeds,
					ultracode: capabilities.ultracode ? ['off', 'on'].map((value) => ({ value, label: value })) : [],
					mode: capabilities.modes.filter((choice) => !modes(chat.agent).some((mode) => mode.value === choice.value && !mode.available))
				}
	);

	function reach() {
		if (!resumable || options) return;
		const rig = chat.rig;
		listOptions(chat.agent, rig).then((options) => (listing = { rig, options }), () => {});
	}

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
			case 'chat.fork.error':
			case 'chat.continue.error':
				if (message.chatId !== chat.id) break;
				failedTitle = message.type === 'chat.fork.error' ? 'Could not fork' : 'Could not continue';
				forkError = message.error;
				forkFailed = true;
				break;
		}
	}

	function answer(request: Request, answer: Answer) {
		const images = 'responses' in answer && !('images' in answer) ? composer?.take() : undefined;
		device.send({ type: 'chat.answer', chatId: chat.id, requestId: request.id, answer: images?.length ? { ...answer, images } : answer });
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
		if (resumable && command === 'model' && value === 'default') return;
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
			return file && { anchor, path: root && relative(file.path, root), cursor: file.cursor, canvas: canvases.find((canvas) => file.path.endsWith(`/canvases/${canvas}`)) };
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
				if (link.canvas) oncanvas(link.canvas);
				else if (link.path) onfile(link.path, link.cursor);
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

	function follow(scroller: HTMLElement, onpin?: (bottom: boolean) => void) {
		let pinned = true;
		let top = 0;
		let visible = false;
		// Scroll anchoring nudges a pinned transcript off the bottom when web fonts swap in.
		const pin = (bottom: boolean) => {
			pinned = bottom;
			scroller.style.overflowAnchor = bottom ? 'none' : '';
			onpin?.(bottom);
		};
		pin(true);
		if (onpin) pinMain = pin;
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
<div class="panel" class:phone={form() === 'phone'} class:fresh={form() === 'desktop' && !selected && attached && !items.length && !chat.queued.length} role="tabpanel" hidden={!active} ondragover={dragover} ondrop={drop} {@attach links}>
	{#if !selected}
		{#if source || handoffs.length || forks.length}
			<div class="handoffs" aria-label="Related Chats">
				{#if source}<Button onclick={() => onchat(source)}>Source Chat · {source.rig}</Button>{/if}
				{#each handoffs as target (target.id)}<Button onclick={() => onchat(target)}>New Chat · {target.rig}</Button>{/each}
				{#each forks as target (target.id)}<Button onclick={() => onchat(target)}>Fork · {target.rig}</Button>{/each}
			</div>
		{/if}
		<Todos todos={chat.todos} />
	{:else}
		<SubagentHeader subagent={selected} path={opened} {trail} main={items} onopen={open} />
	{/if}
	<div class="conversation" hidden={!!selected}>
		<div class="transcript" bind:this={scroller} {@attach (node) => follow(node, (bottom) => (ended = bottom))}>
			<div class="column">
				{#if attached && !items.length && !chat.queued.length}<EmptyChat rig={chat.rig} agent={chat.agent} {project} {branch} />{/if}
				<Transcript bind:this={transcript} {host} {items} continued={chat.continuedFrom} active={active && !selected} answered={attached ? chat.answeredQuestions : []} activity={chat.activity} agent={chat.agent} resets={codexResets} {onhandoff} onopen={open} onattend={attend} {ondiff} {oncanvas} onrewind={capabilities.rewinds ? (index) => run('rewind', index) : undefined} onfork={live && attached && continuable && target ? (index) => continueWith('', [], index) : live && attached && capabilities.forks ? (index) => device.send({ type: 'chat.fork', chatId: chat.id, index }) : undefined} onexpand={chat.dormant ? undefined : (index) => device.send({ type: 'chat.expand', chatId: chat.id, index })} />
				<Queued queued={chat.queued} disabled={!live} oncancel={(index) => device.send({ type: 'chat.unqueue', chatId: chat.id, index, revision: chat.queueRevision })} />
			</div>
		</div>
		{#if form() === 'phone'}<MessageNav {items} {scroller} {reveal} />{/if}
	</div>
	{#if selected}
		{#key opened.join('/')}
			<div class="transcript" {@attach follow}>
				<div class="column">
					{#if selected.prompt}<Prompt prompt={selected.prompt} />{/if}
					<Transcript {host} items={selected.items} main={items} {active} activity={selected.activity} path={opened} agent={selected.agent ?? chat.agent} resets={codexResets} {onhandoff} onopen={open} onattend={attend} {ondiff} {oncanvas} />
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
		{#if !ended && !selected}
			<div class="latest" transition:fade={{ duration: 150 }}>
				<Button round aria-label="Jump to latest" tip="Jump to latest" onclick={() => scroller?.scrollTo({ top: scroller.scrollHeight, behavior: prefersReducedMotion.current ? 'auto' : 'smooth' })}><ArrowDown /></Button>
			</div>
		{/if}
		{#if selected}<Activity status={selected.status} activity={selected.activity} />{/if}
		<Tray {items} todos={selected ? [] : chat.todos} {canvases} changes={selected || chat.status !== 'done' || form() === 'phone' ? 0 : changes} {branch} {onchanges} onopen={open} onattend={attend} {oncanvas} />
		{#if request}
			{#key request.id}
				<RequestCard {host} bind:this={card} {request} {agent} {asker} disabled={!live} onanswer={(chosen) => answer(request, chosen)} onattach={(files) => composer?.attach(files)} />
			{/key}
		{/if}
		{#if !selected}
			{#if continuable}
				<div class="ended">
					<p role="status">{chat.ended ? causes[chat.ended] : "This Session's worktree no longer exists"}</p>
					{#if chat.ended && chat.agentSession && chat.sessionStore !== undefined}
						{@const key = { agent: chat.agent, store: chat.sessionStore, sessionId: chat.agentSession }}
						<Button size="sm" onclick={() => device.send({ type: 'sessions.dismiss', key })}>Dismiss</Button>
					{/if}
				</div>
			{/if}
			<ChatComposer bind:this={composer} {device} {chat} {history} {shelling} {shell} {live} {request} {capabilities} {settings} ready={resumable ? !!options : capabilities.ready} {choices} {branches} {focused} rigPicker={picker} onreach={reach} onrun={run} oncontinue={continuable && target ? continueWith : undefined} />
		{:else}
			<div class="subagent"><span>Subagent</span><Button onclick={() => onopen([])}>Back to Main</Button></div>
		{/if}
	</div>
</div>
{#if preview && active}<FilePreview projectId={chat.projectId} {...preview} />{/if}
<Handoff bind:this={handoff} {device} {chat} available={attached && !chat.dormant && items.some((item) => item.kind === 'user')} />
{#if movable}<MoveToWorktree bind:this={mover} {device} {chat} />{/if}
<Dialog bind:open={forkFailed} title={failedTitle}>
	<p class="failure" role="alert">{forkError}</p>
	{#snippet actions()}<Button onclick={() => (forkFailed = false)}>Close</Button>{/snippet}
</Dialog>
<Rewind bind:open={rewinding} bind:target={rewindTarget} {chat} {items} {capabilities} unavailable={rewindUnavailable} onrewind={(index) => device.send({ type: 'chat.run', chatId: chat.id, command: 'rewind', value: index })} />

<style>
	.handoffs { display: flex; flex: none; flex-wrap: wrap; align-items: center; gap: var(--space-2); padding: var(--space-2) var(--space-4); border-bottom: var(--border-width) solid var(--border); font-size: var(--font-size-sm); }
	.subagent { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); padding: var(--space-3) var(--space-4); font-size: var(--font-size-sm); }
	.subagent span, .report .label, .failure { color: var(--muted-fg); }
	.ended { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--space-2) var(--space-3); padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4); border: var(--border-width) solid var(--border); border-radius: var(--radius-lg); color: var(--muted-fg); font-size: var(--font-size-sm); }
	.ended p { margin: 0; }
	.failure { margin: 0; overflow-wrap: anywhere; }
	.report { display: flex; flex-direction: column; gap: var(--space-1); padding: var(--space-3) var(--space-4); border: var(--border-width) solid var(--border); border-radius: var(--radius-lg); }
	.report .label { font-size: var(--font-size-sm); }
	.conversation { position: relative; display: flex; flex: 1; min-height: 0; overflow: hidden; }
	.conversation::before, .conversation::after { content: ''; position: absolute; z-index: 1; right: 0; left: 0; height: calc(4 * var(--space-2)); pointer-events: none; }
	.conversation::before { top: 0; height: calc(1.55 * var(--font-size-chat)); background: linear-gradient(var(--color-bg), transparent); }
	.phone .conversation::before { height: calc(4 * var(--space-2)); }
	.conversation::after { bottom: 0; background: linear-gradient(transparent, var(--color-bg)); }
	.conversation[hidden] { display: none; }
	.transcript :global(::highlight(landing)) { background-color: var(--match-bg); }
	.latest { position: absolute; z-index: 2; right: 0; bottom: calc(100% + var(--space-2)); border-radius: var(--radius-pill); box-shadow: var(--shadow-float); }
	.panel {
		--chat-gutter: calc(var(--space-6) + var(--space-3));
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.panel[hidden] {
		display: none;
	}

	/* A Chat with nothing in it yet gathers its welcome and Composer at the middle. */
	.fresh {
		justify-content: center;
		padding-bottom: calc(4 * var(--space-6) + 12vh);
	}

	.fresh .conversation {
		flex: none;
	}

	.transcript {
		flex: 1;
		min-width: 0;
		min-height: 0;
		overflow-y: auto;
		padding: var(--space-6) var(--chat-gutter) calc(2 * var(--space-6));
	}

	.column {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		max-width: var(--measure);
		min-height: 100%;
		margin: 0 auto;
		line-height: 1.55;
	}

	.foot {
		position: relative;
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-2);
		width: min(var(--measure), 100% - 2 * var(--chat-gutter));
		margin: 0 auto var(--space-4);
	}

	.phone .transcript {
		padding: var(--space-6) var(--space-4) calc(2 * var(--space-6));
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
