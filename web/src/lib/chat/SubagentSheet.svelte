<script lang="ts">
	import { color, type Placed } from '#lib/chat/agents.ts';
	import SubagentRow from '#lib/chat/SubagentRow.svelte';
	import type { Item } from '#lib/device.svelte.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import ListItem from '#lib/ui/ListItem.svelte';
	import ListLabel from '#lib/ui/ListLabel.svelte';

	type Props = {
		title: string;
		groups: { heading?: string; entries: Placed[] }[];
		main: Item[];
		open: boolean;
		onopen(path: string[]): void;
		onattend(subagent: string): void;
	};

	let { title, groups, main, open = $bindable(), onopen, onattend }: Props = $props();
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
			<li><ListItem element="button" onclick={() => choose(entry)}><SubagentRow subagent={entry.subagent} color={color(main, entry.path)} /></ListItem></li>
		{/each}
	</ul>
{/snippet}

<Dialog bind:open full {title} oncloseautofocus={(event) => attending && event.preventDefault()}>
	{#each groups as { heading, entries } (heading)}
		{#if heading}
			<section aria-label={heading}>
				<ListLabel element="h3">{heading}</ListLabel>
				{@render list(entries)}
			</section>
		{:else}
			{@render list(entries)}
		{/if}
	{/each}
</Dialog>

<style>
	.list { margin: 0 0 var(--space-4); padding: 0; list-style: none; }
</style>
