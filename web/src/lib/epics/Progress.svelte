<script lang="ts">
	import type { Epic } from '#lib/device.svelte.ts';
	import { counts, statuses, tones } from '#lib/epics.svelte.ts';

	let { epic }: { epic: Epic } = $props();

	const tally = $derived(counts(epic));
</script>

<span class="bar" aria-hidden="true">
	{#each statuses as status (status)}
		{#if status !== 'not-started' && tally[status]}
			<span style:width="{(tally[status] / epic.tickets.length) * 100}%" style:background={tones[status]}></span>
		{/if}
	{/each}
</span>

<style>
	.bar {
		display: flex;
		height: 4px;
		overflow: hidden;
		border-radius: 2px;
		background: var(--surface-3);
	}
</style>
