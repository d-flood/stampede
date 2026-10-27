<script lang="ts">
	import { icon, icons } from '#lib/chat/icons.ts';
	import type { Item } from '#lib/device.svelte.ts';
	import type { Snippet } from 'svelte';
	import CaretRight from '~icons/ph/caret-right';

	let { items, activity, open = false, children }: { items: Item[]; activity?: string | null; open?: boolean; children: Snippet } = $props();

	const count = $derived(items.reduce((count, item) => count + (item.kind === 'tools' ? item.calls.length : ['command', 'edit'].includes(item.kind) ? 1 : 0), 0));
	const last = $derived.by(() => {
		for (const item of items.toReversed()) {
			if (item.kind === 'command') return { icon: icons.command, text: `$ ${item.command}` };
			if (item.kind === 'edit') return { icon: icon(item), text: `Edited ${item.path}` };
			if (item.kind === 'tools') {
				const call = item.calls.at(-1);
				if (call) return { icon: icons[call.kind], text: `${call.name}${call.kind === 'other' || call.kind === 'mcp' ? '' : ` ${call.detail}`}` };
			}
		}
	});
	const latest = $derived(activity || last?.text || 'Thoughts');
</script>

{#if count || items.length > 1}
	<details class="calls" {open}>
		<summary><span class="caret"><CaretRight /></span>{#if count}<span class="count">{count} tool {count === 1 ? 'call' : 'calls'}</span>{/if}{#if last?.icon}<span class="icon"><last.icon /></span>{/if}<span class="latest">{latest}</span></summary>
		<div class="content">{@render children()}</div>
	</details>
{:else}
	{@render children()}
{/if}

<style>
	summary { display: flex; align-items: center; gap: var(--space-2); min-height: var(--target-min); color: var(--muted-fg); font-size: var(--font-size-sm); list-style: none; cursor: pointer; }
	summary::-webkit-details-marker { display: none; }
	.caret { display: inline-flex; flex: none; width: 1em; transition: transform var(--dur-fast); }
	details[open] > summary > .caret { transform: rotate(90deg); }
	summary:hover { color: var(--color-fg); }
	.count { flex: none; }
	.count::after { content: ' ·'; }
	.latest { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--font-mono); }
	.icon { display: inline-flex; flex: none; }
	.content { padding-top: var(--space-2); }
</style>
