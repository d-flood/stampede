<script lang="ts">
	import type { Epic } from '#lib/device.svelte.ts';
	import Progress from '#lib/epics/Progress.svelte';
	import { counts } from '#lib/epics.svelte.ts';

	let { epics, onopen }: { epics: Epic[]; onopen(slug: string): void } = $props();

	const listed = $derived(epics.filter((epic) => !epic.archived));
</script>

<section aria-label="Epics">
	<header><h2>Epics</h2></header>
	{#if listed.length}
		<ul>
			{#each listed as epic (epic.slug)}
				{@const tally = counts(epic)}
				<li>
					<button onclick={() => onopen(epic.slug)}>
						<span class="slug">{epic.slug}</span>
						<span class="title">{epic.title}</span>
						<Progress {epic} />
						<span class="summary">
							{tally.merged + tally.skipped} of {epic.tickets.length} resolved{#if tally['in-progress']}{' '}· {tally['in-progress']} in progress{/if}
						</span>
					</button>
				</li>
			{/each}
		</ul>
	{:else}
		<p>No Epics yet. Agents add them with <code>oikonomos epic add</code>.</p>
	{/if}
</section>

<style>
	section {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	header {
		display: flex;
		align-items: center;
		min-height: var(--target-min);
		padding: 0 var(--space-3);
	}

	h2 {
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	ul {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0 var(--space-2) var(--space-3);
		overflow: auto;
		list-style: none;
	}

	button {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		width: 100%;
		padding: var(--space-3);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	button:hover {
		background: var(--hover);
	}

	button:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: calc(-1 * var(--border-width));
	}

	.slug {
		font-weight: 600;
	}

	.title {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.summary {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
	}

	p {
		margin: 0;
		padding: var(--space-2) var(--space-3);
		color: var(--muted-fg);
	}

	code {
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}
</style>
