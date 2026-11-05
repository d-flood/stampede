<script lang="ts">
	import type { SwitcherTab } from '#lib/shell/TabSwitcher.svelte';
	import { tabIcon } from '#lib/ui/Tab.svelte';

	let { tabs, current }: { tabs: SwitcherTab[]; current?: string } = $props();
	const index = $derived(tabs.findIndex(({ id }) => id === current));
</script>

<span class="icons" aria-hidden="true" style:--count={tabs.length} style:--index={index}>
	<span class="row">
		{#each tabs as tab (tab.id)}
			{@const Icon = tabIcon(tab.variant, tab.agent)}
			<span class="icon" class:now={tab.id === current} style:--mark={tab.status && `var(--status-${tab.status})`}><Icon /></span>
		{/each}
	</span>
</span>

<style>
	.icons {
		--step: 1.6em;
		--fade: 0.4em;
		flex: 0 1 auto;
		container-type: inline-size;
		width: calc(var(--count) * var(--step) + 2 * var(--fade));
		min-width: 0;
		height: 1.3em;
		overflow: hidden;
		mask-image: linear-gradient(90deg, transparent, #000 var(--fade), #000 calc(100% - var(--fade)), transparent);
	}

	.row {
		display: flex;
		width: max-content;
		padding-inline: var(--fade);
		transform: translateX(
			clamp(
				min(0px, 100cqw - var(--count) * var(--step) - 2 * var(--fade)),
				50cqw - var(--fade) - (var(--index) + 0.5) * var(--step),
				0px
			)
		);
		transition: transform var(--dur-fast) ease-out;
	}

	.icon {
		position: relative;
		display: inline-grid;
		flex: none;
		place-items: start center;
		width: var(--step);
		height: 1.3em;
		border-radius: var(--radius-sm);
		color: var(--muted-fg);
	}

	.icon :global(svg) {
		width: 1em;
		height: 1em;
	}

	.icon::after {
		position: absolute;
		right: 25%;
		bottom: 0;
		left: 25%;
		height: 2px;
		border-radius: 1px;
		background: var(--mark, transparent);
		content: '';
	}

	.now {
		background: var(--border);
		color: var(--color-fg);
	}
</style>
