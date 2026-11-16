<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import Button from '#lib/ui/Button.svelte';
	import { onDestroy } from 'svelte';

	let { code, path, formatted = code }: { code: string; path: string; formatted?: string } = $props();
	let original = $state(false);
	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout>;
	onDestroy(() => clearTimeout(timer));

	async function copy() {
		await navigator.clipboard.writeText(code);
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), 1500);
	}
</script>

<div class="code-block">
	<div class="actions">
		{#if formatted !== code}<Button variant="ghost" size="sm" aria-pressed={original} onclick={() => (original = !original)}>{original ? 'Show formatted' : 'Show original'}</Button>{/if}
		<Button variant="ghost" size="sm" onclick={copy}>{copied ? 'Copied' : 'Copy original'}</Button>
	</div>
	<pre><Code code={original ? code : formatted} {path} wrap /></pre>
</div>

<style>
	.code-block {
		min-width: 0;
		border-radius: var(--radius-md);
		background: var(--surface-1);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		flex-wrap: wrap;
		padding: var(--space-1) var(--space-1) 0;
		font-family: var(--font-ui);
		font-size: var(--font-size-sm);
	}

	pre {
		margin: 0;
		padding: var(--space-2) var(--space-3) var(--space-3);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		tab-size: 4;
	}
</style>
