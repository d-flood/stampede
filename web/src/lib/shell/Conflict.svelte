<script lang="ts">
	import type { Sync } from '#lib/editor/sync.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Warning from '~icons/ph/warning';

	let { sync, onshowdiff }: { sync: Sync; onshowdiff(disk: string): void } = $props();
</script>

{#if sync.conflict}
	{@const disk = sync.conflict}
	<div class="banner" role="alert">
		<span class="message"><Warning /> Changed on disk while you have unsaved edits.</span>
		<div class="choices">
			<Button onclick={() => sync.keepMine()}>Keep mine</Button>
			<Button onclick={() => sync.load(disk)}>Load disk</Button>
			<Button onclick={() => onshowdiff(disk.content)}>Show diff</Button>
		</div>
	</div>
{/if}

<style>
	.banner {
		display: flex;
		flex: none;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		padding: var(--space-2) var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
		background: color-mix(in oklab, var(--status-waiting) 16%, var(--color-bg));
	}

	.message {
		display: inline-flex;
		flex: 1 1 16rem;
		align-items: center;
		gap: var(--space-2);
	}

	.message :global(svg) {
		flex: none;
		color: var(--status-waiting);
	}

	.choices {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
