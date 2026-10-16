<script lang="ts">
	import ContextBar from '#lib/chat/ContextBar.svelte';
	import { count } from '#lib/chat/count.ts';
	import type { Chat } from '#lib/device.svelte.ts';

	let { chat }: { chat: Chat } = $props();
</script>

<div class="meters">
	{#if chat.context}
		<ContextBar context={chat.context} />
	{/if}
	{#if chat.tokens}
		<span class="tokens">{count(chat.tokens.input)} in · {count(chat.tokens.output)} out</span>
	{/if}
</div>

<style>
	.meters {
		display: flex;
		flex: none;
		align-items: center;
		min-height: 16px;
		padding: 0 var(--space-2) var(--space-2);
		background: var(--surface-1);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.tokens {
		margin-left: auto;
		font-family: var(--font-mono);
	}
</style>
