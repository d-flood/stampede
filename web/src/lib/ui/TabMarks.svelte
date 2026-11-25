<script lang="ts" module>
	import ShellMark from '#lib/ui/ShellMark.svelte';
	import { WaitingMark, WorkingMark } from '#lib/ui/spinners.ts';
	import CheckBold from '~icons/ph/check-bold';

	export type Status = 'working' | 'waiting' | 'background' | 'done';

	const marks = { working: WorkingMark, waiting: WaitingMark, background: ShellMark, done: CheckBold };
</script>

<script lang="ts">
	let { status, dirty = false, fresh = false }: { status?: Status; dirty?: boolean; fresh?: boolean } = $props();
	const Mark = $derived(status && marks[status]);
</script>

{#if status === 'done' && fresh}<span class="mark ready" title={status}><span class="dot"></span></span>{:else if Mark}<span class="mark {status}" title={status}><Mark /></span>{/if}
{#if dirty}<span class="dirty" title="Unsaved changes"></span>{/if}

<style>
	.mark {
		display: inline-flex;
		flex: none;
		color: var(--mark);
	}

	.working {
		--mark: var(--status-working);
	}

	.waiting {
		--mark: var(--status-waiting);
	}

	.background {
		--mark: var(--status-background);
	}

	.done {
		--mark: var(--status-done);
	}

	.ready {
		align-items: center;
		justify-content: center;
		width: 1em;
		height: 1em;
	}

	.ready .dot {
		width: 0.4em;
		height: 0.4em;
		border-radius: 50%;
		background: var(--muted-fg);
	}

	.dirty {
		flex: none;
		width: 0.5em;
		height: 0.5em;
		border-radius: 50%;
		background: currentColor;
	}
</style>
