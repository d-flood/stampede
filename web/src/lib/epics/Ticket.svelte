<script lang="ts">
	import { ago } from '#lib/chat/sessions.ts';
	import Text from '#lib/chat/Text.svelte';
	import type { Epic, Ticket } from '#lib/device.svelte.ts';
	import Blockers from '#lib/epics/Blockers.svelte';
	import { label, record, toggle, tones } from '#lib/epics.svelte.ts';
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';

	let { epic, ticket, onedit }: { epic: Epic; ticket: Ticket; onedit(): void } = $props();

	const phone = $derived(form() === 'phone');
	const authors = { agent: 'Agent', david: 'David' };
	const others = $derived(epic.tickets.filter((other) => other.number !== ticket.number));
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

<div class="ticket" class:phone>
	{#if phone}
		<h1>{ticket.title}</h1>
		<div class="chips">
			<span style:--tone={tones[ticket.status]}>{label(ticket.status)}</span>
			{#if ticket.blockers.length}<span>after {ticket.blockers.join(', ')}</span>{/if}
		</div>
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
		<div class="actions"><Button onclick={onedit}>Edit</Button></div>
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

	.actions {
		display: flex;
		gap: var(--space-2);
	}

	.phone {
		font-size: var(--font-size-lg);
	}

	.phone h1 {
		font-size: 1.25em;
	}
</style>
