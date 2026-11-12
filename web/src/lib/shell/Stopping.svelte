<script lang="ts" module>
	import type { Agent } from '#lib/config.svelte.ts';

	export type Stopped = { kind: 'command' | 'subagent'; label: string } | { kind: 'chat'; label: string; agent: Agent };
</script>

<script lang="ts">
	import { logos } from '#lib/chat/sessions.ts';
	import ShellMark from '#lib/ui/ShellMark.svelte';
	import { SubagentMark } from '#lib/ui/spinners.ts';

	let { items }: { items: Stopped[] } = $props();

	const mark = (item: Stopped) => (item.kind === 'chat' ? logos[item.agent] : item.kind === 'command' ? ShellMark : SubagentMark);
</script>

{#if items.length}
	<ul aria-label="Running">
		{#each items as item, index (index)}
			{@const Mark = mark(item)}
			<li class={item.kind}>
				<span class="mark" aria-hidden="true"><Mark /></span>
				<span class="label" title={item.label}>{item.label}</span>
			</li>
		{/each}
	</ul>
{/if}

<style>
	ul {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		max-height: 12rem;
		margin: 0;
		padding: var(--space-3);
		overflow-y: auto;
		list-style: none;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
	}

	li {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		font-size: var(--font-size-sm);
	}

	.mark {
		display: inline-flex;
		flex: none;
		color: var(--muted-fg);
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.command .label {
		font-family: var(--font-mono);
	}
</style>
