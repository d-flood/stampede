<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import type { Item } from '#lib/device.svelte.ts';

	let { item, onexpand }: { item: Extract<Item, { kind: 'shell' }>; onexpand?(): void } = $props();

	const outcomes = { running: 'running…', ok: '✓', failed: '✗ failed' };

	const output = $derived(item.output.replace(/\x1b\[[\d;]*m/g, ''));
</script>

<details class="shell {item.status}" open={!item.elided} ontoggle={(event) => event.currentTarget.open && item.elided && onexpand?.()}>
	<summary>
		<span class="command">! <Code code={item.command} path="command.sh" /></span>
		<span class="outcome">
			{#if item.pending}<span class="pending" title="Sent with your next message">next message ·</span>{/if}
			{outcomes[item.status]}
		</span>
	</summary>
	{#if output}
		<pre>{output}</pre>
	{/if}
</details>

<style>
	details {
		align-self: flex-end;
		box-sizing: border-box;
		width: min(100%, 40em);
		max-width: 85%;
		padding: 0 var(--space-4);
		border: var(--border-width) solid color-mix(in oklab, var(--ansi-4) 38%, var(--color-bg));
		border-radius: var(--radius-lg) var(--radius-lg) var(--radius-sm) var(--radius-lg);
		background: color-mix(in oklab, var(--ansi-4) 16%, var(--color-bg));
		font-family: var(--font-mono);
		font-size: 0.92em;
	}

	summary {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0 var(--space-4);
		min-height: var(--target-min);
		list-style: none;
		cursor: pointer;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	.command {
		flex: 1 1 16em;
		min-width: 0;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.outcome {
		flex-shrink: 0;
		margin-left: auto;
	}

	.pending {
		color: var(--muted-fg);
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
		margin: 0 0 var(--space-3);
		padding: var(--space-3);
		overflow: auto;
		border-radius: var(--radius-md);
		background: var(--color-bg);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
