<script lang="ts">
	import { rigs } from '#lib/config.svelte.ts';
	import type { Ticket } from '#lib/device.svelte.ts';
	import { record, type OpenEpic } from '#lib/epics.svelte.ts';

	type Props = { open: OpenEpic; ticket: Ticket; defaultRig?: string; label?: string; disabled?: boolean };

	let { open, ticket, defaultRig, label = 'Rig', disabled = false }: Props = $props();
	const choices = $derived(rigs());
	const selected = $derived(ticket.claim?.rig ?? ticket.rig ?? '');
	const saving = $derived(open.savingRigs.includes(ticket.number));
	let error = $state<string>();

	async function select(event: Event) {
		const target = event.currentTarget as HTMLSelectElement;
		open.savingRigs.push(ticket.number);
		error = await record(open.slug, { kind: 'ticket.rig.set', ticket: ticket.number, rig: target.value || null });
		if (error) target.value = ticket.rig ?? '';
		open.savingRigs = open.savingRigs.filter((number) => number !== ticket.number);
	}
</script>

<div>
	<select
		aria-label={label}
		title={selected || defaultRig}
		value={selected}
		disabled={disabled || saving || ticket.status !== 'not-started'}
		onchange={select}
	>
		<option value="">Use run default</option>
		{#if selected && !choices.some((choice) => choice.name === selected)}
			<option value={selected}>{selected} (unavailable)</option>
		{/if}
		{#each choices as choice (choice.name)}
			<option value={choice.name}>{choice.name}</option>
		{/each}
	</select>
	{#if error}<p role="alert">{error}</p>{/if}
</div>

<style>
	div {
		min-width: 0;
	}

	select {
		box-sizing: border-box;
		width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
	}
	select:focus-visible {
		outline: var(--border-width) solid var(--status-working);
	}

	select:disabled {
		color: var(--muted-fg);
	}

	p {
		margin: var(--space-1) 0 0;
		color: var(--ansi-1);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}
</style>
