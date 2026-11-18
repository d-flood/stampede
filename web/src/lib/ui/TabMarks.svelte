<script lang="ts" module>
	import ShellMark from '#lib/ui/ShellMark.svelte';
	import { WaitingMark, WorkingMark } from '#lib/ui/spinners.ts';
	import CheckBold from '~icons/ph/check-bold';

	export type Status = 'working' | 'waiting' | 'background' | 'done';

	const marks = { working: WorkingMark, waiting: WaitingMark, background: ShellMark, done: CheckBold };
</script>

<script lang="ts">
	let { status, dirty = false }: { status?: Status; dirty?: boolean } = $props();
	const Mark = $derived(status && marks[status]);
</script>

{#if Mark}<span class="mark {status}" title={status}><Mark /></span>{/if}
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

	.dirty {
		flex: none;
		width: 0.5em;
		height: 0.5em;
		border-radius: 50%;
		background: currentColor;
	}
</style>
