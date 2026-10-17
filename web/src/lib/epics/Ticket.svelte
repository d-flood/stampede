<script lang="ts">
	import { ago } from '#lib/chat/sessions.ts';
	import Text from '#lib/chat/Text.svelte';
	import { rigs } from '#lib/config.svelte.ts';
	import type { Chat, Device, Epic, Ticket } from '#lib/device.svelte.ts';
	import Blockers from '#lib/epics/Blockers.svelte';
	import { label, onFrontier, record, toggle, tones, type OpenEpic } from '#lib/epics.svelte.ts';
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';
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
	const authors = { agent: 'Agent', david: 'David', oikonomos: 'Oikonomos' };
	const others = $derived(epic.tickets.filter((other) => other.number !== ticket.number));
	const startable = $derived(onFrontier(epic, ticket));
	const starting = $derived(open.starting.includes(ticket.number));
	const error = $derived(open.errors[ticket.number]);
	const choices = $derived(rigs());
	const rig = $derived(choices.some((choice) => choice.name === open.rig) ? open.rig : choices[0]?.name);
	let refused = $state<string>();
	let draft = $state('');
	let adding = $state(false);
	let unsaved = $state<string>();

	async function block(number: number) {
		refused = await record(epic.slug, { kind: 'blockers.set', ticket: ticket.number, blockers: toggle(ticket.blockers, number) });
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

{#snippet start()}
	<div class="start">
		<label>
			<span>Rig</span>
			<select value={rig} disabled={starting} onchange={(event) => (open.rig = event.currentTarget.value)}>
				{#each choices as choice (choice.name)}
					<option value={choice.name}>{choice.name}</option>
				{/each}
			</select>
		</label>
		<Button
			variant="primary"
			disabled={!rig || epic.setup === null || starting}
			onclick={() => rig && open.start(device, ticket.number, rig)}
		>
			{starting ? 'Starting…' : 'Start'}
		</Button>
	</div>
{/snippet}

<div class="ticket" class:phone>
	{#if phone}
		<h1>{ticket.title}</h1>
		<div class="chips">
			<span style:--tone={tones[ticket.status]}>{label(ticket.status)}</span>
			{#if ticket.blockers.length}<span>after {ticket.blockers.join(', ')}</span>{/if}
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
			{#if startable}{@render start()}{/if}
		</div>
	{:else if startable}
		{@render start()}
	{/if}
	{#if startable && epic.setup === null}
		<p class="hint">Record a Setup Command before starting a Ticket.</p>
	{/if}
</div>

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
		margin-top: calc(-1 * var(--space-2));
		font-size: var(--font-size-sm);
	}

	.chips span {
		padding: 1px var(--space-3);
		border: var(--border-width) solid var(--tone, var(--border));
		border-radius: 1em;
		color: var(--tone, var(--muted-fg));
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

	.start label {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--muted-fg);
	}

	select {
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
		color: var(--color-fg);
		font: inherit;
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

	.phone .start label select {
		flex: 1;
	}
</style>
