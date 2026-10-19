<script lang="ts">
	import type { TicketTag } from '#lib/device.svelte.ts';
	import { ticketTags } from '#lib/epics.svelte.ts';

	type Props = { tags: TicketTag[]; onchange(tags: TicketTag[]): void; disabled?: boolean };

	let { tags, onchange, disabled = false }: Props = $props();
</script>

<div role="group" aria-label="Tags">
	{#each Object.entries(ticketTags) as [value, tag]}
		{@const key = value as TicketTag}
		<button
			type="button"
			{disabled}
			aria-pressed={tags.includes(key)}
			title={tag.description}
			onclick={() => onchange(tags.includes(key) ? tags.filter((tag) => tag !== key) : [...tags, key])}
		>
			{tag.label}
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
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: none;
		color: var(--muted-fg);
		font: inherit;
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
	}
</style>
