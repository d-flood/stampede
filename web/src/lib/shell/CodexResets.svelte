<script lang="ts">
	import type { ResetCredits } from '#lib/device.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Tooltip from '#lib/ui/Tooltip.svelte';

	let { credits, now }: { credits: ResetCredits; now: number } = $props();
	let confirming = $state(false);
	let busy = $state(false);
	let error = $state('');
	let message = $state('');
	let attempt = $state<{ idempotencyKey: string; creditId?: string }>();
	const expiry = $derived.by(() => {
		if (credits.expiresAt === undefined) return '';
		const minutes = Math.max(0, Math.ceil((credits.expiresAt - now) / 60_000));
		if (!minutes) return 'Expires now';
		if (minutes >= 1440) return `Expires in ${Math.ceil(minutes / 1440)}d`;
		if (minutes >= 60) return `Expires in ${Math.ceil(minutes / 60)}h`;
		return `Expires in ${minutes}m`;
	});

	async function redeem() {
		if (busy) return;
		busy = true;
		error = '';
		message = '';
		attempt ??= { idempotencyKey: crypto.randomUUID(), creditId: credits.creditId };
		try {
			const response = await fetch('/api/usage/codex/reset', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(attempt)
			});
			if (!response.ok) throw new Error(await response.text());
			const outcome: 'reset' | 'alreadyRedeemed' | 'nothingToReset' | 'noCredit' = await response.json();
			message = outcome === 'reset' || outcome === 'alreadyRedeemed' ? 'Reset applied' : outcome === 'nothingToReset' ? 'No eligible limit to reset' : 'No resets available';
			attempt = undefined;
			confirming = false;
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Reset failed. Try again.';
		} finally {
			busy = false;
		}
	}
</script>

<div class="resets" aria-label="Codex resets">
	<div class="summary">
		<span>{credits.availableCount} {credits.availableCount === 1 ? 'reset' : 'resets'}</span>
		{#if credits.availableCount > 0 && credits.expiresAt !== undefined}
			{@const expiresAt = credits.expiresAt}
			<span aria-hidden="true">·</span>
			<Tooltip text={`Next reset expires ${new Date(expiresAt).toLocaleString()}`}>
				{#snippet children(props)}
					<time {...props} datetime={new Date(expiresAt).toISOString()}>{expiry}</time>
				{/snippet}
			</Tooltip>
		{/if}
	</div>
	<Button disabled={!credits.availableCount || busy || confirming} aria-label="Use a Codex reset" onclick={() => { confirming = true; message = ''; }}>Use</Button>
	{#if confirming}
		<div class="confirmation">
			<span>Use 1 reset?</span>
			<Button variant="ghost" disabled={busy} onclick={() => { confirming = false; error = ''; }}>Cancel</Button>
			<Button variant="primary" disabled={busy} onclick={redeem}>{busy ? 'Using…' : attempt ? 'Retry' : 'Confirm'}</Button>
		</div>
	{/if}
	{#if error}<div class="error" role="alert">{error}</div>{/if}
	{#if message}<div class="message" role="status">{message}</div>{/if}
</div>

<style>
	.resets {
		display: flex;
		grid-column: 1 / -1;
		flex-wrap: wrap;
		align-items: center;
		gap: 0 var(--space-2);
		margin-top: var(--space-1);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.summary {
		display: flex;
		flex: 1;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0 var(--space-2);
		min-width: 0;
	}

	.summary > span:first-child,
	time {
		white-space: nowrap;
	}

	.resets :global(button) {
		padding: 0 var(--space-2);
		font-size: var(--font-size-sm);
	}

	.confirmation {
		display: flex;
		flex-basis: 100%;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		margin-top: var(--space-1);
	}

	.confirmation span {
		flex: 1;
	}

	.error,
	.message {
		flex-basis: 100%;
		overflow-wrap: anywhere;
	}

	.error {
		color: var(--ansi-1);
	}
</style>
