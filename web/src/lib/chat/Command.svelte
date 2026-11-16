<script lang="ts">
	import Code from '#lib/chat/Code.svelte';
	import CommandCode from '#lib/chat/CommandCode.svelte';
	import Elapsed from '#lib/chat/Elapsed.svelte';
	import type { Item } from '#lib/device.svelte.ts';

	let { item, onexpand, full = false, open = $bindable(false) }: { item: Extract<Item, { kind: 'command' }>; onexpand?(): void; full?: boolean; open?: boolean } = $props();

	const outcomes = { running: 'running…', ok: '✓', failed: '✗ failed' };

	const output = $derived(item.output.replace(/\x1b\[[\d;]*m/g, ''));
</script>

{#snippet outcome()}
	<span class="outcome">
		{#if item.status === 'running' && item.startedAt}<Elapsed startedAt={item.startedAt} />{/if}
		{outcomes[item.status]}
	</span>
{/snippet}

<div class="entry {item.status}" class:full>
	{#if full}
		<div class="heading"><span>Command</span>{@render outcome()}</div>
		<CommandCode code={item.command} />
	{/if}
	{#if !full || output || item.elided}
		<details class={item.status} bind:open ontoggle={(event) => event.currentTarget.open && item.elided && onexpand?.()}>
			<summary>
				<span class="command">{#if full}Output{:else if open}Command{:else}$ <Code code={item.command} path="command.sh" />{/if}</span>
				{#if !full}{@render outcome()}{/if}
			</summary>
			{#if open && !full}<CommandCode code={item.command} />{/if}
			{#if output}<pre class="output">{output}</pre>{/if}
		</details>
	{/if}
</div>

<style>
	.entry {
		min-width: 0;
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	summary,
	.heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		min-height: var(--target-min);
	}

	summary {
		list-style: none;
		cursor: pointer;
	}

	.heading,
	.full summary {
		font-family: var(--font-ui);
		color: var(--muted-fg);
	}

	.full summary {
		justify-content: flex-start;
		list-style: disclosure-closed inside;
		display: list-item;
	}

	.full details[open] summary {
		list-style-type: disclosure-open;
	}

	.entry:not(.full) summary::-webkit-details-marker {
		display: none;
	}

	.command {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.outcome {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
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

	.output {
		max-height: 24em;
		margin: var(--space-1) 0 0;
		padding: var(--space-3);
		overflow: auto;
		border-radius: var(--radius-lg);
		background: var(--surface-1);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		tab-size: 4;
	}
</style>
