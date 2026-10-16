<script lang="ts">
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';

	let { text = $bindable() }: { text?: string } = $props();

	let copied = $state(false);

	async function copyAll() {
		if (text === undefined) return;
		await navigator.clipboard.writeText(text);
		copied = true;
	}
</script>

<Dialog bind:open={() => text !== undefined, (open) => !open && ((text = undefined), (copied = false))} full title="Copy">
	<pre {@attach (pre) => pre.scrollTo({ top: pre.scrollHeight })}>{text}</pre>
	{#snippet actions()}
		<Button variant="primary" onclick={copyAll}>{copied ? 'Copied' : 'Copy all'}</Button>
	{/snippet}
</Dialog>

<style>
	pre {
		flex: 1;
		min-height: 0;
		margin: 0;
		padding: var(--space-2);
		overflow: auto;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-md);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
