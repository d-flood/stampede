<script lang="ts">
	import { agentNames } from '#lib/chat/sessions.ts';
	import type { Agent } from '#lib/config.svelte.ts';
	import type { Item, ResetCredits } from '#lib/device.svelte.ts';
	import CodexResets from '#lib/shell/CodexResets.svelte';
	import Button from '#lib/ui/Button.svelte';

	let { item, agent, resets = null, onhandoff }: { item: Extract<Item, { kind: 'notice' }>; agent?: Agent; resets?: ResetCredits | null; onhandoff?(): void } = $props();
	const name = $derived(agent ? agentNames[agent] : 'Agent');
</script>

{#if item.notice === 'error'}
	<div class="notice card error" role="alert">
		<strong>{name} error</strong>
		<p>{item.text}</p>
	</div>
{:else if item.notice === 'limit'}
	<div class="notice card limit">
		<strong>{name} usage limit reached</strong>
		<p>{item.text}</p>
		<div class="actions">
			{#if agent === 'codex' && resets}<CodexResets credits={resets} now={Date.now()} label="Use a reset" />{/if}
			<Button disabled={!onhandoff} onclick={() => onhandoff?.()}>Continue with another Rig</Button>
		</div>
	</div>
{:else}
	<div class="notice {item.notice}">{item.text}</div>
{/if}

<style>
	.notice {
		font-size: 0.92em;
		overflow-wrap: anywhere;
	}

	.compacted,
	.interrupted {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		color: var(--muted-fg);
	}

	.compacted::before,
	.compacted::after,
	.interrupted::before,
	.interrupted::after {
		content: '';
		flex: 1;
		border-top: var(--border-width) solid var(--border);
	}

	.speed {
		padding: var(--space-2) var(--space-3);
		border-left: 2px solid var(--status-waiting);
		border-radius: var(--radius-md);
		background: var(--surface-1);
	}

	.card {
		--tint: var(--ansi-1);
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-3) var(--space-4);
		border: var(--border-width) solid color-mix(in oklab, var(--tint) 35%, var(--border));
		border-radius: var(--radius-lg);
		background: color-mix(in oklab, var(--tint) 7%, var(--color-bg));
		font-family: var(--font-ui);
	}

	.limit {
		--tint: var(--status-waiting);
	}

	strong {
		font-weight: 600;
	}

	p {
		margin: 0;
		color: var(--muted-fg);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		margin-top: var(--space-2);
	}
</style>
