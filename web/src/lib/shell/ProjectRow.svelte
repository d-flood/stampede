<script lang="ts">
	import type { Epic, Project } from '#lib/device.svelte.ts';
	import Progress from '#lib/epics/Progress.svelte';
	import { counts } from '#lib/epics.svelte.ts';
	import type { Tile } from '#lib/projects.ts';
	import ProjectIcon from '#lib/shell/ProjectIcon.svelte';
	import Badge from '#lib/ui/Badge.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import CaretDown from '~icons/ph/caret-down';
	import CaretRight from '~icons/ph/caret-right';
	import CircleNotchBold from '~icons/ph/circle-notch-bold';

	type Props = {
		project: Project;
		label: string;
		labels: Map<string, string>;
		tiles: Tile[];
		epics: Epic[];
		root: boolean;
		working?: boolean;
		needs?: number;
		compact?: boolean;
		gutter?: boolean;
		focused?: string;
		dragging?: boolean;
		onfocus(id: string): void;
		onclose(id: string): void;
		ondelete(id: string): void;
		onworktree(project: Project): void;
		onmenu(id: string): void;
		onicon(project: Project): void;
	};

	let { project, label, labels, tiles, epics, root, working = false, needs = 0, compact = false, gutter = false, focused, dragging = false, onfocus, onclose, ondelete, onworktree, onmenu, onicon }: Props =
		$props();

	let folded = $state(false);
	const runs = $derived(epics.filter((epic) => epic.run && !epic.archived));
	const active = $derived(project.id === focused || tiles.some((tile) => tile.id === focused));
	const listed = $derived(tiles.length > 0 && (compact ? active : !folded));

	function select(event: MouseEvent & { currentTarget: HTMLElement }, id: string) {
		// A long-press opens the menu on a timer, and its release still clicks.
		if (event.currentTarget.dataset.state !== 'open') onfocus(id);
	}
</script>

{#snippet worktrees()}
	<div class="worktrees" role="group" aria-label="Worktrees of {label}">
		{#each tiles as tile (tile.id)}
			{@const name = labels.get(tile.id) ?? tile.mark}
			<Menu context items={[{ label: 'Delete worktree', onselect: () => ondelete(tile.id) }]}>
				{#snippet trigger(props)}
					<button
						{...props}
						tabindex={0}
						class="worktree"
						class:focused={tile.id === focused}
						aria-label={tile.status ? `${name}, ${tile.status}` : name}
						aria-current={tile.id === focused}
						title={tile.status ? `${name} ${tile.status}` : name}
						onclick={(event) => select(event, tile.id)}
					>
						<span class="tile {tile.status ?? ''}">{tile.mark}</span>
						{#if !compact}<span class="name">{name}</span>{/if}
					</button>
				{/snippet}
			</Menu>
		{/each}
	</div>
{/snippet}

<li class={compact ? 'compact' : 'card'}>
	<div class="head" class:open={compact && listed}>
		<Menu context items={[
			{ label: 'Change project icon…', onselect: () => onicon(project) },
			...(root ? [{ label: 'New worktree…', onselect: () => onworktree(project) }] : []),
			{ label: 'Close', onselect: () => onclose(project.id) }
		]} onopen={() => onmenu(project.id)}>
			{#snippet trigger(props)}
				<button
					{...props}
					tabindex={0}
					class="project"
					class:focused={project.id === focused}
					class:dragging
					aria-label={label}
					aria-current={project.id === focused}
					title={compact ? label : undefined}
					data-project={project.id}
					onclick={(event) => select(event, project.id)}
				>
					<span class="square">
						<ProjectIcon {project} />
						{#if compact && needs}<span class="attention">{needs}</span>{/if}
						{#if tiles.length}<span class="count" title="{tiles.length} {tiles.length === 1 ? 'worktree' : 'worktrees'}">{tiles.length}</span>{/if}
						{#if working}<span class="spinner" title="working"><CircleNotchBold /></span>{/if}
					</span>
					{#if !compact}
						<span class="text">
							<strong>{label}</strong>
							<span class="path">{project.path}</span>
							{#each runs as epic (epic.slug)}
								{@const tally = counts(epic)}
								<span class="run">Run {epic.slug} · <span class="resolved">{tally.merged + tally.skipped} of {epic.tickets.length}</span></span>
								<Progress {epic} />
							{/each}
						</span>
						{#if needs}<Badge tone="waiting">{needs}</Badge>{/if}
					{/if}
				</button>
			{/snippet}
		</Menu>
		{#if compact && listed}
			{@render worktrees()}
		{:else if !compact && tiles.length}
			<IconButton label="{folded ? 'Show' : 'Hide'} worktrees of {label}" aria-expanded={!folded} onclick={() => (folded = !folded)}>
				{#if folded}<CaretRight />{:else}<CaretDown />{/if}
			</IconButton>
		{:else if !compact && gutter}
			<span class="gutter"></span>
		{/if}
	</div>
	{#if compact}
		{#each runs as epic (epic.slug)}
			{@const tally = counts(epic)}
			<span class="progress" title="Run {epic.slug}: {tally.merged + tally.skipped} of {epic.tickets.length} resolved">
				{tally.merged + tally.skipped}/{epic.tickets.length}
			</span>
		{/each}
	{:else if listed}
		{@render worktrees()}
	{/if}
</li>

<style>
	li {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}

	.compact {
		--square: calc(var(--target-min) + var(--space-2));

		align-items: center;
	}

	.card {
		--square: var(--target-min);
	}

	.head {
		display: flex;
		gap: var(--space-2);
	}

	.compact .head {
		flex-direction: column;
		align-items: center;
		gap: var(--space-1);
		border-radius: var(--radius-lg);
	}

	.compact .head.open {
		margin: calc(-1 * var(--space-1));
		padding: var(--space-1);
		background: var(--surface-3);
		box-shadow: inset 0 0 0 var(--border-width) var(--border);
	}

	.card .head {
		align-items: center;
	}

	.project {
		position: relative;
		padding: 0;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		font: inherit;
		cursor: pointer;
	}

	.compact .project {
		width: var(--square);
		height: var(--square);
		color: var(--muted-fg);
		font-weight: 600;
		touch-action: none;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	.compact .project:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.compact .project.focused {
		border-color: var(--color-fg);
		background: var(--color-bg);
		color: var(--color-fg);
	}

	.compact .project.focused::before {
		content: '';
		position: absolute;
		top: 20%;
		bottom: 20%;
		left: calc(-1 * var(--space-2) - var(--border-width));
		width: 3px;
		border-radius: 0 3px 3px 0;
		background: var(--color-fg);
	}

	.card .project {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-3);
		box-sizing: border-box;
		min-width: 0;
		padding: var(--space-2);
		color: var(--color-fg);
		text-align: start;
	}

	.card .project.focused {
		border-color: var(--color-fg);
	}

	.project:focus-visible,
	.worktree:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	.project.dragging {
		box-shadow: var(--shadow-float);
		cursor: grabbing;
	}

	.square {
		position: relative;
		display: grid;
		flex: none;
		place-items: center;
		border-radius: inherit;
	}

	.compact .square {
		width: 100%;
		height: 100%;
	}

	.card .square {
		width: var(--square);
		height: var(--square);
		border-radius: var(--radius-md);
		background: var(--surface-3);
		font-weight: 600;
	}

	.attention,
	.count,
	.spinner {
		position: absolute;
		display: grid;
		place-items: center;
		box-sizing: border-box;
		min-width: 1.4em;
		height: 1.4em;
		border: var(--border-width) solid var(--surface-2);
		border-radius: 0.7em;
		background: var(--surface-1);
		font-size: calc(var(--font-size-sm) - 1px);
	}

	.attention,
	.count {
		padding: 0 0.3em;
		font-variant-numeric: tabular-nums;
		font-weight: 600;
		line-height: 1;
	}

	.attention {
		top: calc(-1 * var(--space-1) - var(--border-width));
		right: calc(-1 * var(--space-1) - var(--border-width));
		background: var(--status-waiting);
		color: var(--color-bg);
	}

	.count {
		bottom: calc(-1 * var(--space-1) - var(--border-width));
		left: calc(-1 * var(--space-1) - var(--border-width));
		color: var(--muted-fg);
	}

	.spinner {
		right: calc(-1 * var(--space-1) - var(--border-width));
		bottom: calc(-1 * var(--space-1) - var(--border-width));
		color: var(--status-working);
	}

	.spinner :global(svg) {
		animation: spin 1s linear var(--pulse);
	}

	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}

	.gutter {
		flex: none;
		width: var(--target-min);
	}

	.text {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}

	.path {
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.run {
		font-size: var(--font-size-sm);
	}

	.resolved {
		color: var(--status-done);
	}

	.progress {
		color: var(--status-done);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
	}

	.worktrees {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.compact .worktrees {
		align-items: center;
	}

	.card .worktrees {
		margin-left: calc(var(--space-2) + var(--square) / 2);
		padding-left: var(--space-2);
		border-left: var(--border-width) solid var(--border);
	}

	.worktree {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		padding: 0;
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: start;
		cursor: pointer;
	}

	.card .worktree {
		min-height: var(--target-min);
		padding-right: var(--space-2);
	}

	.card .worktree:hover {
		background: var(--hover);
	}

	.card .worktree.focused {
		background: var(--color-bg);
	}

	.tile {
		display: grid;
		flex: none;
		place-items: center;
		box-sizing: border-box;
		border: var(--border-width) solid var(--muted-fg);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		font-size: var(--font-size-sm);
		font-weight: 700;
	}

	.compact .tile {
		width: calc(var(--square) - 4px);
		height: calc(var(--square) - 4px);
	}

	.card .tile {
		width: 2.2em;
		height: 2.2em;
	}

	.compact .worktree:hover .tile {
		background: var(--hover);
	}

	.compact .worktree.focused .tile {
		outline: var(--border-width) solid var(--color-fg);
		outline-offset: var(--border-width);
	}

	.tile.working,
	.tile.waiting,
	.tile.done {
		border-width: 2px;
		border-color: currentColor;
	}

	.tile.working {
		color: var(--status-working);
	}

	.tile.waiting {
		color: var(--status-waiting);
	}

	.tile.done {
		color: var(--status-done);
	}

	.tile.stopping {
		border-style: dashed;
		color: var(--muted-fg);
	}

	.name {
		overflow: hidden;
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
