<script lang="ts">
	import type { Epic, Project } from '#lib/device.svelte.ts';
	import Progress from '#lib/epics/Progress.svelte';
	import { counts } from '#lib/epics.svelte.ts';
	import { failures, perform, progress } from '#lib/git-busy.svelte.ts';
	import { pullTrunk, type Trunk } from '#lib/git.ts';
	import { hue, type Cloud, type Tile } from '#lib/projects.ts';
	import ShellMark from '#lib/ui/ShellMark.svelte';
	import { WorkingMark } from '#lib/ui/spinners.ts';
	import ProjectIcon from '#lib/shell/ProjectIcon.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import ArrowDown from '~icons/ph/arrow-down';
	import ArrowDownBold from '~icons/ph/arrow-down-bold';
	import CloudBold from '~icons/ph/cloud-bold';
	import CloudFill from '~icons/ph/cloud-fill';

	type Props = {
		project: Project;
		label: string;
		labels: Map<string, string>;
		tiles: Tile[];
		epics: Epic[];
		root: boolean;
		working?: boolean;
		background?: boolean;
		needs?: number;
		trunk?: Trunk;
		compact?: boolean;
		focused?: string;
		dragging?: boolean;
		onfocus(id: string): void;
		onclose(id: string): void;
		ondelete(id: string): void;
		onsetup(id: string): void;
		onworktree(project: Project): void;
		onmenu(id: string): void;
		onicon(project: Project): void;
	};

	let { project, label, labels, tiles, epics, root, working = false, background = false, needs = 0, trunk, compact = false, focused, dragging = false, onfocus, onclose, ondelete, onsetup, onworktree, onmenu, onicon }: Props =
		$props();

	const runs = $derived(epics.filter((epic) => epic.run && !epic.archived));
	const active = $derived(project.id === focused || tiles.some((tile) => tile.id === focused));
	const listed = $derived(tiles.length > 0 && (!compact || active));
	const tint = $derived(hue(project.path));
	const pulling = $derived(progress[project.id]?.action === 'trunk');
	const failed = $derived(failures[project.id]?.action === 'trunk' ? failures[project.id].text : undefined);

	function look({ setup, state, inSync }: Cloud): { text?: string; tone?: 'failed' | 'waiting'; tile?: 'lost' | 'failed'; hollow?: boolean; spinning?: boolean } {
		if (state === 'gone' || state === 'unreachable') return { text: state, tile: 'lost', hollow: true };
		if (setup === 'failed') return { text: 'setup failed', tone: 'failed', tile: 'failed' };
		if (setup !== 'ready') return { text: 'setting up', spinning: true };
		if (!inSync) return { text: 'out of sync', tone: 'waiting' };
		if (state === 'asleep') return { text: 'asleep', hollow: true };
		return {};
	}

	function select(event: MouseEvent & { currentTarget: HTMLElement }, id: string) {
		// A long-press opens the menu on a timer, and its release still clicks.
		if (event.currentTarget.dataset.state !== 'open') onfocus(id);
	}
</script>

<li class={compact ? 'compact' : 'card'} class:tray={compact && listed}>
	<Menu context items={[
		{ label: 'Change project icon…', onselect: () => onicon(project) },
		...(trunk
			? [{ label: `Pull ${trunk.branch}`, icon: ArrowDown, hint: `${trunk.behind} new`, disabled: !!progress[project.id], onselect: () => perform(project.id, 'trunk', () => pullTrunk(project.id)) }]
			: []),
		...(root && !project.host ? [{ label: 'New worktree…', onselect: () => onworktree(project) }] : []),
		{ label: 'Close', onselect: () => onclose(project.id) }
	]} onopen={() => onmenu(project.id)}>
		{#snippet trigger(props)}
			<button
				{...props}
				tabindex={0}
				class="project squircle"
				class:focused={project.id === focused}
				class:dragging
				style:--hue="var(--ansi-{tint})"
				aria-label={label}
				aria-current={project.id === focused}
				title={compact ? label : undefined}
				data-project={project.id}
				onclick={(event) => select(event, project.id)}
			>
				<span class="square squircle">
					<ProjectIcon {project} />
					{#if needs}<span class="attention" title="{needs} {needs === 1 ? 'needs' : 'need'} attention"><span class="assistive">{needs}</span></span>{/if}
					{#if trunk}<span class="behind" class:pulling class:failed title={failed ?? `${trunk.behind} new on origin/${trunk.branch}`}><ArrowDownBold /></span>{/if}
					{#if tiles.length}<span class="count" title="{tiles.length} {tiles.length === 1 ? 'worktree' : 'worktrees'}">{tiles.length}</span>{/if}
					{#if working}<span class="spinner" title="working"><WorkingMark /></span>{:else if background}<span class="shell" title="background"><ShellMark /></span>{/if}
				</span>
				<span class="text">
					<strong>{label}</strong>
					<span class="path"><bdi>{project.path.replace(/^(?:\/home|\/Users)\/[^/]+/, '~')}</bdi></span>
					{#each runs as epic (epic.slug)}
						{@const tally = counts(epic)}
						<span class="run">Run {epic.slug} · <span class="resolved">{tally.merged + tally.skipped} of {epic.tickets.length}</span></span>
						<Progress {epic} />
					{/each}
				</span>
			</button>
		{/snippet}
	</Menu>
	{#if tiles.length}
		<div class="worktrees" class:shut={!listed} role="group" aria-label="Worktrees of {label}">
			{#each tiles as tile (tile.id)}
				{@const name = labels.get(tile.id) ?? tile.mark}
				{@const cloud = tile.cloud && look(tile.cloud)}
				{@const description = [name, cloud && 'on a Cloud Host', cloud?.text, tile.status].filter(Boolean).join(', ')}
				<Menu
					context
					items={[
						...(tile.cloud?.setup === 'failed' ? [{ label: 'Run Cloud Setup again', onselect: () => onsetup(tile.id) }] : []),
						{ label: 'Delete worktree', onselect: () => ondelete(tile.id) }
					]}
				>
					{#snippet trigger(props)}
						<button
							{...props}
							tabindex={0}
							class="worktree"
							class:focused={tile.id === focused}
							aria-label={description}
							aria-current={tile.id === focused}
							title={cloud ? description : tile.status ? `${name} ${tile.status}` : name}
							onclick={(event) => select(event, tile.id)}
						>
							<span class="tile squircle {cloud?.tile ?? tile.status ?? ''}">
								{tile.mark}
								{#if cloud}<span class="cloud" class:hollow={cloud.hollow}>{#if cloud.hollow}<CloudBold />{:else}<CloudFill />{/if}</span>{/if}
								{#if tile.needs}<span class="attention" title="{tile.needs} {tile.needs === 1 ? 'needs' : 'need'} attention"><span class="assistive">{tile.needs}</span></span>{/if}
								{#if cloud?.spinning}<span class="spinner" title="setting up"><WorkingMark /></span>{:else if tile.status === 'working'}<span class="spinner" title="working"><WorkingMark /></span>{:else if tile.status === 'background'}<span class="shell" title="background"><ShellMark /></span>{/if}
							</span>
							<span class="name"><span class="label">{name}</span>{#if cloud?.text}<span class="state {cloud.tone ?? ''}">· {cloud.text}</span>{/if}</span>
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
		background: var(--color-bg);
		box-shadow: inset 0 0 0 var(--border-width) var(--border-strong);
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
		min-height: 0;
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

	.compact .project:has(:global(.monogram)) {
		border-color: var(--border);
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
		--tile-base: var(--hover);
		background: var(--hover);
		color: var(--color-fg);
	}

	.project.focused {
		--tile-base: var(--selected-bg);
		border-color: var(--selected-border);
		background: var(--selected-bg);
		color: var(--color-fg);
	}

	.tile::before {
		content: '';
		position: absolute;
		top: 22%;
		bottom: 22%;
		left: calc(-1 * var(--space-3) - var(--square) + var(--target-min));
		width: 4px;
		border-radius: 0 4px 4px 0;
		background: var(--color-fg);
		opacity: 0;
		scale: 1 0;
		transition:
			opacity var(--dur-fast),
			scale var(--dur-med) var(--ease-out);
	}

	.compact .worktree.focused .tile::before {
		opacity: 1;
		scale: 1;
	}

	.compact .project.focused {
		--tile-base: var(--hover);
		border-color: var(--border-strong);
		background: none;
	}

	.compact .project::before {
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
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
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
		--tile-base: var(--surface-3);
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
	.behind,
	.count,
	.cloud,
	.spinner,
	.shell {
		position: absolute;
		display: grid;
		place-items: center;
		box-sizing: border-box;
		min-width: 1.4em;
		height: 1.4em;
		border: var(--border-width) solid var(--canvas);
		border-radius: 0.7em;
		box-shadow: 0 0 0 var(--border-width) var(--canvas);
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
		top: calc(-1 * var(--space-1) / 2);
		right: calc(-1 * var(--space-1) / 2);
		min-width: 0;
		width: var(--space-3);
		height: var(--space-3);
		padding: 0;
		background: var(--status-waiting);
	}

	.assistive {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
	}

	.behind {
		top: calc(-1 * var(--space-1) - var(--border-width));
		left: calc(-1 * var(--space-1) - var(--border-width));
		overflow: hidden;
		color: var(--muted-fg);
	}

	.behind.pulling {
		color: var(--accent);
	}

	.pulling :global(svg) {
		animation: drop 0.9s var(--ease-out) var(--pulse);
	}

	.behind.failed {
		color: var(--git-deleted);
	}

	@keyframes drop {
		from {
			opacity: 0;
			translate: 0 -60%;
		}
		35%,
		70% {
			opacity: 1;
		}
		to {
			opacity: 0;
			translate: 0 60%;
		}
	}

	.count {
		z-index: 1;
		bottom: calc(-1 * var(--space-1) - var(--border-width));
		left: calc(-1 * var(--space-1) - var(--border-width));
		color: var(--muted-fg);
	}

	.cloud {
		bottom: calc(-1 * var(--space-1) - var(--border-width));
		left: calc(-1 * var(--space-1) - var(--border-width));
		color: var(--color-fg);
		transition: color var(--dur-fast);
	}

	.cloud.hollow {
		color: var(--muted-fg);
	}

	.spinner,
	.shell {
		right: calc(-1 * var(--space-1) - var(--border-width));
		bottom: calc(-1 * var(--space-1) - var(--border-width));
	}

	.spinner {
		color: var(--status-working);
	}

	.shell {
		color: var(--status-background);
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

	/* Right-to-left moves the ellipsis to the path's start, keeping its distinguishing tail. */
	.path {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		direction: rtl;
		text-align: left;
	}

	.run {
		font-size: var(--font-size-sm);
	}

	.resolved {
		color: var(--status-done);
	}

	.progress {
		color: var(--status-done);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
	}

	.worktrees {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-height: 0;
		padding-left: calc(var(--square) - var(--target-min));
		clip-path: inset(calc(-1 * var(--space-1) - var(--border-width)) -100vmax);
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
		width: var(--target-min);
		min-height: var(--target-min);
		padding: 0;
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
		left: calc(-1 * var(--space-2));
		border-left: var(--border-width) solid var(--border-strong);
		transition:
			left var(--dur-med) var(--ease-out),
			width var(--dur-med) var(--ease-out);
	}

	.worktree::before {
		top: calc(-1 * var(--space-1));
		width: var(--space-1);
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
		left: calc(-1 * var(--space-3));
	}

	.card .worktree::before {
		width: calc(var(--space-3) - var(--space-1));
	}

	.card .worktree:hover {
		background: var(--hover);
	}

	.card .worktree.focused {
		background: var(--surface-3);
	}

	.tile {
		position: relative;
		display: grid;
		flex: none;
		place-items: center;
		box-sizing: border-box;
		width: var(--target-min);
		height: var(--target-min);
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
		border-color: currentColor;
		background: var(--surface-3);
	}

	.tile.working,
	.tile.waiting,
	.tile.background,
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

	.tile.background {
		color: var(--status-background);
	}

	.tile.done {
		color: var(--status-done);
	}

	.tile .attention,
	.tile .cloud,
	.tile .spinner,
	.tile .shell {
		font-size: 8px;
	}

	.tile.failed {
		border-width: 2px;
		border-color: currentColor;
		color: var(--ansi-1);
	}

	.tile.stopping,
	.tile.lost {
		border-style: dashed;
		color: var(--muted-fg);
	}

	.name {
		display: flex;
		gap: var(--space-1);
		min-width: 0;
		overflow: hidden;
		font-size: var(--font-size-sm);
		white-space: nowrap;
		opacity: 0;
		transition: opacity calc(var(--dur-fast) / 2) linear;
	}

	.card .name {
		opacity: 1;
		transition: opacity var(--dur-med) var(--dur-fast);
	}

	.label {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.state {
		flex: none;
		color: var(--muted-fg);
		transition: color var(--dur-fast);
	}

	.state.failed {
		color: var(--ansi-1);
	}

	.state.waiting {
		color: var(--status-waiting);
	}
</style>
