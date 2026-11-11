<script lang="ts">
	import { ago } from '#lib/chat/sessions.ts';
	import Text from '#lib/chat/Text.svelte';
	import { rigs } from '#lib/config.svelte.ts';
	import type { Chat, Device, Epic, Ticket, TicketTag } from '#lib/device.svelte.ts';
	import Blockers from '#lib/epics/Blockers.svelte';
	import RigSelect from '#lib/epics/RigSelect.svelte';
	import Tags from '#lib/epics/Tags.svelte';
	import { label, marks, onFrontier, record, resolved, toggle, tones, type OpenEpic } from '#lib/epics.svelte.ts';
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';
	import Chip from '#lib/ui/Chip.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import ChatCircle from '~icons/ph/chat-circle';

	type Props = {
		epic: Epic;
		ticket: Ticket;
		onedit(): void;
		open: OpenEpic;
		chat?: Chat;
		device: Device;
		onchat(chatId: string): void;
		onselect(number: number): void;
	};

	let { epic, ticket, onedit, open, chat, device, onchat, onselect }: Props = $props();

	const phone = $derived(form() === 'phone');
	const authors = { agent: 'Agent', david: 'David', stampede: 'Stampede' };
	const others = $derived(epic.tickets.filter((other) => other.number !== ticket.number));
	const blockedBy = $derived(others.filter((other) => ticket.blockers.includes(other.number)));
	const blocks = $derived(others.filter((other) => other.blockers.includes(ticket.number)));
	const Mark = $derived(marks[ticket.status]);
	const startable = $derived(!epic.run && onFrontier(epic, ticket));
	const reopenable = $derived(resolved(ticket.status) || ticket.status === 'in-progress');
	const edited = $derived(!!ticket.claim?.edited);
	const starting = $derived(open.starting.includes(ticket.number));
	const error = $derived(open.errors[ticket.number]);
	const choices = $derived(rigs());
	const defaultRig = $derived(choices.some((choice) => choice.name === open.rig) ? open.rig : choices[0]?.name);
	const rig = $derived(ticket.rig ?? defaultRig);
	const held = $derived(ticket.status === 'in-progress' ? ticket.claim : undefined);
	const chatStatus = $derived(ticket.releasing ? 'stopping' : (chat?.status ?? 'closed'));
	let tagging = $state(false);
	let tagError = $state<string>();
	let refused = $state<string>();
	let changing = $state(false);
	let draft = $state('');
	let adding = $state(false);
	let unsaved = $state<string>();
	let reopening = $state(false);
	let confirming = $state(false);
	let unmoved = $state<string>();

	async function reopen(confirmed = false) {
		if (ticket.status === 'in-progress' && !confirmed) {
			confirming = true;
			return;
		}
		confirming = false;
		reopening = true;
		unmoved = await record(epic.slug, { kind: 'ticket.reopened', ticket: ticket.number });
		reopening = false;
	}

	async function block(number: number) {
		refused = await record(epic.slug, { kind: 'blockers.set', ticket: ticket.number, blockers: toggle(ticket.blockers, number) });
	}

	async function tag(tags: TicketTag[]) {
		tagging = true;
		tagError = await record(epic.slug, { kind: 'ticket.tags.set', ticket: ticket.number, tags });
		tagging = false;
	}

	async function add(event: SubmitEvent) {
		event.preventDefault();
		if (adding || !draft.trim()) return;
		adding = true;
		unsaved = await record(epic.slug, { kind: 'note.added', ticket: ticket.number, author: 'david', text: draft.trim() });
		adding = false;
		if (!unsaved) draft = '';
	}
</script>

{#snippet move()}
	<Button disabled={reopening || ticket.releasing} onclick={() => reopen()}>Move to not started</Button>
{/snippet}

{#snippet start()}
	<Button
		variant="primary"
		disabled={!choices.some((choice) => choice.name === rig) || epic.setup === null || starting || open.savingRigs.includes(ticket.number)}
		onclick={() => rig && open.start(device, ticket.number, rig)}
	>
		{starting ? 'Starting…' : 'Start'}
	</Button>
{/snippet}

{#snippet openChat(chat: Chat, primary: boolean)}
	<Button variant={primary ? 'primary' : 'default'} onclick={() => onchat(chat.id)}><ChatCircle /> Open Chat</Button>
{/snippet}

{#snippet ref(other: Ticket)}
	{@const Other = marks[other.status]}
	<button class="ref" title={other.title} onclick={() => onselect(other.number)}>
		<span class="mark" style:color={tones[other.status]}><Other /></span>
		<span class="number">{other.number}</span>
		<span class="title">{other.title}</span>
	</button>
{/snippet}

<article class="ticket" class:phone aria-label="{epic.slug}/{ticket.number}">
	{#if !phone}<p class="crumb">{epic.slug}/{ticket.number}</p>{/if}
	<h2>{ticket.title}</h2>
	<div class="state">
		<span class="status" style:--tone={tones[ticket.status]}><Mark />{label(ticket.status)}</span>
		{#if held}
			<span class="holder">{held.rig}, <span style:color={chat ? `var(--status-${chat.status})` : undefined}>{chatStatus}</span></span>
		{/if}
		{#if edited}<span class="edited">edited since its Chat started</span>{/if}
		{#if !phone}
			<span class="actions">
				{#if chat}{@render openChat(chat, chat.status === 'waiting')}{/if}
				<Button onclick={onedit}>Edit</Button>
				{#if reopenable}{@render move()}{/if}
				{#if startable}{@render start()}{/if}
			</span>
		{/if}
	</div>
	{#if ticket.attention}
		<p class="attention" role="status">{ticket.attention}</p>
	{/if}
	{#if error}
		<p class="attention" role="alert">{error}</p>
	{/if}
	{#if unmoved}<p class="error" role="alert">{unmoved}</p>{/if}
	{#if startable && epic.setup === null}
		<p class="hint">Record a Setup Command before starting a Ticket.</p>
	{/if}
	{#if phone && (blockedBy.length || blocks.length)}
		<div class="chips">
			{#each blockedBy as other (other.number)}
				{@const Other = marks[other.status]}
				<Chip onclick={() => onselect(other.number)}><span class="mark" style:color={tones[other.status]}><Other /></span>after {other.number}</Chip>
			{/each}
			{#each blocks as other (other.number)}
				<Chip onclick={() => onselect(other.number)}>blocks {other.number}</Chip>
			{/each}
		</div>
	{/if}
	<div class="columns">
		<div class="doc">
			<section aria-label="Body">
				<Text item={{ kind: 'text', text: ticket.body, streaming: false, phase: 'final' }} />
			</section>
			<section aria-label="Notes">
				<h3>Notes</h3>
				{#each ticket.notes as note, index (index)}
					<div class="note">
						<span class="meta">{authors[note.author]} · {ago(note.at)}</span>
						<Text item={{ kind: 'text', text: note.text, streaming: false, phase: 'final' }} />
					</div>
				{:else}
					<p>No Notes yet.</p>
				{/each}
				<form onsubmit={add}>
					<TextField bind:value={draft} aria-label="Add a Note" placeholder="Add a Note…" enterkeyhint="send" autocomplete="off" readonly={adding} />
				</form>
				{#if unsaved}<p class="error" role="alert">{unsaved}</p>{/if}
			</section>
		</div>
		<div class="props">
			<div class="prop">
				<h3>Rig</h3>
				{#if ticket.status === 'not-started'}
					<RigSelect {open} {ticket} {defaultRig} disabled={starting} />
				{:else}
					<p class="value">{ticket.claim?.rig ?? ticket.rig ?? 'Run default'}</p>
				{/if}
			</div>
			{#if others.length}
				<section class="prop" aria-label="Blockers">
					<div class="heading">
						<h3>Blocked by</h3>
						<Button variant="link" size="sm" aria-expanded={changing} onclick={() => (changing = !changing)}>{changing ? 'Done' : 'Change'}</Button>
					</div>
					{#if changing}
						<Blockers tickets={others} blockers={ticket.blockers} ontoggle={block} />
					{:else}
						{#each blockedBy as other (other.number)}{@render ref(other)}{:else}<p>None</p>{/each}
					{/if}
					{#if refused}<p class="error" role="alert">{refused}</p>{/if}
				</section>
			{/if}
			{#if blocks.length}
				<div class="prop">
					<h3>Blocks</h3>
					{#each blocks as other (other.number)}{@render ref(other)}{/each}
				</div>
			{/if}
			<section class="prop" aria-label="Tags">
				<h3>Tags</h3>
				<Tags tags={ticket.tags ?? []} onchange={tag} disabled={tagging} />
				{#if tagError}<p role="alert">{tagError}</p>{/if}
			</section>
		</div>
	</div>
	{#if phone && (startable || chat || reopenable)}
		<div class="bottom">
			{#if startable}
				{@render start()}
			{:else if chat}
				{@render openChat(chat, true)}
			{/if}
			{#if reopenable}{@render move()}{/if}
		</div>
	{/if}
</article>

<Dialog
	bind:open={confirming}
	title="Move {epic.slug}/{ticket.number} to not started?"
	description="Its Chat stops. Its worktree and Agent session are kept for the next Chat that takes it."
>
	{#snippet actions()}
		<Button onclick={() => (confirming = false)}>Cancel</Button>
		<Button variant="primary" onclick={() => reopen(true)}>Move</Button>
	{/snippet}
</Dialog>

<style>
	.ticket {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		line-height: 1.5;
	}

	.crumb {
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	h2 {
		max-width: 40ch;
		margin: 0;
		font-size: calc(var(--font-size-lg) + 4px);
		line-height: 1.25;
	}

	.crumb + h2 {
		margin-top: calc(-1 * var(--space-2));
	}

	.state {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		padding-bottom: var(--space-4);
		border-bottom: var(--border-width) solid var(--border);
		font-size: var(--font-size-sm);
	}

	.status {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		padding: 1px var(--space-3) 1px var(--space-2);
		border-radius: 1em;
		background: color-mix(in oklab, var(--tone) 15%, transparent);
		color: var(--tone);
		font-weight: 500;
	}

	.holder {
		color: var(--muted-fg);
	}

	.edited {
		padding: 1px var(--space-3);
		border: var(--border-width) solid var(--status-waiting);
		border-radius: 1em;
		color: var(--status-waiting);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin-left: auto;
		font-size: var(--font-size-md);
	}

	.attention {
		padding: var(--space-2) var(--space-3);
		border-left: 2px solid var(--status-waiting);
		background: color-mix(in oklab, var(--status-waiting) 10%, transparent);
		color: var(--color-fg);
		overflow-wrap: anywhere;
	}

	.columns {
		display: grid;
		grid-template-columns: minmax(0, 68ch) minmax(11rem, 15rem);
		justify-content: space-between;
		gap: var(--space-6) calc(2 * var(--space-6));
		padding-top: var(--space-2);
	}

	@container (max-width: 44rem) {
		.columns {
			grid-template-columns: minmax(0, 1fr);
		}
	}

	.doc,
	.props,
	section,
	.prop {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		min-width: 0;
	}

	.doc,
	.props {
		gap: var(--space-6);
	}

	h3 {
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
	}

	.ref {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0;
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		text-align: left;
		cursor: pointer;
	}

	.ref:hover .title {
		text-decoration: underline;
	}

	.ref .title {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.mark {
		display: flex;
		flex: none;
	}

	.number {
		color: var(--muted-fg);
		font-family: var(--font-mono);
	}

	.value {
		color: var(--color-fg);
		font-size: var(--font-size-sm);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.note {
		padding-left: var(--space-3);
		border-left: 2px solid var(--border);
	}

	.meta {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	p {
		margin: 0;
		color: var(--muted-fg);
	}

	.error {
		color: var(--ansi-1);
		font-size: var(--font-size-sm);
	}

	.hint {
		font-size: var(--font-size-sm);
	}

	.ref:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	.bottom {
		position: sticky;
		bottom: calc(-1 * var(--space-3));
		display: flex;
		gap: var(--space-2);
		margin: 0 calc(-1 * var(--space-3)) calc(-1 * var(--space-3));
		padding: var(--space-2) var(--space-3) var(--space-3);
		border-top: var(--border-width) solid var(--border);
		background: var(--color-bg);
	}

	.bottom > :global(button:first-child) {
		flex: 1;
	}

	.phone {
		font-size: var(--font-size-lg);
	}

	.phone h2 {
		font-size: 1.25em;
	}

	.phone .state {
		padding-bottom: var(--space-3);
	}

	.phone .columns {
		grid-template-columns: minmax(0, 1fr);
	}
</style>
