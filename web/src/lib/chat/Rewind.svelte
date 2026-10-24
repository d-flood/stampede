<script lang="ts">
	import type { Capabilities, Chat, Item } from '#lib/device.svelte.ts';
	import { agentNames } from '#lib/config.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';

	let { open = $bindable(false), target = $bindable<number>(), chat, items, capabilities, unavailable, onrewind }: {
		open: boolean;
		target?: number;
		chat: Chat;
		items: Item[];
		capabilities: Capabilities;
		unavailable?: string;
		onrewind(index: number): void;
	} = $props();

	const messages = $derived(items.flatMap((item, index) => item.kind === 'user' ? [{ item, index }] : []));
	const selected = $derived(messages.find(({ index }) => index === target));
	const boundary = $derived(selected ? selected.item.rewindFrom ?? selected.index : undefined);
	const first = $derived(messages.find(({ index }) => index === boundary));
	const number = $derived(messages.findIndex(({ index }) => index === boundary) + 1);
	const selectedNumber = $derived(messages.findIndex(({ index }) => index === target) + 1);
	const restores = $derived(chat.agent === 'claude-code' && capabilities.rewindsFiles);
	const title = $derived(target === undefined ? 'Choose where to rewind' : selected && first ? `Rewind before message ${number}?` : 'Rewind unavailable');

	function confirm() {
		if (unavailable || !selected || !first) return;
		onrewind(selected.index);
		open = false;
	}
</script>

<Dialog bind:open {title}>
	{#if target === undefined}
		<p>Choose the first user message to remove. That message and everything after it will be removed. Review the details before confirming.</p>
		<div class="messages">
			{#each messages as message, index (message.index)}
				<button onclick={() => (target = message.index)}><span>Message {index + 1}</span>{message.item.text || `${message.item.images} attached images`}</button>
			{/each}
			{#if !messages.length}<p>No user messages to rewind.</p>{/if}
		</div>
	{:else if selected && first}
		<p class="boundary"><strong>Message {selectedNumber} will be removed, including its prompt and attachments.</strong></p>
		{#if boundary !== target}
			<p>Codex rewinds whole turns. Message {selectedNumber} was sent during the turn that began at message {number}. That entire turn will be removed too.</p>
		{/if}
		<blockquote><span>First message removed · {number}</span>{first.item.text || `${first.item.images} attached images`}</blockquote>
		<dl>
			<div><dt>Keep</dt><dd>{number === 1 ? 'No earlier conversation. The chat will be empty.' : `Everything before message ${number}.`}</dd></div>
			<div><dt>Remove</dt><dd>Message {number} and every later message, agent reply, and tool result.</dd></div>
			<div><dt>Files · {agentNames[chat.agent]}</dt><dd>{restores ? `Restore checkpointed file edits to their state before message ${number}. Changes made through Bash or subagents may remain.` : 'Files are left unchanged, including edits made by the removed turns.'}</dd></div>
		</dl>
		<p>Commands, commits, and external actions are not undone.</p>
		{#if unavailable}<p role="status">{unavailable}</p>{/if}
	{:else}
		<p>This message is no longer in the conversation.</p>
	{/if}
	{#snippet actions()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		{#if selected && first}<Button variant="danger" disabled={!!unavailable} onclick={confirm}>Rewind before message {number}</Button>{/if}
	{/snippet}
</Dialog>

<style>
	p { margin: 0 0 var(--space-3); color: var(--muted-fg); }
	.boundary { color: var(--color-fg); }
	blockquote { max-height: 9rem; margin: 0 0 var(--space-4); padding: var(--space-3); overflow-y: auto; border-left: 2px solid var(--ansi-1); background: var(--surface-2); white-space: pre-wrap; overflow-wrap: anywhere; }
	blockquote span, .messages span { display: block; margin-bottom: var(--space-1); color: var(--muted-fg); font-size: var(--font-size-sm); }
	dl { margin: 0 0 var(--space-3); }
	dl div { margin-bottom: var(--space-3); }
	dt { font-weight: 600; }
	dd { margin: var(--space-1) 0 0; color: var(--muted-fg); }
	.messages { display: flex; flex-direction: column; gap: var(--space-2); max-height: 55dvh; overflow-y: auto; }
	.messages button { min-height: var(--target-min); padding: var(--space-3); border: var(--border-width) solid var(--border); border-radius: var(--radius-md); background: var(--surface-2); color: var(--color-fg); font: inherit; text-align: left; white-space: pre-wrap; overflow-wrap: anywhere; cursor: pointer; }
	.messages button:hover { background: var(--hover); }
	.messages button:focus-visible { outline: var(--border-width) solid var(--status-working); }
</style>
