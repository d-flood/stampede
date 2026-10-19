<script lang="ts">
	import type { Epic } from '#lib/device.svelte.ts';
	import Progress from '#lib/epics/Progress.svelte';
	import { counts } from '#lib/epics.svelte.ts';

	let { epics, onopen }: { epics: Epic[]; onopen(slug: string): void } = $props();
	let showArchived = $state(false);

	const listed = $derived(epics.filter((epic) => showArchived || !epic.archived));
</script>

<section aria-label="Epics">
	<header>
		<h2>Epics</h2>
		<label><input type="checkbox" bind:checked={showArchived} /> Show archived</label>
	</header>
	{#if listed.length}
		<ul>
			{#each listed as epic (epic.slug)}
				{@const tally = counts(epic)}
				<li>
					<button onclick={() => onopen(epic.slug)}>
						<span class="line">
							<span class="slug">{epic.slug}</span>
							{#if epic.archived}<span class="archived">Archived</span>{/if}
							{#if epic.run}<span class="live" class:paused={!!epic.run.paused}>{epic.run.paused ? 'Run paused' : 'Run live'}</span>{/if}
						</span>
						<span class="title">{epic.title}</span>
						<Progress {epic} />
						<span class="line summary">
							<span>
								{tally.merged + tally.skipped} of {epic.tickets.length} resolved{#if tally['in-progress']}{' '}· {tally['in-progress']} in progress{/if}
							</span>
							{#if epic.run}<span>{epic.run.rig} · ×{epic.run.parallel}</span>{/if}
						</span>
					</button>
				</li>
			{/each}
		</ul>
	{:else}
		{#if epics.length}
			<p>No active Epics.</p>
		{:else}
			<p>No Epics yet. Agents add them with <code>stampede epic add</code>.</p>
		{/if}
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
		justify-content: space-between;
		flex-wrap: wrap;
		gap: 0 var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-3);
	}

	h2 {
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	label {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		cursor: pointer;
	}

	input {
		accent-color: var(--status-working);
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

	.line {
		display: flex;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.slug {
		font-weight: 600;
	}

	.live {
		color: var(--status-done);
		font-size: var(--font-size-sm);
	}

	.paused {
		color: var(--status-waiting);
	}

	.archived {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
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
