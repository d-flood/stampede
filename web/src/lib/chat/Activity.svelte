<script lang="ts">
	import type { Status } from '#lib/device.svelte.ts';
	import CircleNotchBold from '~icons/ph/circle-notch-bold';
	import HandPalmFill from '~icons/ph/hand-palm-fill';

	let { status, activity }: { status: Status; activity?: string | null } = $props();
</script>

{#if status !== 'done'}
	<div class="activity" class:waiting={status === 'waiting'} role="status" aria-label="Current activity">
		{#if status === 'waiting'}<HandPalmFill />{:else}<span class="spinner"><CircleNotchBold /></span>{/if}
		<span class="label">{status === 'waiting' ? 'Waiting for your response' : activity || 'Working…'}</span>
	</div>
{/if}

<style>
	.activity { display: flex; align-items: center; gap: var(--space-2); min-height: var(--target-min); color: var(--status-working); font-size: var(--font-size-sm); }
	.waiting { color: var(--status-waiting); }
	.spinner { display: inline-flex; flex: none; animation: spin 1s linear var(--pulse); }
	.label { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	@keyframes spin { to { transform: rotate(1turn); } }
</style>
