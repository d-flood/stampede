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
	import Dialog from '#lib/ui/Dialog.svelte';
	import CaretRight from '~icons/ph/caret-right';

	type Props = {
		epic: Epic;
		ticket: Ticket;
		onedit(): void;
		open: OpenEpic;
		chat?: Chat;
		device: Device;
		onchat(chatId: string): void;
	};

	let { epic, ticket, onedit, open, chat, device, onchat }: Props = $props();

	const phone = $derived(form() === 'phone');
	const authors = { agent: 'Agent', david: 'David', stampede: 'Stampede' };
	const others = $derived(epic.tickets.filter((other) => other.number !== ticket.number));
	const Mark = $derived(marks[ticket.status]);
	const startable = $derived(!epic.run && onFrontier(epic, ticket));
	const reopenable = $derived(resolved(ticket.status) || ticket.status === 'in-progress');
	const edited = $derived(!!ticket.claim?.edited);
	const starting = $derived(open.starting.includes(ticket.number));
	const error = $derived(open.errors[ticket.number]);
	const choices = $derived(rigs());
	const defaultRig = $derived(choices.some((choice) => choice.name === open.rig) ? open.rig : choices[0]?.name);
	const rig = $derived(ticket.rig ?? defaultRig);
	let tagging = $state(false);
	let tagError = $state<string>();
	let refused = $state<string>();
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
	<div class="start">
		<div class="rig">
			<span>Rig</span>
			<RigSelect {open} {ticket} {defaultRig} disabled={starting} />
		</div>
		<Button
			variant="primary"
			disabled={!choices.some((choice) => choice.name === rig) || epic.setup === null || starting || open.savingRigs.includes(ticket.number)}
			onclick={() => rig && open.start(device, ticket.number, rig)}
		>
			{starting ? 'Starting…' : 'Start'}
		</Button>
	</div>
{/snippet}

<div class="ticket" class:phone>
	{#if phone}
		<h1>{ticket.title}</h1>
	{/if}
	{#if phone || edited}
		<div class="chips">
			{#if phone}
				<span class="status" style:--tone={tones[ticket.status]}><Mark />{label(ticket.status)}</span>
				{#if ticket.blockers.length}<span>after {ticket.blockers.join(', ')}</span>{/if}
			{/if}
			{#if edited}<span style:--tone="var(--status-waiting)">edited since its Chat started</span>{/if}
		</div>
	{/if}
	{#if ticket.attention}
		<p class="attention" role="status">{ticket.attention}</p>
	{/if}
	{#if error}
		<p class="attention" role="alert">{error}</p>
	{/if}
	{#if phone && chat}
		<button class="chat" onclick={() => onchat(chat.id)}>
			<span>Chat · <span style:color="var(--status-{chat.status})">{chat.status}</span></span>
			<CaretRight />
		</button>
	{/if}
	<section aria-label="Body">
		<Text item={{ kind: 'text', text: ticket.body, streaming: false, phase: 'final' }} />
	</section>
	<section aria-label="Tags">
		<h2>Tags</h2>
		<Tags tags={ticket.tags ?? []} onchange={tag} disabled={tagging} />
		{#if tagError}<p role="alert">{tagError}</p>{/if}
	</section>
	{#if others.length}
		<section aria-label="Blockers">
			<h2>Blocked by</h2>
			<Blockers tickets={others} blockers={ticket.blockers} ontoggle={block} />
			{#if refused}<p class="error" role="alert">{refused}</p>{/if}
		</section>
	{/if}
	<section aria-label="Notes">
		<h2>Notes</h2>
		{#each ticket.notes as note, index (index)}
			<div class="note">
				<span class="meta">{authors[note.author]} · {ago(note.at)}</span>
				<Text item={{ kind: 'text', text: note.text, streaming: false, phase: 'final' }} />
			</div>
		{:else}
			<p>No Notes yet.</p>
		{/each}
		<form onsubmit={add}>
			<input bind:value={draft} aria-label="Add a Note" placeholder="Add a Note…" enterkeyhint="send" autocomplete="off" readonly={adding} />
		</form>
		{#if unsaved}<p class="error" role="alert">{unsaved}</p>{/if}
	</section>
	{#if !phone}
		<div class="actions">
			<Button onclick={onedit}>Edit</Button>
			{#if chat}<Button onclick={() => onchat(chat.id)}>Open Chat</Button>{/if}
			{#if reopenable}{@render move()}{/if}
			{#if startable}{@render start()}{/if}
		</div>
	{:else if startable}
		{@render start()}
	{:else if reopenable}
		{@render move()}
	{/if}
	{#if unmoved}<p class="error" role="alert">{unmoved}</p>{/if}
	{#if startable && epic.setup === null}
		<p class="hint">Record a Setup Command before starting a Ticket.</p>
	{/if}
</div>

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
		gap: var(--space-4);
		line-height: 1.5;
	}

	h1 {
		margin: 0;
		font-size: var(--font-size-lg);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		font-size: var(--font-size-sm);
	}

	h1 + .chips {
		margin-top: calc(-1 * var(--space-2));
	}

	.chips span {
		padding: 1px var(--space-3);
		border: var(--border-width) solid var(--tone, var(--border));
		border-radius: 1em;
		color: var(--tone, var(--muted-fg));
	}

	.status :global(svg) {
		margin-right: var(--space-1);
		vertical-align: -0.125em;
	}

	.attention {
		padding: var(--space-2) var(--space-3);
		border-left: 2px solid var(--status-waiting);
		background: color-mix(in oklab, var(--status-waiting) 10%, transparent);
		color: var(--color-fg);
		overflow-wrap: anywhere;
	}

	.chat {
		display: flex;
		align-items: center;
		justify-content: space-between;
		min-height: var(--target-min);
		padding: 0 var(--space-4);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: none;
		color: var(--color-fg);
		font: inherit;
	}

	.chat :global(svg) {
		color: var(--muted-fg);
	}

	section[aria-label='Notes'],
	section[aria-label='Tags'],
	section[aria-label='Blockers'] {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	h2 {
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
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

	input {
		box-sizing: border-box;
		width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
		outline: none;
	}

	input:focus-visible {
		border-color: var(--status-working);
	}

	.actions,
	.start {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.actions .start {
		margin-left: auto;
	}

	.start .rig {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--muted-fg);
	}

	.rig > :global(div) {
		flex: 1;
		min-width: 0;
	}

	.hint {
		font-size: var(--font-size-sm);
	}

	.phone {
		font-size: var(--font-size-lg);
	}

	.phone h1 {
		font-size: 1.25em;
	}

	.phone .start {
		flex-direction: column;
		align-items: stretch;
	}

</style>
