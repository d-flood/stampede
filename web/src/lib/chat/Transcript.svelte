<script lang="ts">
	import Calls from '#lib/chat/Calls.svelte';
	import Canvas from '#lib/chat/Canvas.svelte';
	import Html from '#lib/chat/Html.svelte';
	import Media from '#lib/chat/Media.svelte';
	import Notice from '#lib/chat/Notice.svelte';
	import Spawn from '#lib/chat/Spawn.svelte';
	import Text from '#lib/chat/Text.svelte';
	import User from '#lib/chat/User.svelte';
	import Questions from '#lib/chat/Questions.svelte';
	import Shell from '#lib/chat/Shell.svelte';
	import type { AnsweredQuestions, Item, Subagent } from '#lib/device.svelte.ts';

	type Entry = Item | { kind: 'questions'; record: AnsweredQuestions };
	let { items, active = true, answered = [], activity, path = [], onopen, onattend, ondiff, oncanvas, onexpand, onrewind }: { items: Item[]; active?: boolean; answered?: AnsweredQuestions[]; activity?: string | null; path?: string[]; onopen(path: string[]): void; onattend(subagent: string): void; ondiff(path: string): void; oncanvas(canvas: string): void; onexpand?(index: number): void; onrewind?(index: number): void } = $props();
	const entries = $derived.by(() => {
		const records = (after: number): Entry[] => answered.filter((record) => Math.min(record.after, items.length) === after).map((record) => ({ kind: 'questions', record }));
		return [...records(0), ...items.flatMap((item, index): Entry[] => [item, ...records(index + 1)])];
	});

	function interim(item: Entry) {
		return !['user', 'shell', 'text', 'html', 'media', 'notice', 'questions'].includes(item.kind) && !(item.kind === 'canvas' && item.write);
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
</script>

{#snippet entry(item: Entry, index: number)}
	{#if item.kind === 'user'}
		<User {item} first={index === 0} index={items.indexOf(item)} number={items.slice(0, items.indexOf(item)).filter((entry) => entry.kind === 'user').length + 1} {onrewind} />
	{:else if item.kind === 'text'}
		<Text {item} {active} />
	{:else if item.kind === 'html'}
		<Html {item} />
	{:else if item.kind === 'media'}
		<Media {item} />
	{:else if item.kind === 'canvas'}
		<Canvas {item} {oncanvas} />
	{:else if item.kind === 'shell'}
		<Shell {item} onexpand={onexpand && (() => onexpand(items.indexOf(item)))} />
	{:else if item.kind === 'notice'}
		<Notice {item} />
	{:else if item.kind === 'questions'}
		<Questions record={item.record} />
	{/if}
{/snippet}

{#each runs as run (run.indices[0])}
	{#if run.kind === 'spawn'}
		<Spawn subagents={run.indices.map((index) => entries[index]).filter((item): item is Subagent => item.kind === 'subagent')} {path} {onopen} {onattend} />
	{:else if run.kind === 'rail'}
		<Calls items={run.indices.map((index) => entries[index]).filter((item): item is Item => item.kind !== 'questions')} activity={run === runs.at(-1) ? activity : null} open={path.length > 0} {ondiff} {oncanvas} onexpand={onexpand && ((item) => onexpand(items.indexOf(item)))} />
	{:else}
		{#each run.indices as index (index)}
			{@render entry(entries[index], index)}
		{/each}
	{/if}
{/each}
