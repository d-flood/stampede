<script lang="ts">
	import type { Placed } from '#lib/chat/agents.ts';
	import SubagentRow from '#lib/chat/SubagentRow.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';

	type Props = {
		title: string;
		groups: { heading?: string; entries: Placed[] }[];
		open: boolean;
		onopen(path: string[]): void;
		onattend(subagent: string): void;
	};

	let { title, groups, open = $bindable(), onopen, onattend }: Props = $props();
	let attending = false;

	function choose({ path, subagent }: Placed) {
		attending = subagent.status === 'waiting';
		open = false;
		if (attending) onattend(subagent.id);
		else onopen(path);
	}
</script>

{#snippet list(entries: Placed[])}
	<ul class="list">
		{#each entries as entry (entry.path.join('/'))}
			<li><button class="item" onclick={() => choose(entry)}><SubagentRow subagent={entry.subagent} /></button></li>
		{/each}
	</ul>
{/snippet}

<Dialog bind:open full {title} oncloseautofocus={(event) => attending && event.preventDefault()}>
	{#each groups as { heading, entries } (heading)}
		{#if heading}
			<section aria-label={heading}>
				<h3 class="heading">{heading}</h3>
				{@render list(entries)}
			</section>
		{:else}
			{@render list(entries)}
		{/if}
	{/each}
</Dialog>

<style>
	.list { margin: 0 0 var(--space-4); padding: 0; list-style: none; }
	.heading { display: flex; align-items: center; min-height: var(--target-min); margin: 0; padding: 0 var(--space-2); color: var(--muted-fg); font-size: var(--font-size-sm); font-weight: normal; }
	.item { display: flex; box-sizing: border-box; align-items: center; width: 100%; min-height: var(--target-min); padding: var(--space-1) var(--space-2); border: none; border-radius: var(--radius-sm); background: none; color: inherit; font: inherit; text-align: start; cursor: pointer; }
	.item:hover { background: var(--hover); }
</style>
