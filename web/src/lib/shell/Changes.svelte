<script lang="ts">
	import { kinds, split, type Change } from '#lib/git.ts';

	let { changes, onopen }: { changes: Change[]; onopen(change: Change): void } = $props();

	const groups = $derived(
		kinds
			.map((group) => ({ ...group, changes: changes.filter((change) => change.kind === group.kind) }))
			.filter((group) => group.changes.length)
	);
</script>

<section aria-label="Changes">
	<header><h2>Uncommitted changes</h2></header>
	<div class="list">
		{#each groups as { kind, letter, label, changes } (kind)}
			<ul aria-label={label}>
				{#each changes as change (change.path)}
					{@const { name, dir } = split(change.path)}
					<li>
						<button
							class:deleted={kind === 'deleted'}
							style:--git="var(--git-{kind})"
							title={change.from ? `${change.from} → ${change.path}` : change.path}
							onclick={() => onopen(change)}
						>
							<span class="letter" aria-hidden="true">{letter}</span>
							<span class="name">{name}</span>
							<span class="dir">{dir}</span>
						</button>
					</li>
				{/each}
			</ul>
		{:else}
			<p>No uncommitted changes</p>
		{/each}
	</div>
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

	.list {
		flex: 1;
		overflow: auto;
		padding-bottom: var(--space-3);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	button {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-3);
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		white-space: nowrap;
		cursor: pointer;
	}

	button:hover {
		background: var(--hover);
	}

	button:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: calc(-1 * var(--border-width));
	}

	.letter {
		flex: none;
		width: 1ch;
		color: var(--git);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-weight: 700;
	}

	.deleted .name {
		text-decoration: line-through;
	}

	.name {
		flex: none;
		max-width: 70%;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.dir {
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
	}

	p {
		margin: 0;
		padding: var(--space-2) var(--space-3);
		color: var(--muted-fg);
	}
</style>
