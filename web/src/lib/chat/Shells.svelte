<script lang="ts">
	import Command from '#lib/chat/Command.svelte';
	import type { Item } from '#lib/device.svelte.ts';

	let { items }: { items: Item[] } = $props();

	function running(items: Item[], parent = '', owner = 'Main'): { id: string; owner: string; item: Extract<Item, { kind: 'command' }> }[] {
		return items.flatMap((item, index) => {
			const id = `${parent}/${index}`;
			if (item.kind === 'subagent') return running(item.items, id, item.name || item.task);
			return item.kind === 'command' && item.status === 'running' ? [{ id, owner, item }] : [];
		});
	}

	const shells = $derived(running(items));
</script>

{#if shells.length}
	<section aria-label="Running shells">
		<strong>Running shells · {shells.length}</strong>
		<div class="shells">
			{#each shells as shell (shell.id)}
				<div class="shell">
					{#if shell.owner !== 'Main'}<span class="owner">{shell.owner}</span>{/if}
					<Command item={shell.item} />
				</div>
			{/each}
		</div>
	</section>
{/if}

<style>
	section { min-width: 0; border: var(--border-width) solid var(--border); border-radius: var(--radius-lg); padding: var(--space-2) var(--space-3); }
	strong { color: var(--status-working); font-size: var(--font-size-sm); }
	.shells { max-height: 25vh; overflow: auto; }
	.shell { min-width: 0; }
	.owner { color: var(--muted-fg); font-size: var(--font-size-sm); overflow-wrap: anywhere; }
</style>
