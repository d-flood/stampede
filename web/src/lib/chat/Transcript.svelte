<script lang="ts">
	import Command from '#lib/chat/Command.svelte';
	import Edit from '#lib/chat/Edit.svelte';
	import Html from '#lib/chat/Html.svelte';
	import Notice from '#lib/chat/Notice.svelte';
	import Subagent from '#lib/chat/Subagent.svelte';
	import Text from '#lib/chat/Text.svelte';
	import Thinking from '#lib/chat/Thinking.svelte';
	import Tools from '#lib/chat/Tools.svelte';
	import User from '#lib/chat/User.svelte';
	import type { Item } from '#lib/device.svelte.ts';

	let { items, ondiff }: { items: Item[]; ondiff(path: string): void } = $props();

	function interim(item: Item) {
		return !['user', 'html', 'notice'].includes(item.kind) && !(item.kind === 'text' && item.phase === 'final');
	}

	const runs = $derived(
		items.reduce<{ rail: boolean; indices: number[] }[]>((runs, item, index) => {
			const rail = interim(item);
			const last = runs.at(-1);
			if (last && last.rail === rail) last.indices.push(index);
			else runs.push({ rail, indices: [index] });
			return runs;
		}, [])
	);

	function dot(item: Item) {
		if (item.kind === 'text' || item.kind === 'thinking') return 'said';
		if (item.kind === 'command' || item.kind === 'subagent') return item.status;
		return 'done';
	}
</script>

{#snippet entry(item: Item, index: number)}
	{#if item.kind === 'user'}
		<User {item} first={index === 0} />
	{:else if item.kind === 'text'}
		<Text {item} />
	{:else if item.kind === 'thinking'}
		<Thinking {item} />
	{:else if item.kind === 'tools'}
		<Tools {item} />
	{:else if item.kind === 'html'}
		<Html {item} />
	{:else if item.kind === 'edit'}
		<Edit {item} {ondiff} />
	{:else if item.kind === 'command'}
		<Command {item} />
	{:else if item.kind === 'subagent'}
		<Subagent {item} {ondiff} />
	{:else if item.kind === 'notice'}
		<Notice {item} />
	{/if}
{/snippet}

{#each runs as run (run.indices[0])}
	{#if run.rail}
		<div class="rail">
			{#each run.indices as index (index)}
				<div class="step" class:prose={items[index].kind === 'text'} data-dot={dot(items[index])}>{@render entry(items[index], index)}</div>
			{/each}
		</div>
	{:else}
		{#each run.indices as index (index)}
			{@render entry(items[index], index)}
		{/each}
	{/if}
{/each}

<style>
	.rail {
		--indent: var(--space-6);
		--dot: 9px;
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		margin-left: var(--space-1);
		padding-left: var(--indent);
		border-left: var(--border-width) solid var(--border);
	}

	:global(.phone) .rail {
		--indent: var(--space-4);
	}

	.step {
		position: relative;
		min-width: 0;
	}

	.step::before {
		content: '';
		position: absolute;
		top: calc(var(--target-min) / 2 - var(--dot) / 2);
		left: calc(-1 * var(--indent) - var(--border-width) / 2 - var(--dot) / 2);
		box-sizing: border-box;
		width: var(--dot);
		height: var(--dot);
		border: var(--border-width) solid var(--muted-fg);
		border-radius: 50%;
		background: var(--muted-fg);
	}

	.prose::before {
		top: calc(0.5lh - var(--dot) / 2);
	}

	[data-dot='said']::before {
		background: var(--color-bg);
	}

	[data-dot='ok']::before {
		border-color: var(--status-done);
		background: var(--status-done);
	}

	[data-dot='failed']::before {
		border-color: var(--ansi-1);
		background: var(--ansi-1);
	}

	[data-dot='waiting']::before {
		border-color: var(--status-waiting);
		background: var(--status-waiting);
	}

	[data-dot='running']::before,
	[data-dot='working']::before {
		border-color: var(--status-working);
		background: var(--status-working);
		animation: fade 1.6s ease-in-out var(--pulse);
	}

	@keyframes fade {
		50% {
			opacity: 0.3;
		}
	}
</style>
