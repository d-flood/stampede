<script lang="ts">
	import { icons } from '#lib/chat/icons.ts';
	import SubagentMenu from '#lib/chat/SubagentMenu.svelte';
	import SubagentSheet from '#lib/chat/SubagentSheet.svelte';
	import type { Subagent } from '#lib/device.svelte.ts';
	import { form } from '#lib/form.ts';

	let { subagents, path, onopen, onattend }: { subagents: Subagent[]; path: string[]; onopen(path: string[]): void; onattend(subagent: string): void } = $props();

	const entries = $derived(subagents.map((subagent) => ({ path: [...path, subagent.id], subagent })));
	const running = $derived(subagents.filter(({ status }) => status !== 'done').length);
	const status = $derived(subagents.some(({ status }) => status === 'waiting') ? 'waiting' : running ? 'working' : 'done');
	const text = $derived(`${subagents.length} ${subagents.length === 1 ? 'Subagent' : 'Subagents'} · ${running ? `${running} running` : 'done'}`);
	let open = $state(false);
</script>

{#snippet marker(props: Record<string, unknown>)}
	<button {...props} class="spawn {status}"><span class="icon"><icons.subagent /></span>{text}</button>
{/snippet}

{#if entries.length === 1}
	{@render marker({ onclick: () => onopen(entries[0].path) })}
{:else if form() === 'phone'}
	{@render marker({ onclick: () => (open = true) })}
	<SubagentSheet title={text} groups={[{ entries }]} bind:open {onopen} {onattend} />
{:else}
	<SubagentMenu heading="Subagents" {entries} bind:open trigger={marker} onselect={onopen} side="bottom" align="start" />
{/if}

<style>
	.spawn { display: inline-flex; align-self: flex-start; align-items: center; gap: var(--space-2); max-width: 100%; min-height: var(--target-min); padding: 0 var(--space-3) 0 var(--space-2); border: var(--border-width) solid var(--border); border-radius: calc(var(--target-min) / 2); background: var(--surface-1); color: var(--muted-fg); font: inherit; font-size: var(--font-size-sm); white-space: nowrap; cursor: pointer; }
	.spawn:hover { background: var(--hover); color: var(--color-fg); }
	.icon { display: inline-flex; flex: none; }
	.working .icon { color: var(--status-working); }
	.waiting .icon { color: var(--status-waiting); }
</style>
