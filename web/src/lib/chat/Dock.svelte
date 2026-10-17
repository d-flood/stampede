<script lang="ts">
	import { clock, type Placed } from '#lib/chat/agents.ts';
	import SubagentMenu from '#lib/chat/SubagentMenu.svelte';
	import CircleNotchBold from '~icons/ph/circle-notch-bold';
	import HandPalmFill from '~icons/ph/hand-palm-fill';

	const PILL = 10;
	const MORE = 3.5;

	let { subagents, onopen }: { subagents: Placed[]; onopen(path: string[]): void } = $props();

	const running = $derived(subagents.filter(({ subagent }) => subagent.status !== 'done'));
	const finished = $derived(subagents.filter(({ subagent }) => subagent.status === 'done').reverse().sort((a, b) => ended(b) - ended(a)));
	let space = $state({ width: 0, em: 0, gap: 0 });
	const fits = $derived.by(() => {
		const { width, em, gap } = space;
		if (!width || running.length * (PILL * em + gap) - gap <= width) return running.length;
		return Math.max(0, Math.floor((width - MORE * em) / (PILL * em + gap)));
	});
	let more = $state(false);
	let list = $state(false);

	function ended({ subagent }: Placed) {
		return (subagent.startedAt ?? 0) + (subagent.totals?.durationMs ?? 0);
	}

	function measure(element: HTMLElement) {
		const observer = new ResizeObserver(() => {
			const style = getComputedStyle(element);
			space = { width: element.clientWidth, em: parseFloat(style.fontSize), gap: parseFloat(style.columnGap) };
		});
		observer.observe(element);
		return () => observer.disconnect();
	}
</script>

{#snippet overflow(props: Record<string, unknown>)}
	<button {...props} class="pill more" style:min-width="{MORE}em">+{running.length - fits}</button>
{/snippet}

{#snippet chip(props: Record<string, unknown>)}
	<button {...props} class="chip">{finished.length} finished</button>
{/snippet}

<section class="dock" aria-label="Subagents">
	<div class="pills" {@attach measure}>
		{#each running.slice(0, fits) as { path, subagent } (path.join('/'))}
			<button class="pill {subagent.status}" style:min-width="{PILL}em" title={subagent.task} onclick={() => onopen(path)}>
				<span class="mark">{#if subagent.status === 'waiting'}<HandPalmFill />{:else}<CircleNotchBold />{/if}</span>
				<span class="task">{subagent.task}</span>
				{#if subagent.startedAt}<span class="clock">{clock(subagent.startedAt)}</span>{/if}
			</button>
		{/each}
		{#if fits < running.length}
			<SubagentMenu heading="Running" entries={running} bind:open={more} trigger={overflow} onselect={onopen} />
		{/if}
	</div>
	{#if finished.length}
		<SubagentMenu heading="Finished" entries={finished} bind:open={list} trigger={chip} onselect={onopen} />
	{/if}
</section>

<style>
	.dock { display: flex; align-items: center; gap: var(--space-2); font-size: var(--font-size-sm); }
	.pills { display: flex; flex: 1; gap: var(--space-2); min-width: 0; overflow: hidden; }
	.pill, .chip { display: inline-flex; flex: none; align-items: center; gap: var(--space-1); min-height: var(--target-min); padding: 0 var(--space-2); border: var(--border-width) solid var(--border); border-radius: calc(var(--target-min) / 2); background: var(--surface-1); color: var(--color-fg); font: inherit; white-space: nowrap; cursor: pointer; }
	.pill { flex: 0 1 auto; max-width: 22em; }
	.more { flex: none; justify-content: center; }
	.chip { color: var(--muted-fg); }
	.pill:hover, .chip:hover { background: var(--hover); }
	.mark { display: inline-flex; flex: none; color: var(--status-working); }
	.working .mark { animation: spin 1s linear var(--pulse); }
	.waiting { border-color: var(--status-waiting); background: color-mix(in oklab, var(--status-waiting) 16%, transparent); }
	.waiting .mark { color: var(--status-waiting); }
	.task { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; text-align: start; }
	.clock { flex: none; color: var(--muted-fg); font-variant-numeric: tabular-nums; }
	@keyframes spin { to { transform: rotate(1turn); } }
</style>
