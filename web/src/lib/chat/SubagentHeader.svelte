<script lang="ts">
	import { clock, totals, type Placed } from '#lib/chat/agents.ts';
	import { icons } from '#lib/chat/icons.ts';
	import type { Subagent } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';
	import CaretLeft from '~icons/ph/caret-left';
	import CheckBold from '~icons/ph/check-bold';
	import CircleNotchBold from '~icons/ph/circle-notch-bold';
	import HandPalmFill from '~icons/ph/hand-palm-fill';
	import XBold from '~icons/ph/x-bold';

	let { subagent, trail, onopen }: { subagent: Subagent; trail: Placed[]; onopen(path: string[]): void } = $props();

	const marks = { working: CircleNotchBold, waiting: HandPalmFill, done: CheckBold };
	const ended = $derived(subagent.outcome === 'failed' || subagent.outcome === 'stopped' ? subagent.outcome : undefined);
	const Mark = $derived(ended ? XBold : marks[subagent.status]);
</script>

<header class="header" class:phone={form() === 'phone'}>
	<nav aria-label="Breadcrumb">
		<button class="crumb" onclick={() => onopen([])}><CaretLeft />Main</button>
		{#each trail as { path, subagent: ancestor } (path.join('/'))}
			<span class="separator">/</span>
			<button class="crumb" title={ancestor.task} onclick={() => onopen(path)}>{ancestor.task}</button>
		{/each}
		<span class="separator">/</span>
		<span class="current" aria-current="page"><icons.subagent /><strong>{subagent.task}</strong></span>
	</nav>
	{#if subagent.name}<span class="chip" title="Type">{subagent.name}</span>{/if}
	{#if subagent.model}<span class="chip" title="Model">{subagent.model}</span>{/if}
	<span class="status {subagent.status}" class:ended>
		<span class="mark" title={ended ?? subagent.status}><Mark /></span>
		{#if subagent.status !== 'done'}
			{#if subagent.startedAt}<span>{clock(subagent.startedAt)}</span>{/if}
		{:else}
			{#if ended}<span class="outcome">{ended}</span>{/if}
			{#if ended && subagent.totals}<span>·</span>{/if}
			{#if subagent.totals}<span>{totals(subagent.totals)}</span>{/if}
		{/if}
	</span>
</header>

<style>
	.header { display: flex; flex: none; flex-wrap: wrap; align-items: center; gap: var(--space-1) var(--space-2); padding: var(--space-2) var(--space-6); border-bottom: var(--border-width) solid var(--border); font-size: var(--font-size-sm); }
	nav { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-1); min-width: 0; margin-right: var(--space-1); }
	.crumb { display: inline-flex; align-items: center; gap: var(--space-1); max-width: 14em; min-height: var(--target-min); padding: 0 var(--space-1); border: none; border-radius: var(--radius-md); background: none; color: var(--muted-fg); font: inherit; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; cursor: pointer; }
	.crumb:hover { background: var(--hover); color: var(--color-fg); }
	.separator { color: var(--muted-fg); }
	.current { display: inline-flex; align-items: center; gap: var(--space-1); min-width: 0; }
	.current :global(svg) { flex: none; }
	.current strong { min-width: 0; overflow-wrap: anywhere; }
	.chip { min-width: 0; padding: 0 var(--space-2); border: var(--border-width) solid var(--border); border-radius: 999px; background: var(--surface-2); color: var(--muted-fg); overflow-wrap: anywhere; }
	.status { display: inline-flex; flex-wrap: wrap; align-items: center; gap: var(--space-1); margin-left: auto; color: var(--muted-fg); font-variant-numeric: tabular-nums; }
	.mark { display: inline-flex; color: var(--status-done); }
	.working .mark { color: var(--status-working); animation: spin 1s linear var(--pulse); }
	.waiting .mark { color: var(--status-waiting); }
	.ended .mark, .outcome { color: var(--ansi-1); }
	.phone { padding: var(--space-2) var(--space-4); font-size: var(--font-size-md); }
	@keyframes spin { to { transform: rotate(1turn); } }
</style>
