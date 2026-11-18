<script lang="ts">
	import type { TabEntry } from '#lib/shell/Overview.svelte';

	let { tabs, current }: { tabs: TabEntry[]; current?: string } = $props();
	const index = $derived(tabs.findIndex(({ id }) => id === current));
</script>

<span class="dots" aria-hidden="true" style:--count={tabs.length} style:--index={index}>
	<span class="row">
		{#each tabs as tab (tab.id)}
			<span class="dot" class:now={tab.id === current} style:--mark={tab.status && `var(--status-${tab.status})`}></span>
		{/each}
	</span>
</span>

<style>
	.dots {
		--step: 0.75em;
		--fade: 0.5em;
		flex: 0 1 auto;
		container-type: inline-size;
		width: calc((var(--count) + 1) * var(--step) + 2 * var(--fade));
		min-width: 0;
		height: 0.5em;
		overflow: hidden;
		mask-image: linear-gradient(90deg, transparent, #000 var(--fade), #000 calc(100% - var(--fade)), transparent);
	}

	.row {
		display: flex;
		align-items: center;
		width: max-content;
		height: 100%;
		padding-inline: var(--fade);
		transform: translateX(
			clamp(
				min(0px, 100cqw - (var(--count) + 1) * var(--step) - 2 * var(--fade)),
				50cqw - var(--fade) - (var(--index) + 1) * var(--step),
				0px
			)
		);
		transition: transform var(--dur-fast) ease-out;
	}

	.dot {
		flex: none;
		width: 0.375em;
		height: 0.375em;
		margin-right: calc(var(--step) - 0.375em);
		border-radius: var(--radius-pill);
		background: var(--mark, var(--border-strong));
		transition: width var(--dur-fast) ease-out;
	}

	.now {
		width: calc(var(--step) + 0.375em);
		background: var(--color-fg);
	}
</style>
