<script lang="ts">
	import { clock, totals } from '#lib/chat/agents.ts';
	import type { Subagent } from '#lib/device.svelte.ts';
	import { agentNames, logos } from '#lib/chat/sessions.ts';
	import { SubagentMark, WaitingMark } from '#lib/ui/spinners.ts';
	import CheckBold from '~icons/ph/check-bold';
	import XBold from '~icons/ph/x-bold';

	let { subagent }: { subagent: Subagent } = $props();

	const marks = { working: SubagentMark, waiting: WaitingMark, done: CheckBold };
	const ended = $derived(subagent.outcome === 'failed' || subagent.outcome === 'stopped' ? subagent.outcome : undefined);
	const Mark = $derived(ended ? XBold : marks[subagent.status]);
</script>

<span class="row {subagent.status}" class:ended>
	<span class="mark"><Mark /></span>
	<span class="body">
		<span class="title">
			<span class="task">{subagent.task}</span>
			{#if subagent.agent}{@const Logo = logos[subagent.agent]}<span class="logo" role="img" aria-label={agentNames[subagent.agent]} title={agentNames[subagent.agent]}><Logo aria-hidden="true" /></span>{/if}
			{#if subagent.name}<span class="type">{subagent.name}</span>{/if}
			{#if subagent.role}<span class="role">{subagent.role}</span>{/if}
		</span>
		{#if subagent.status !== 'done' && subagent.activity}<span class="activity">{subagent.activity}</span>{/if}
	</span>
	<span class="meta">
		{#if subagent.status !== 'done'}
			{#if subagent.startedAt}{clock(subagent.startedAt)}{/if}
		{:else}
			{#if ended}<span class="outcome">{ended}</span>{/if}
			{#if ended && subagent.totals}·{/if}
			{#if subagent.totals}{totals(subagent.totals)}{/if}
		{/if}
	</span>
</span>

<style>
	.row { display: flex; flex: 1; align-items: center; gap: var(--space-2); min-width: 0; }
	.mark { display: inline-flex; flex: none; color: var(--status-done); }
	.working .mark { color: var(--status-working); }
	.waiting .mark { color: var(--status-waiting); }
	.ended .mark, .outcome { color: var(--ansi-1); }
	.body { display: flex; flex: 1; flex-direction: column; min-width: 0; }
	.title { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-1) var(--space-2); min-width: 0; }
	.task, .activity { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.type { flex: none; padding: 0 var(--space-1); border-radius: var(--radius-sm); background: var(--surface-3); color: var(--muted-fg); font-size: var(--font-size-sm); }
	.logo { display: inline-flex; flex: none; }
	.role { flex: none; color: var(--muted-fg); font-size: var(--font-size-sm); }
	.activity { color: var(--muted-fg); font-family: var(--font-mono); font-size: var(--font-size-sm); }
	.meta { flex: none; color: var(--muted-fg); font-size: var(--font-size-sm); font-variant-numeric: tabular-nums; }
</style>
