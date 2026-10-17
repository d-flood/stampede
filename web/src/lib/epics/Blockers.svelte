<script lang="ts">
	import type { Ticket } from '#lib/device.svelte.ts';

	type Props = { tickets: Ticket[]; blockers: number[]; ontoggle(number: number): void };

	let { tickets, blockers, ontoggle }: Props = $props();
</script>

<div role="group" aria-label="Blocked by">
	{#each tickets as ticket (ticket.number)}
		<button type="button" aria-pressed={blockers.includes(ticket.number)} title={ticket.title} onclick={() => ontoggle(ticket.number)}>
			{ticket.number}
		</button>
	{/each}
</div>

<style>
	div {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	button {
		min-width: var(--target-min);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: none;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		cursor: pointer;
	}

	button:hover {
		background: var(--hover);
	}

	button:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	button[aria-pressed='true'] {
		border-color: var(--color-fg);
		background: var(--surface-2);
		color: var(--color-fg);
		font-weight: 600;
	}
</style>
