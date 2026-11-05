<script lang="ts">
	import { clock, color, duration, tools } from '#lib/chat/agents.ts';
	import { agentNames, logos } from '#lib/chat/sessions.ts';
	import SubagentDot from '#lib/chat/SubagentDot.svelte';
	import type { Item, Subagent } from '#lib/device.svelte.ts';

	let { subagents, path, main, onopen, onattend }: { subagents: Subagent[]; path: string[]; main: Item[]; onopen(path: string[]): void; onattend(subagent: string): void } = $props();

	function state(subagent: Subagent) {
		if (subagent.status === 'waiting') return 'waiting for you';
		if (subagent.status === 'working') return subagent.startedAt ? clock(subagent.startedAt) : 'started';
		if (subagent.outcome === 'failed' || subagent.outcome === 'stopped') return subagent.outcome;
		return subagent.totals ? `finished in ${duration(subagent.totals.durationMs)}, ${tools(subagent.totals.tools)}` : 'finished';
	}
</script>

<div class="lines">
	{#each subagents as subagent (subagent.id)}
		{@const placed = [...path, subagent.id]}
		<button class="spawn" class:failed={subagent.outcome === 'failed'} class:waiting={subagent.status === 'waiting'} onclick={() => (subagent.status === 'waiting' ? onattend(subagent.id) : onopen(placed))}>
			<SubagentDot color={color(main, placed)} pulse={subagent.status !== 'done'} />
			{#if subagent.agent}{@const Logo = logos[subagent.agent]}<span class="logo" role="img" aria-label={agentNames[subagent.agent]} title={agentNames[subagent.agent]}><Logo aria-hidden="true" /></span>{/if}
			{#if subagent.name}<span class="name">{subagent.name}</span>{/if}
			{#if subagent.role}<span class="role">{subagent.role}</span>{/if}
			<span class="task">{subagent.task}</span>
			<span class="state">{state(subagent)}</span>
		</button>
	{/each}
</div>

<style>
	.lines { display: flex; flex-direction: column; align-items: flex-start; }
	.spawn { display: flex; align-items: center; gap: var(--space-2); max-width: 100%; min-height: var(--target-min); padding: 0 var(--space-2); margin-left: calc(-1 * var(--space-2)); border: none; border-radius: var(--radius-md); background: none; color: var(--muted-fg); font-family: var(--font-ui); font-size: var(--font-size-sm); white-space: nowrap; text-align: start; cursor: pointer; }
	.spawn:hover { background: var(--hover); }
	.logo { display: inline-flex; flex: none; color: var(--color-fg); }
	.name { flex: none; color: var(--color-fg); font-weight: 600; }
	.role { flex: none; }
	.task { flex: 0 1 auto; min-width: 0; overflow: hidden; color: var(--color-fg); text-overflow: ellipsis; }
	.state { flex: none; font-variant-numeric: tabular-nums; }
	.waiting .state { color: var(--status-waiting); }
	.failed .state { color: var(--ansi-1); }
</style>
