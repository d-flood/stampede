<script lang="ts">
	import { color, same, subagents as flatten, type Placed } from '#lib/chat/agents.ts';
	import Canvases from '#lib/chat/Canvases.svelte';
	import Shells from '#lib/chat/Shells.svelte';
	import { commands } from '#lib/chat/shells.ts';
	import SubagentDot from '#lib/chat/SubagentDot.svelte';
	import SubagentRow from '#lib/chat/SubagentRow.svelte';
	import SubagentSheet from '#lib/chat/SubagentSheet.svelte';
	import type { Item } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import Chip from '#lib/ui/Chip.svelte';
	import Menu from '#lib/ui/Menu.svelte';
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
	const menuGroups = $derived(groups.map(({ heading, entries }) => ({ heading, items: entries.map((entry) => ({ label: entry.subagent.task, data: entry, onselect: () => select(entry.path) })) })));
	const summary = $derived(
		[
			[running.length - waiting.length, 'running'],
			[finished.length, 'done']
		]
			.filter(([count]) => count)
			.map(([count, label]) => `${count} ${label}`)
			.join(', ')
	);
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

{#snippet subagentRow(data: unknown)}
	{@const { path, subagent } = data as Placed}
	<SubagentRow {subagent} color={color(items, path)} />
{/snippet}

{#snippet trigger(props: Record<string, unknown>)}
	<Chip {...props} aria-label={['Subagents', summary].filter(Boolean).join(': ')}>
		<span class="dots">
			{#each shown.slice(0, DOTS) as { path, subagent } (path.join('/'))}<SubagentDot color={color(items, path)} pulse={subagent.status !== 'done'} />{/each}
		</span>
		{#if shown.length > DOTS}<span class="more">+{shown.length - DOTS}</span>{/if}
		{#if summary}<span>{summary}</span>{/if}
	</Chip>
{/snippet}

{#if subagents.length || shells.length || canvases.length}
	<section class="tray" aria-label="Tray">
		{#if subagents.length}
			<section class="subagents" aria-label="Subagents">
				{#if form() === 'phone'}
					{@render trigger({ onclick: () => (sheet = true) })}
					<SubagentSheet title="Subagents" {groups} bind:open={sheet} main={items} {onopen} {onattend} />
				{:else}
					<Menu items={menuGroups} row={subagentRow} {trigger} side="top" />
				{/if}
				{#if waiting.length}
					<Chip onclick={() => onattend(waiting[0].subagent.id)}><span class="waiting">{waiting.length} waiting</span></Chip>
				{/if}
			</section>
		{/if}
		{#if shells.length}<Shells {shells} />{/if}
		{#if canvases.length}<Canvases {canvases} {oncanvas} />{/if}
	</section>
{/if}

<style>
	.tray { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); min-width: 0; font-size: var(--font-size-sm); }
	.subagents { display: flex; flex: none; gap: var(--space-1); }
	.dots { display: inline-flex; gap: 3px; margin-inline-end: var(--space-1); }
	.more { color: var(--muted-fg); font-variant-numeric: tabular-nums; }
	.waiting { color: var(--status-waiting); }
</style>
