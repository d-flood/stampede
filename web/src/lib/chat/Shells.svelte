<script lang="ts">
	import Command from '#lib/chat/Command.svelte';
	import type { Item } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import CaretDown from '~icons/ph/caret-down';

	let { items }: { items: Item[] } = $props();

	function running(items: Item[], parent = '', owner = 'Main'): { id: string; owner: string; item: Extract<Item, { kind: 'command' }> }[] {
		return items.flatMap((item, index) => {
			const id = `${parent}/${index}`;
			if (item.kind === 'subagent') return running(item.items, id, item.name || item.task);
			return item.kind === 'command' && item.status === 'running' ? [{ id, owner, item }] : [];
		});
	}

	const shells = $derived(running(items));
	const phone = $derived(form() === 'phone');
</script>

{#snippet list()}
	<div class="shells">
		{#each shells as shell (shell.id)}
			<div class="shell">
				{#if shell.owner !== 'Main'}<span class="owner">{shell.owner}</span>{/if}
				<Command item={shell.item} />
			</div>
		{/each}
	</div>
{/snippet}

{#if shells.length}
	<section aria-label="Running shells" class:phone>
		{#if phone}
			<details>
				<summary><strong>Running shells · {shells.length}</strong><span class="caret"><CaretDown /></span></summary>
				{@render list()}
			</details>
		{:else}
			<strong>Running shells · {shells.length}</strong>
			{@render list()}
		{/if}
	</section>
{/if}

<style>
	section { min-width: 0; border: var(--border-width) solid var(--border); border-radius: var(--radius-lg); padding: var(--space-2) var(--space-3); }
	strong { color: var(--status-working); font-size: var(--font-size-sm); }
	.shells { max-height: 25vh; overflow: auto; }
	.shell { min-width: 0; }
	.owner { color: var(--muted-fg); font-size: var(--font-size-sm); overflow-wrap: anywhere; }
	.phone { padding: 0; overflow: hidden; }
	.phone summary { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); min-height: var(--target-min); padding: 0 var(--space-3); list-style: none; cursor: pointer; }
	.phone summary::-webkit-details-marker { display: none; }
	.phone details[open] > summary { border-bottom: var(--border-width) solid var(--border); background: var(--surface-2); }
	.phone .shells { padding: 0 var(--space-3); }
	.caret { display: inline-flex; flex: none; color: var(--muted-fg); transition: transform var(--dur-fast); }
	details[open] > summary > .caret { transform: rotate(180deg); }
</style>
