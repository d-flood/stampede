<script lang="ts">
	import type { Status } from '#lib/device.svelte.ts';
	import { WaitingMark, WorkingMark } from '#lib/ui/spinners.ts';

	let { status, activity }: { status: Status; activity?: string | null } = $props();
</script>

{#if status !== 'done'}
	<div class="activity" class:waiting={status === 'waiting'} role="status" aria-label="Current activity">
		{#if status === 'waiting'}<WaitingMark />{:else}<WorkingMark />{/if}
		<span class="label">{status === 'waiting' ? 'Waiting for your response' : activity || 'Working…'}</span>
	</div>
{/if}

<style>
	.activity { --pulse: 0; display: flex; align-items: center; gap: var(--space-2); min-height: var(--target-min); color: var(--status-working); font-size: var(--font-size-sm); }
	.waiting { color: var(--status-waiting); }
	.label { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
