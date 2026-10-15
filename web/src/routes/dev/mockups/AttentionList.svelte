<script lang="ts">
	import Badge from '#lib/ui/Badge.svelte';
	import Claude from '~icons/simple-icons/claude';
	import { attention } from './data.ts';
</script>

<div class="attention" role="menu" aria-label="Needs attention">
	<header>Needs attention</header>
	{#each attention as item (item.tab.label)}
		<div class="item" role="menuitem" tabindex="-1">
			<span class="logo"><Claude /></span>
			<span class="what">
				<span class="label"><b>{item.tab.label}</b> <span class="muted">in {item.project}</span></span>
				<span class="title">{item.title}</span>
				<span class="muted">{item.note}</span>
			</span>
			<span class="when">
				<Badge tone={item.tab.status}>{item.tab.status}</Badge>
				<span class="muted">{item.time}</span>
			</span>
		</div>
	{/each}
</div>

<style>
	.attention {
		box-sizing: border-box;
		width: 22rem;
		padding: var(--space-1);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--surface-2);
		box-shadow: var(--shadow-float);
		color: var(--color-fg);
		font-size: var(--font-size-md);
	}

	header {
		padding: var(--space-2) var(--space-2) var(--space-1);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.item {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		gap: var(--space-2);
		padding: var(--space-2);
		border-radius: var(--radius-md);
	}

	.item:first-of-type {
		background: var(--hover);
	}

	.logo {
		font-size: var(--font-size-lg);
		line-height: 1;
	}

	.what,
	.when {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.when {
		align-items: flex-end;
	}

	.title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.muted {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}
</style>
