<script lang="ts">
	import type { ToolCall } from '#lib/device.svelte.ts';
	import Caret from '#lib/ui/Caret.svelte';

	const labels: Record<string, string> = {
		approved: 'Approved',
		denied: 'Denied',
		timedOut: 'Timed out',
		aborted: 'Aborted'
	};

	let { call, open = $bindable(false) }: { call: ToolCall; open?: boolean } = $props();
	const label = $derived(labels[call.name] ?? call.name);
	const blocked = $derived(call.name !== 'approved');
</script>

{#if call.detail || call.rationale}
	<details class="approval" class:blocked bind:open>
		<summary>
			<Caret /><span class="verdict">{label}</span>
			{#if !open && call.detail}<span class="preview" title={call.detail}>{call.detail}</span>{/if}
		</summary>
		{#if open}
			<div class="body">
				{#if call.detail}<pre class="action">{call.detail}</pre>{/if}
				{#if call.rationale}<p class="rationale">{call.rationale}</p>{/if}
			</div>
		{/if}
	</details>
{:else}
	<div class="plain" class:blocked><span class="verdict">{label}</span></div>
{/if}

<style>
	.approval {
		min-width: 0;
	}

	summary,
	.plain {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
	}

	summary {
		list-style: none;
		cursor: pointer;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	.verdict {
		flex: none;
		color: var(--color-fg);
	}

	.blocked .verdict {
		color: var(--ansi-1);
	}

	.preview {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.body {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin-top: var(--space-1);
		padding: var(--space-3);
		border-radius: var(--radius-lg);
		background: var(--surface-1);
	}

	.action {
		margin: 0;
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		tab-size: 4;
	}

	.rationale {
		margin: 0;
		color: var(--muted-fg);
		font-family: var(--font-ui);
		font-size: var(--font-size-sm);
		line-height: 1.45;
		overflow-wrap: anywhere;
	}
</style>
