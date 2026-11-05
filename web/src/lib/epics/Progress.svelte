<script lang="ts">
	import type { Epic } from '#lib/device.svelte.ts';
	import { counts, statuses, tones } from '#lib/epics.svelte.ts';

	let { epic, segmented = false }: { epic: Epic; segmented?: boolean } = $props();

	const tally = $derived(counts(epic));
	const ordered = $derived(statuses.flatMap((status) => epic.tickets.filter((ticket) => ticket.status === status)));
</script>

<span class="bar" class:segmented aria-hidden="true">
	{#if segmented}
		{#each ordered as ticket (ticket.number)}
			<span style:background={ticket.status === 'not-started' ? undefined : tones[ticket.status]}></span>
		{/each}
	{:else}
		{#each statuses as status (status)}
			{#if status !== 'not-started' && tally[status]}
				<span style:width="{(tally[status] / epic.tickets.length) * 100}%" style:background={tones[status]}></span>
			{/if}
		{/each}
	{/if}
</span>

<style>
	.bar {
		display: flex;
		height: 4px;
		overflow: hidden;
		border-radius: 2px;
		background: var(--border);
	}

	.segmented {
		gap: 2px;
		height: 6px;
		border-radius: 0;
		background: none;
	}

	.segmented span {
		flex: 1;
		border-radius: 2px;
		background: var(--border);
	}
</style>
