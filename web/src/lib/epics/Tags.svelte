<script lang="ts">
	import type { TicketTag } from '#lib/device.svelte.ts';
	import { ticketTags } from '#lib/epics.svelte.ts';
	import Chip from '#lib/ui/Chip.svelte';

	type Props = { tags: TicketTag[]; onchange(tags: TicketTag[]): void; disabled?: boolean };

	let { tags, onchange, disabled = false }: Props = $props();
</script>

<div role="group" aria-label="Tags">
	{#each Object.entries(ticketTags) as [value, tag]}
		{@const key = value as TicketTag}
		<Chip
			{disabled}
			pressed={tags.includes(key)}
			title={tag.description}
			onclick={() => onchange(tags.includes(key) ? tags.filter((tag) => tag !== key) : [...tags, key])}
		>
			{tag.label}
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
