<script lang="ts">
	import type { Ticket } from '#lib/device.svelte.ts';
	import Chip from '#lib/ui/Chip.svelte';

	type Props = { tickets: Ticket[]; blockers: number[]; ontoggle(number: number): void };

	let { tickets, blockers, ontoggle }: Props = $props();
</script>

<div role="group" aria-label="Blocked by">
	{#each tickets as ticket (ticket.number)}
		<Chip mono pressed={blockers.includes(ticket.number)} title={ticket.title} onclick={() => ontoggle(ticket.number)}>
			{ticket.number}
		</Chip>
	{/each}
</div>

<style>
	div {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
