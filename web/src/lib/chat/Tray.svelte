<script lang="ts">
	import { color, same, subagents as flatten, type Placed } from '#lib/chat/agents.ts';
	import Canvases from '#lib/chat/Canvases.svelte';
	import Shells from '#lib/chat/Shells.svelte';
	import { commands } from '#lib/chat/shells.ts';
	import SubagentDot from '#lib/chat/SubagentDot.svelte';
	import SubagentMenu from '#lib/chat/SubagentMenu.svelte';
	import SubagentSheet from '#lib/chat/SubagentSheet.svelte';
	import type { Item } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import { untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';

	const DELAY = 1500;
	const LINGER = 1000;
	const DOTS = 3;

	let { items, canvases = [], onopen, onattend, oncanvas }: { items: Item[]; canvases?: string[]; onopen(path: string[]): void; onattend(subagent: string): void; oncanvas(canvas: string): void } = $props();

	const all = $derived(commands(items));
	const times = new SvelteMap<string, { since: number; until?: number }>();
	let now = $state(Date.now());

	$effect(() => {
		for (const { id, item } of all) {
			const time = untrack(() => times.get(id));
			if (item.status === 'running' && !time) times.set(id, { since: Date.now() });
			else if (item.status !== 'running' && time && !time.until) times.set(id, { ...time, until: Date.now() });
		}
	});

	const shells = $derived(all.filter(({ id }) => {
		const time = times.get(id);
		return !!time && (time.until ?? now) - time.since >= DELAY && now - (time.until ?? now) < LINGER;
	}));
	const ticking = $derived([...times.values()].some(({ until }) => until === undefined || now - until < LINGER));

	$effect(() => {
		if (!ticking) return;
		const timer = setInterval(() => (now = Date.now()), 100);
		return () => clearInterval(timer);
	});

	const subagents = $derived(flatten(items));
	const running = $derived(subagents.filter(({ subagent }) => subagent.status !== 'done'));
	const finished = $derived(subagents.filter(({ subagent }) => subagent.status === 'done').reverse().sort((a, b) => ended(b) - ended(a)));
	const waiting = $derived(running.filter(({ subagent }) => subagent.status === 'waiting'));
	const groups = $derived([{ heading: 'Running', entries: running }, { heading: 'Finished', entries: finished }].filter(({ entries }) => entries.length));
	const shown = $derived([...running, ...finished]);
	const summary = $derived(
		[
			[running.length - waiting.length, 'running'],
			[finished.length, 'done']
		]
			.filter(([count]) => count)
			.map(([count, label]) => `${count} ${label}`)
			.join(', ')
	);
	let menu = $state(false);
	let sheet = $state(false);

	function ended({ subagent }: Placed) {
		return (subagent.startedAt ?? 0) + (subagent.totals?.durationMs ?? 0);
	}

	function select(path: string[]) {
		const subagent = subagents.find((placed) => same(placed.path, path))?.subagent;
		if (subagent?.status === 'waiting') onattend(subagent.id);
		else onopen(path);
	}
</script>

{#snippet trigger(props: Record<string, unknown>)}
	<button {...props} class="open" aria-label={['Subagents', summary].filter(Boolean).join(': ')}>
		<span class="dots">
			{#each shown.slice(0, DOTS) as { path, subagent } (path.join('/'))}<SubagentDot color={color(items, path)} pulse={subagent.status !== 'done'} />{/each}
		</span>
		{#if shown.length > DOTS}<span class="more">+{shown.length - DOTS}</span>{/if}
		{#if summary}<span>{summary}</span>{/if}
	</button>
{/snippet}

{#if subagents.length || shells.length || canvases.length}
	<section class="tray" aria-label="Tray">
		{#if subagents.length}
			<section class="chip subagents" aria-label="Subagents">
				{#if form() === 'phone'}
					{@render trigger({ onclick: () => (sheet = true) })}
					<SubagentSheet title="Subagents" {groups} bind:open={sheet} main={items} {onopen} {onattend} />
				{:else}
					<SubagentMenu {groups} main={items} bind:open={menu} {trigger} onselect={select} side="top" align="start" />
				{/if}
				{#if waiting.length}
					<button class="waiting" onclick={() => onattend(waiting[0].subagent.id)}>{waiting.length} waiting</button>
				{/if}
			</section>
		{/if}
		{#if shells.length}<Shells {shells} />{/if}
		{#if canvases.length}<Canvases {canvases} {oncanvas} />{/if}
	</section>
{/if}

<style>
	.tray { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); min-width: 0; font-size: var(--font-size-sm); }
	.chip { display: inline-flex; flex: none; align-items: stretch; min-height: var(--target-min); border: var(--border-width) solid var(--border); border-radius: calc(var(--target-min) / 2); background: var(--surface-1); overflow: hidden; white-space: nowrap; }
	.chip button { display: inline-flex; align-items: center; gap: var(--space-2); padding: 0 var(--space-2); border: none; background: none; color: var(--color-fg); font: inherit; cursor: pointer; }
	.chip button:hover { background: var(--hover); }
	.dots { display: inline-flex; gap: 3px; }
	.more { margin-inline-start: calc(-1 * var(--space-1)); color: var(--muted-fg); font-variant-numeric: tabular-nums; }
	.chip .waiting { border-inline-start: var(--border-width) solid var(--border); color: var(--status-waiting); }
</style>
