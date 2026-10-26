<script lang="ts">
	import type { Epic, Project } from '#lib/device.svelte.ts';
	import Progress from '#lib/epics/Progress.svelte';
	import { counts } from '#lib/epics.svelte.ts';
	import type { Tile } from '#lib/projects.ts';
	import ProjectIcon from '#lib/shell/ProjectIcon.svelte';
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

<li class={compact ? 'compact' : 'card'} class:tray={compact && listed}>
	<div class="head" class:reserve={!compact && gutter}>
		<Menu context items={[
			{ label: 'Change project icon…', onselect: () => onicon(project) },
			...(root ? [{ label: 'New worktree…', onselect: () => onworktree(project) }] : []),
			{ label: 'Close', onselect: () => onclose(project.id) }
		]} onopen={() => onmenu(project.id)}>
			{#snippet trigger(props)}
				<button
					{...props}
					tabindex={0}
					class="project squircle"
					class:focused={project.id === focused}
					class:dragging
					aria-label={label}
					aria-current={project.id === focused}
					title={compact ? label : undefined}
					data-project={project.id}
					onclick={(event) => select(event, project.id)}
				>
					<span class="square squircle">
						<ProjectIcon {project} />
						{#if needs}<span class="attention">{needs}</span>{/if}
						{#if tiles.length}<span class="count" title="{tiles.length} {tiles.length === 1 ? 'worktree' : 'worktrees'}">{tiles.length}</span>{/if}
						{#if working}<span class="spinner" title="working"><CircleNotchBold /></span>{/if}
					</span>
					<span class="text">
						<strong>{label}</strong>
						<span class="path">{project.path}</span>
						{#each runs as epic (epic.slug)}
							{@const tally = counts(epic)}
							<span class="run">Run {epic.slug} · <span class="resolved">{tally.merged + tally.skipped} of {epic.tickets.length}</span></span>
							<Progress {epic} />
						{/each}
					</span>
				</button>
			{/snippet}
		</Menu>
		{#if !compact && tiles.length}
			<span class="fold">
				<IconButton label="{folded ? 'Show' : 'Hide'} worktrees of {label}" aria-expanded={!folded} onclick={() => (folded = !folded)}>
					{#if folded}<CaretRight />{:else}<CaretDown />{/if}
				</IconButton>
			</span>
		{/if}
	</div>
	{#if tiles.length}
		<div class="worktrees" class:shut={!listed} role="group" aria-label="Worktrees of {label}">
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
							<span class="tile squircle {tile.status ?? ''}">{tile.mark}</span>
							<span class="name">{name}</span>
						</button>
					{/snippet}
				</Menu>
			{/each}
		</div>
	{/if}
	{#if compact}
		{#each runs as epic (epic.slug)}
			{@const tally = counts(epic)}
			<span class="progress" title="Run {epic.slug}: {tally.merged + tally.skipped} of {epic.tickets.length} resolved">
				{tally.merged + tally.skipped}/{epic.tickets.length}
			</span>
		{/each}
	{/if}
</li>

<style>
	li {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
		border-radius: calc(var(--radius-tile) + var(--space-1));
		interpolate-size: allow-keywords;
		transition:
			margin var(--dur-med) var(--ease-out),
			padding var(--dur-med) var(--ease-out),
			background var(--dur-fast),
			box-shadow var(--dur-fast);
	}

	.compact {
		--square: var(--tile);
	}

	.card {
		--square: calc(var(--target-min) + 8px);
	}

	@media (pointer: coarse) {
		.card {
			--square: var(--target-min);
		}
	}

	.tray {
		margin: calc(-1 * var(--space-1));
		padding: var(--space-1);
		background: var(--surface-1);
		box-shadow: inset 0 0 0 var(--border-width) var(--border);
	}

	.head {
		position: relative;
		display: flex;
		align-items: center;
		transition: padding var(--dur-med) var(--ease-out);
	}

	.reserve {
		padding-right: calc(var(--target-min) + var(--space-2));
	}

	.fold {
		position: absolute;
		top: 50%;
		right: 0;
		translate: 0 -50%;
		animation: appear var(--dur-med) ease-out var(--dur-fast) backwards;
	}

	.project {
		position: relative;
		display: flex;
		align-items: center;
		gap: 0;
		box-sizing: border-box;
		width: var(--square);
		height: var(--square);
		min-width: 0;
		padding: 0;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-tile);
		background: var(--surface-1);
		color: var(--muted-fg);
		font: inherit;
		text-align: start;
		cursor: pointer;
		transition:
			width var(--dur-med) var(--ease-out),
			height var(--dur-med) var(--ease-out),
			padding var(--dur-med) var(--ease-out),
			gap var(--dur-med) var(--ease-out),
			border-radius var(--dur-med) var(--ease-out),
			background var(--dur-fast),
			border-color var(--dur-fast),
			color var(--dur-fast);
	}

	.compact .project {
		touch-action: none;
	}

	.card .project {
		gap: var(--space-3);
		width: 100%;
		height: auto;
		padding: var(--space-2);
		border-radius: var(--radius-lg);
		color: var(--color-fg);
	}

	.project:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.project.focused {
		border-color: color-mix(in oklab, var(--color-fg) 38%, var(--color-bg));
		background: var(--surface-3);
		color: var(--color-fg);
	}

	.project::before {
		content: '';
		position: absolute;
		top: 22%;
		bottom: 22%;
		left: calc(-1 * var(--space-3) - var(--border-width));
		width: 4px;
		border-radius: 0 4px 4px 0;
		background: var(--color-fg);
		opacity: 0;
		scale: 1 0;
		transition:
			opacity var(--dur-fast),
			scale var(--dur-med) var(--ease-out);
	}

	.compact .project.focused::before {
		opacity: 1;
		scale: 1;
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
		width: calc(var(--square) - 2 * var(--border-width));
		height: calc(var(--square) - 2 * var(--border-width));
		border-radius: calc(var(--radius-tile) - var(--border-width));
		font-weight: 600;
		transition:
			width var(--dur-med) var(--ease-out),
			height var(--dur-med) var(--ease-out),
			background var(--dur-med);
	}

	.card .square {
		width: var(--square);
		height: var(--square);
		background: var(--surface-3);
	}

	.card .focused .square {
		background: var(--color-bg);
	}

	@supports (corner-shape: squircle) {
		.compact .project,
		.square {
			border-radius: calc(var(--square) * 0.42);
		}

		.tile {
			border-radius: 40%;
		}
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

	.text {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
		overflow: hidden;
		opacity: 0;
		transition: opacity calc(var(--dur-fast) / 2) linear;
	}

	.card .text {
		opacity: 1;
		transition: opacity var(--dur-med) var(--dur-fast);
	}

	strong,
	.path {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.path {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
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
		overflow: clip;
		overflow-clip-margin: var(--space-1);
		transition:
			height var(--dur-med) var(--ease-out),
			margin var(--dur-med) var(--ease-out),
			padding var(--dur-med) var(--ease-out),
			opacity var(--dur-med),
			visibility var(--dur-med);
	}

	.card .worktrees {
		margin-left: calc(var(--space-2) + var(--square) / 2);
		padding-left: var(--space-3);
	}

	.worktrees.shut {
		height: 0;
		margin-top: calc(-1 * var(--space-1));
		opacity: 0;
		visibility: hidden;
	}

	.worktree {
		position: relative;
		display: flex;
		flex: none;
		align-items: center;
		gap: 0;
		box-sizing: border-box;
		width: var(--square);
		min-height: calc(var(--square) - 6px);
		padding: 0 0 0 3px;
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: start;
		cursor: pointer;
		transition:
			width var(--dur-med) var(--ease-out),
			min-height var(--dur-med) var(--ease-out),
			padding var(--dur-med) var(--ease-out),
			gap var(--dur-med) var(--ease-out),
			background var(--dur-fast);
	}

	.card .worktree {
		gap: var(--space-2);
		width: 100%;
		min-height: var(--target-min);
		padding: 0 var(--space-2) 0 0;
	}

	.worktree::before,
	.worktree:not(:last-child)::after {
		content: '';
		position: absolute;
		left: calc(-1 * var(--space-3));
		border-left: var(--border-width) solid var(--border-strong);
		opacity: 0;
		transition: opacity calc(var(--dur-fast) / 2) linear;
	}

	.worktree::before {
		top: calc(-1 * var(--space-1));
		width: calc(var(--space-3) - var(--space-1));
		height: calc(50% + var(--space-1));
		border-bottom: var(--border-width) solid var(--border-strong);
		border-bottom-left-radius: var(--radius-md);
	}

	.worktree:not(:last-child)::after {
		top: 50%;
		bottom: calc(-1 * var(--space-1));
	}

	.card .worktree::before,
	.card .worktree:not(:last-child)::after {
		opacity: 1;
		transition: opacity var(--dur-med) var(--dur-fast);
	}

	.card .worktree:hover {
		background: var(--hover);
	}

	.card .worktree.focused {
		background: var(--surface-3);
	}

	.tile {
		display: grid;
		flex: none;
		place-items: center;
		box-sizing: border-box;
		width: calc(var(--square) - 6px);
		height: calc(var(--square) - 6px);
		border: var(--border-width) solid var(--muted-fg);
		border-radius: calc(var(--radius-tile) - 2px);
		background: var(--surface-1);
		font-size: var(--font-size-sm);
		font-weight: 700;
		transition:
			width var(--dur-med) var(--ease-out),
			height var(--dur-med) var(--ease-out),
			background var(--dur-fast);
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
		min-width: 0;
		overflow: hidden;
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
		opacity: 0;
		transition: opacity calc(var(--dur-fast) / 2) linear;
	}

	.card .name {
		opacity: 1;
		transition: opacity var(--dur-med) var(--dur-fast);
	}

	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}

	@keyframes appear {
		from {
			opacity: 0;
		}
	}
</style>
