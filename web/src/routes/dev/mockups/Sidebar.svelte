<script lang="ts">
	import Badge from '#lib/ui/Badge.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import ArrowsClockwise from '~icons/ph/arrows-clockwise';
	import { changes } from './data.ts';

	const sections = ['Files', 'Changes', 'History', 'Search'];
</script>

<div class="sidebar">
	<div class="sections" role="tablist" aria-label="Sidebar">
		{#each sections as section (section)}
			<button role="tab" aria-selected={section === 'Changes'}>
				{section}
				{#if section === 'Changes'}<Badge>{changes.length}</Badge>{/if}
			</button>
		{/each}
	</div>
	<header>
		<h2>Uncommitted changes</h2>
		<IconButton label="Refresh"><ArrowsClockwise /></IconButton>
	</header>
	<ul>
		{#each changes as change, n (change.dir + change.name)}
			<li class:selected={n === 0}>
				<span class="status s-{change.status}">{change.status}</span>
				<span class="name">{change.name}</span>
				<span class="dir">{change.dir}</span>
				<span class="counts">
					{#if change.added}<span class="added">+{change.added}</span>{/if}
					{#if change.removed}<span class="removed">−{change.removed}</span>{/if}
				</span>
			</li>
		{/each}
	</ul>
</div>

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		min-height: 0;
		font-size: var(--font-size-md);
	}

	.sections {
		display: grid;
		grid-template-columns: repeat(4, auto);
		gap: var(--space-1);
		margin: var(--space-2);
		padding: var(--space-1);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
	}

	.sections button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		cursor: pointer;
	}

	.sections [aria-selected='true'] {
		background: var(--color-bg);
		color: var(--color-fg);
		font-weight: 600;
	}

	header {
		display: flex;
		align-items: center;
		padding: 0 var(--space-1) 0 var(--space-4);
	}

	h2 {
		flex: 1;
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	ul {
		margin: 0;
		padding: 0 var(--space-2);
		list-style: none;
	}

	li {
		display: grid;
		grid-template-columns: 1.4em auto minmax(0, 1fr) auto;
		align-items: baseline;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		line-height: var(--target-min);
	}

	.selected {
		background: var(--hover);
	}

	.status {
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-weight: 700;
		text-align: center;
	}

	.s-M {
		color: var(--ansi-3);
	}

	.s-A,
	.added {
		color: var(--ansi-2);
	}

	.s-D,
	.removed {
		color: var(--ansi-1);
	}

	.s-D ~ .name {
		text-decoration: line-through;
	}

	.dir {
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.counts {
		display: flex;
		gap: var(--space-1);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}
</style>
