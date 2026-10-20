<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import type { Item } from '#lib/device.svelte.ts';

	let { item, onexpand }: { item: Extract<Item, { kind: 'command' }>; onexpand?(): void } = $props();

	const outcomes = { running: 'running…', ok: '✓', failed: '✗ failed' };

	const output = $derived(item.output.replace(/\x1b\[[\d;]*m/g, ''));
</script>

<details class={item.status} ontoggle={(event) => event.currentTarget.open && item.elided && onexpand?.()}>
	<summary>
		<span class="command">$ <Code code={item.command} path="command.sh" /></span>
		<span class="outcome">{outcomes[item.status]}</span>
	</summary>
	{#if output}
		<pre>{output}</pre>
	{/if}
</details>

<style>
	details {
		font-family: var(--font-mono);
		font-size: 0.92em;
	}

	summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		min-height: var(--target-min);
		list-style: none;
		cursor: pointer;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	.command {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	details[open] .command {
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.outcome {
		flex-shrink: 0;
	}

	.running .outcome {
		color: var(--status-working);
	}

	.ok .outcome {
		color: var(--status-done);
	}

	.failed .outcome {
		color: var(--ansi-1);
	}

	pre {
		max-height: 24em;
		margin: var(--space-1) 0 0;
		padding: var(--space-3);
		overflow: auto;
		border-radius: var(--radius-lg);
		background: var(--surface-1);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
