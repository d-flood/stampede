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
	import type { Agent } from '#lib/config.svelte.ts';
	import type { AnsweredQuestions, Item, ResetCredits, Subagent } from '#lib/device.svelte.ts';
	import { tick } from 'svelte';

	type Entry = Item | { kind: 'questions'; record: AnsweredQuestions };
	let { host, items, main, active = true, answered = [], activity, path = [], agent, resets, outlined, onopen, onattend, ondiff, oncanvas, onexpand, onrewind, onfork, onhandoff }: { host: string | undefined; items: Item[]; main?: Item[]; active?: boolean; answered?: AnsweredQuestions[]; activity?: string | null; path?: string[]; agent?: Agent; resets?: ResetCredits | null; /** The item whose block wears an outline. */ outlined?: number; onhandoff?(): void; onopen(path: string[]): void; onattend(subagent: string): void; ondiff(path: string): void; oncanvas(canvas: string): void; onexpand?(index: number): void; onrewind?(index: number): void; onfork?(index: number): void } = $props();
	const wrapped = new WeakMap<AnsweredQuestions, Entry>();
	const entries = $derived.by(() => {
		const wrap = (record: AnsweredQuestions) => wrapped.get(record) ?? wrapped.set(record, { kind: 'questions', record }).get(record)!;
		const records = (after: number): Entry[] => answered.filter((record) => Math.min(record.after, items.length) === after).map(wrap);
		return [...records(0), ...items.flatMap((item, index): Entry[] => [item, ...records(index + 1)])];
	});

	const positions = $derived(new Map(items.map((item, index) => [item, index])));

	const prompts = $derived.by(() => {
		const prompts = new Map<Item, { index: number; number: number }>();
		items.forEach((item, index) => item.kind === 'user' && prompts.set(item, { index, number: prompts.size + 1 }));
		return prompts;
	});

	function interim(item: Entry) {
		return !['user', 'shell', 'text', 'html', 'media', 'notice', 'questions'].includes(item.kind) && !(item.kind === 'canvas' && item.write);
	}

	type Block = { kind: 'spawn' | 'rail' | 'plain'; indices: number[]; entries: Entry[] };
	let previous: Block[][] = [];
	// Reuse unchanged turns so a new item re-renders only the turn it lands in.
	const turns = $derived.by(() => {
		const built = entries.reduce<Block[][]>((turns, item, index) => {
			const kind = item.kind === 'subagent' ? 'spawn' : interim(item) ? 'rail' : 'plain';
			if (!turns.length || (item.kind === 'user' && turns.at(-1)!.length)) turns.push([]);
			const turn = turns.at(-1)!;
			const last = turn.at(-1);
			if (kind !== 'plain' && last?.kind === kind) {
				last.indices.push(index);
				last.entries.push(item);
			} else turn.push({ kind, indices: [index], entries: [item] });
			return turns;
		}, []);
		previous = built.map((turn, index) => (same(turn, previous[index]) ? previous[index] : turn));
		return previous;
	});

	function same(turn: Block[], before: Block[] | undefined) {
		return turn.length === before?.length && turn.every((block, index) => {
			const old = before[index];
			return block.kind === old.kind && block.indices[0] === old.indices[0] && block.entries.length === old.entries.length && block.entries.every((entry, at) => entry === old.entries[at]);
		});
	}

	// Render the newest turns first and older ones in idle time, so a long Chat paints without a long freeze.
	const batch = 10;
	let limit = $state(batch);
	const shown = $derived(turns.length > limit ? turns.slice(-limit) : turns);

	$effect(() => {
		if (limit === Infinity || !turns.length) return;
		if (limit >= turns.length) {
			limit = Infinity;
			return;
		}
		if (typeof requestIdleCallback !== 'function') {
			const timer = setTimeout(() => (limit += batch));
			return () => clearTimeout(timer);
		}
		const idle = requestIdleCallback(() => (limit += batch));
		return () => cancelIdleCallback(idle);
	});

	let root = $state<HTMLElement>();

	export async function reveal(item: number, land: (node: HTMLElement) => void) {
		const target = items[item];
		const at = turns.findIndex((turn) => turn.some((block) => block.entries.includes(target)));
		if (at < 0) return;
		// A turn above keeps the target off scroll offset 0, where browsers stop anchoring as older turns render in.
		limit = Math.max(limit, turns.length - at + 1);
		await tick();
		const block = turns[at].find((block) => block.entries.includes(target))!;
		const node = root?.querySelector<HTMLElement>(`[data-item="${positions.get(block.entries[0] as Item)}"]`);
		if (node) land(node);
	}

	function role(block: Block) {
		if (block.kind === 'rail') return 'activity';
		if (block.kind === 'spawn') return 'subagent';
		const [item] = block.entries;
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
		{@const prompt = prompts.get(item)!}
		<User {host} {item} first={index === 0} index={prompt.index} number={prompt.number} {onrewind} {onfork} />
	{:else if item.kind === 'text'}
		<Text {item} {active} />
	{:else if item.kind === 'html'}
		<Html {item} />
	{:else if item.kind === 'media'}
		<Media {host} {item} />
	{:else if item.kind === 'canvas'}
		<Canvas {item} {oncanvas} />
	{:else if item.kind === 'shell'}
		<Shell {item} onexpand={onexpand && (() => onexpand(items.indexOf(item)))} />
	{:else if item.kind === 'notice'}
		<Notice {item} {agent} {resets} {onhandoff} />
	{:else if item.kind === 'questions'}
		<Questions {host} record={item.record} />
	{/if}
{/snippet}

<div class="turns" bind:this={root}>
	{#each shown as turn (turn[0].indices[0])}
		{@const first = turn[0].entries[0]}
		<div class="turn" data-user-message={first.kind === 'user' ? prompts.get(first)!.index : undefined}>
			{#each turn as block, index (block.indices[0])}
				{@const at = positions.get(block.entries[0] as Item)}
				<div class="block" class:outlined={at !== undefined && at === outlined} data-item={at} style:margin-top={space(turn, index)}>
					{#if block.kind === 'spawn'}
						<Spawn subagents={block.entries.filter((item): item is Subagent => item.kind === 'subagent')} {path} main={main ?? items} {onopen} {onattend} />
					{:else if block.kind === 'rail'}
						<Calls items={block.entries.filter((item): item is Item => item.kind !== 'questions')} activity={block === turns.at(-1)!.at(-1) ? activity : null} open={path.length > 0} {ondiff} {oncanvas} onexpand={onexpand && ((item) => onexpand(items.indexOf(item)))} />
					{:else}
						{@render entry(block.entries[0], block.indices[0])}
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

	.outlined {
		border-radius: var(--radius-md);
		outline: calc(2 * var(--border-width)) solid var(--selected-border);
		outline-offset: var(--space-2);
	}

	/* Skipped turns resize as they scroll into view, which only scroll anchoring keeps steady. */
	@supports (overflow-anchor: auto) {
		.turn {
			content-visibility: auto;
			contain-intrinsic-size: auto 500px;
			/* Room for focus rings and a prompt's actions button, which paint containment would clip. */
			overflow-clip-margin: var(--space-6);
		}
	}

	@media (pointer: coarse) and (max-width: 767px) {
		.turns {
			gap: 32px;
		}
	}
</style>
