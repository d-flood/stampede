<script lang="ts">
	import Command from '#lib/chat/Command.svelte';
	import Calls from '#lib/chat/Calls.svelte';
	import Edit from '#lib/chat/Edit.svelte';
	import { icon } from '#lib/chat/icons.ts';
	import Html from '#lib/chat/Html.svelte';
	import Notice from '#lib/chat/Notice.svelte';
	import Spawn from '#lib/chat/Spawn.svelte';
	import Text from '#lib/chat/Text.svelte';
	import Thinking from '#lib/chat/Thinking.svelte';
	import Tools from '#lib/chat/Tools.svelte';
	import User from '#lib/chat/User.svelte';
	import Questions from '#lib/chat/Questions.svelte';
	import type { AnsweredQuestions, Item, Subagent } from '#lib/device.svelte.ts';

	type Entry = Item | { kind: 'questions'; record: AnsweredQuestions };
	let { items, answered = [], activity, path = [], onopen, ondiff }: { items: Item[]; answered?: AnsweredQuestions[]; activity?: string | null; path?: string[]; onopen(path: string[]): void; ondiff(path: string): void } = $props();
	const entries = $derived.by(() => {
		const records = (after: number): Entry[] => answered.filter((record) => Math.min(record.after, items.length) === after).map((record) => ({ kind: 'questions', record }));
		return [...records(0), ...items.flatMap((item, index): Entry[] => [item, ...records(index + 1)])];
	});

	function interim(item: Entry) {
		return !['user', 'text', 'html', 'notice', 'questions'].includes(item.kind);
	}

	const runs = $derived(
		entries.reduce<{ kind: 'spawn' | 'rail' | 'plain'; indices: number[] }[]>((runs, item, index) => {
			const kind = item.kind === 'subagent' ? 'spawn' : interim(item) ? 'rail' : 'plain';
			const last = runs.at(-1);
			if (last?.kind === kind) last.indices.push(index);
			else runs.push({ kind, indices: [index] });
			return runs;
		}, [])
	);

	function dot(item: Entry) {
		if (item.kind === 'text' || item.kind === 'thinking') return 'said';
		if (item.kind === 'command') return item.status;
		return 'done';
	}

	function marker(item: Entry) {
		return item.kind === 'questions' ? undefined : icon(item);
	}
</script>

{#snippet entry(item: Entry, index: number)}
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
	{:else if item.kind === 'notice'}
		<Notice {item} />
	{:else if item.kind === 'questions'}
		<Questions record={item.record} />
	{/if}
{/snippet}

{#each runs as run (run.indices[0])}
	{#if run.kind === 'spawn'}
		<Spawn subagents={run.indices.map((index) => entries[index]).filter((item): item is Subagent => item.kind === 'subagent')} {path} {onopen} />
	{:else if run.kind === 'rail'}
		<Calls items={run.indices.map((index) => entries[index]).filter((item): item is Item => item.kind !== 'questions')} activity={run === runs.at(-1) ? activity : null}>
			<div class="rail">
				{#each run.indices as index (index)}
					{@const Marker = marker(entries[index])}
					<div class="step" data-dot={dot(entries[index])}>
						{#if Marker}<span class="marker"><Marker /></span>{/if}
						{@render entry(entries[index], index)}
					</div>
				{/each}
			</div>
		</Calls>
	{:else}
		{#each run.indices as index (index)}
			{@render entry(entries[index], index)}
		{/each}
	{/if}
{/each}

<style>
	.rail {
		--indent: var(--space-6);
		--dot: 9px;
		--marker: 14px;
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

	[data-dot='said']::before {
		content: '';
		position: absolute;
		top: calc(var(--target-min) / 2 - var(--dot) / 2);
		left: calc(-1 * var(--indent) - var(--border-width) / 2 - var(--dot) / 2);
		box-sizing: border-box;
		width: var(--dot);
		height: var(--dot);
		border: var(--border-width) solid var(--muted-fg);
		border-radius: 50%;
		background: var(--color-bg);
	}

	.marker {
		position: absolute;
		top: calc(var(--target-min) / 2 - var(--marker) / 2);
		left: calc(-1 * var(--indent) - var(--border-width) / 2 - var(--marker) / 2);
		display: flex;
		width: var(--marker);
		height: var(--marker);
		background: var(--color-bg);
		color: var(--muted-fg);
	}

	.marker :global(svg) {
		width: 100%;
		height: 100%;
	}

	[data-dot='ok'] > .marker {
		color: var(--status-done);
	}

	[data-dot='failed'] > .marker {
		color: var(--ansi-1);
	}

	[data-dot='waiting'] > .marker {
		color: var(--status-waiting);
	}

	[data-dot='running'] > .marker,
	[data-dot='working'] > .marker {
		color: var(--status-working);
		animation: fade 1.6s ease-in-out var(--pulse);
	}

	@keyframes fade {
		50% {
			opacity: 0.3;
		}
	}
</style>
