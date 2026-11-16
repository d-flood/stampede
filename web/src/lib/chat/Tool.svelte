<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import CodeBlock from '#lib/chat/CodeBlock.svelte';
	import { formatJson } from '#lib/chat/format-json.ts';
	import type { ToolCall } from '#lib/device.svelte.ts';
	import Caret from '#lib/ui/Caret.svelte';

	let { call, open = $bindable(false) }: { call: ToolCall; open?: boolean } = $props();
	const formatted = $derived(open ? formatJson(call.detail) : undefined);
</script>

<details class="tool" bind:open>
	<summary>
		<Caret /><span class="name" title={call.name}>{call.name}</span>
		{#if !open}<span class="preview" title={call.detail}><Code code={call.detail} path="arguments.json" /></span>{/if}
	</summary>
	{#if open}<CodeBlock code={call.detail} formatted={formatted ?? call.detail} path={formatted === undefined ? 'arguments.txt' : 'arguments.json'} />{/if}
</details>

<style>
	.tool { min-width: 0; }

	summary {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		list-style: none;
		cursor: pointer;
	}

	summary::-webkit-details-marker { display: none; }

	.name {
		min-width: 0;
		flex: 0 1 auto;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	details[open] .name { white-space: normal; overflow-wrap: anywhere; }

	.preview {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--muted-fg);
		font-family: var(--font-mono);
	}
</style>
