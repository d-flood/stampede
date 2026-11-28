<script lang="ts">
	import { ago, logos } from '#lib/chat/sessions.ts';
	import Transcript from '#lib/chat/Transcript.svelte';
	import type { Device, FoundSession, Item, ServerMessage, Session } from '#lib/device.svelte.ts';
	import { hostOf } from '#lib/host.ts';
	import { sessionKey } from '#lib/shell/session-search.svelte.ts';
	import ListLabel from '#lib/ui/ListLabel.svelte';
	import Shortcut from '#lib/ui/Shortcut.svelte';
	import { onMount, tick, untrack } from 'svelte';

	/** Shows the turn that matched the highlighted Session, read-only, without starting its Agent. */
	let { device, session }: { device: Device; session?: Session | FoundSession } = $props();

	let shown = $state.raw<{ session: Session; items: Item[]; focus: number }>();
	let latest: { id: string; session: Session } | undefined;
	let scroller = $state<HTMLElement>();
	let transcript = $state<Transcript>();
	const Logo = $derived(shown && logos[shown.session.key.agent]);
	const inert = () => {};
	const key = $derived(session && `${sessionKey(session)}:${'item' in session ? session.item : ''}`);

	function request(target: Session | FoundSession) {
		latest = { id: crypto.randomUUID(), session: target };
		device.send({ type: 'sessions.preview', previewId: latest.id, key: target.key, item: 'item' in target ? target.item : undefined });
	}

	$effect(() => {
		if (!key) {
			shown = latest = undefined;
			return;
		}
		const target = untrack(() => session)!;
		const timer = setTimeout(() => request(target), 120);
		return () => clearTimeout(timer);
	});

	onMount(() =>
		device.listen({
			message(message: ServerMessage) {
				if (message.type === 'hello' && latest) request(latest.session);
				if (message.type !== 'sessions.preview' || message.previewId !== latest?.id) return;
				shown = { session: latest.session, items: message.items, focus: message.focus };
				void land(message.focus);
			}
		})
	);

	async function land(focus: number) {
		await tick();
		transcript?.reveal(focus, (node) => {
			if (scroller) scroller.scrollTop += node.getBoundingClientRect().top - scroller.getBoundingClientRect().top - parseFloat(getComputedStyle(scroller).paddingTop);
		});
	}
</script>

<section class="preview" aria-label="Session preview">
	{#if shown}
		<header>
			{#if Logo}<span class="logo" aria-hidden="true"><Logo /></span>{/if}
			<span class="title" title={shown.session.title}>{shown.session.title}</span>
			<span class="time">{ago(shown.session.updatedAt)}</span>
		</header>
		<div class="scroller" bind:this={scroller}>
			{#if shown.items.length}
				<Transcript bind:this={transcript} host={hostOf(shown.session.project.id)} items={shown.items} active={false} agent={shown.session.key.agent} outlined={shown.focus} onopen={inert} onattend={inert} ondiff={inert} oncanvas={inert} />
			{:else}
				<ListLabel element="p">Nothing to preview yet</ListLabel>
			{/if}
		</div>
	{:else if !session}
		<div class="scroller"><ListLabel element="p">Highlight a session to preview it</ListLabel></div>
	{/if}
	<footer aria-hidden="true"><span><Shortcut keys="↵" /> Open</span><span><Shortcut keys="↑" /><Shortcut keys="↓" /> Browse</span></footer>
</section>

<style>
	.preview {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		background: var(--plane);
	}

	header {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-4);
		border-bottom: var(--border-width) solid var(--border);
	}

	.logo {
		display: inline-flex;
		flex: none;
	}

	.title {
		min-width: 0;
		overflow: hidden;
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.time {
		flex: none;
		margin-left: auto;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.scroller {
		--font-size-chat: var(--font-size-chat-sm);
		flex: 1;
		min-height: 0;
		padding: var(--space-4) var(--space-6) var(--space-6);
		overflow-y: auto;
		line-height: 1.55;
	}

	footer {
		display: flex;
		flex: none;
		gap: var(--space-4);
		margin-top: auto;
		padding: var(--space-2) var(--space-4);
		border-top: var(--border-width) solid var(--border);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	footer span {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}
</style>
