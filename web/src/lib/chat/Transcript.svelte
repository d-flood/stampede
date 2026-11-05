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
	let { items, main, active = true, answered = [], activity, path = [], onopen, onattend, ondiff, oncanvas, onexpand, onrewind }: { items: Item[]; main?: Item[]; active?: boolean; answered?: AnsweredQuestions[]; activity?: string | null; path?: string[]; onopen(path: string[]): void; onattend(subagent: string): void; ondiff(path: string): void; oncanvas(canvas: string): void; onexpand?(index: number): void; onrewind?(index: number): void } = $props();
	const entries = $derived.by(() => {
		const records = (after: number): Entry[] => answered.filter((record) => Math.min(record.after, items.length) === after).map((record) => ({ kind: 'questions', record }));
		return [...records(0), ...items.flatMap((item, index): Entry[] => [item, ...records(index + 1)])];
	});

	function interim(item: Entry) {
		return !['user', 'shell', 'text', 'html', 'media', 'notice', 'questions'].includes(item.kind) && !(item.kind === 'canvas' && item.write);
	}

	type Block = { kind: 'spawn' | 'rail' | 'plain'; indices: number[] };
	const turns = $derived(
		entries.reduce<Block[][]>((turns, item, index) => {
			const kind = item.kind === 'subagent' ? 'spawn' : interim(item) ? 'rail' : 'plain';
			if (!turns.length || (item.kind === 'user' && turns.at(-1)!.length)) turns.push([]);
			const turn = turns.at(-1)!;
			const last = turn.at(-1);
			if (kind !== 'plain' && last?.kind === kind) last.indices.push(index);
			else turn.push({ kind, indices: [index] });
			return turns;
		}, [])
	);

	function role(block: Block) {
		if (block.kind === 'rail') return 'activity';
		if (block.kind === 'spawn') return 'subagent';
		const item = entries[block.indices[0]];
		return item.kind === 'user' ? 'prompt' : item.kind === 'text' ? item.phase : 'other';
	}

	const quiet = ['commentary', 'activity', 'subagent'];

	function space(turn: Block[], index: number) {
		if (!index) return undefined;
		const [previous, current] = [role(turn[index - 1]), role(turn[index])];
		if (previous === 'prompt') return '20px';
		if (current === 'final') return '22px';
		if (previous === 'activity' && current === 'commentary') return '14px';
		if (quiet.includes(previous) && quiet.includes(current)) return '8px';
		return 'var(--space-4)';
	}
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

<div class="turns">
	{#each turns as turn (turn[0].indices[0])}
		<div class="turn">
			{#each turn as block, index (block.indices[0])}
				<div class="block" style:margin-top={space(turn, index)}>
					{#if block.kind === 'spawn'}
						<Spawn subagents={block.indices.map((index) => entries[index]).filter((item): item is Subagent => item.kind === 'subagent')} {path} main={main ?? items} {onopen} {onattend} />
					{:else if block.kind === 'rail'}
						<Calls items={block.indices.map((index) => entries[index]).filter((item): item is Item => item.kind !== 'questions')} activity={block === turns.at(-1)!.at(-1) ? activity : null} open={path.length > 0} {ondiff} {oncanvas} onexpand={onexpand && ((item) => onexpand(items.indexOf(item)))} />
					{:else}
						{@render entry(entries[block.indices[0]], block.indices[0])}
					{/if}
				</div>
			{/each}
		</div>
	{/each}
</div>

<style>
	.turns {
		display: flex;
		flex-direction: column;
		gap: 44px;
	}

	.turn,
	.block {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	@media (pointer: coarse) and (max-width: 767px) {
		.turns {
			gap: 32px;
		}
	}
</style>
