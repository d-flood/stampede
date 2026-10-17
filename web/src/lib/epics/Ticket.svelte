<script lang="ts">
	import { ago } from '#lib/chat/sessions.ts';
	import Text from '#lib/chat/Text.svelte';
	import type { Ticket } from '#lib/device.svelte.ts';
	import { label, tones } from '#lib/epics.svelte.ts';
	import { form } from '#lib/form.ts';

	let { ticket }: { ticket: Ticket } = $props();

	const phone = $derived(form() === 'phone');
	const authors = { agent: 'Agent', david: 'David' };
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
	</section>
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

	section[aria-label='Notes'] {
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

	.phone {
		font-size: var(--font-size-lg);
	}

	.phone h1 {
		font-size: 1.25em;
	}
</style>
