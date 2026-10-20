<script lang="ts">
	import type { Epic, Project } from '#lib/device.svelte.ts';
	import Progress from '#lib/epics/Progress.svelte';
	import { counts } from '#lib/epics.svelte.ts';
	import type { Tile } from '#lib/projects.ts';
	import ProjectIcon from '#lib/shell/ProjectIcon.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import type { Snippet } from 'svelte';
	import FolderOpen from '~icons/ph/folder-open';

	type Props = {
		projects: Project[];
		labels: Map<string, string>;
		worktrees: Map<string, Tile[]>;
		epics: Record<string, Epic[]>;
		roots: Set<string>;
		focused?: string;
		onfocus(id: string): void;
		onclose(id: string): void;
		ondelete(id: string): void;
		onworktree(project: Project): void;
		onpick(): void;
		onicon(project: Project): void;
		badge?: Snippet<[Project]>;
	};

	let { projects, labels, worktrees, epics, roots, focused, onfocus, onclose, ondelete, onworktree, onpick, onicon, badge }: Props = $props();
</script>

<nav aria-label="Projects">
	<ol>
		{#each projects as project (project.id)}
			{@const label = labels.get(project.id) ?? project.path}
			{@const tiles = worktrees.get(project.id) ?? []}
			<li>
				<Menu context items={[
					{ label: 'Change project icon…', onselect: () => onicon(project) },
					...(roots.has(project.id) ? [{ label: 'New worktree…', onselect: () => onworktree(project) }] : []),
					{ label: 'Close', onselect: () => onclose(project.id) }
				]}>
					{#snippet trigger(props)}
						<button
							{...props}
							tabindex={0}
							class="project"
							class:focused={project.id === focused}
							aria-label={label}
							aria-current={project.id === focused}
							onclick={(event) => {
								// A long-press opens the menu on a timer, and its release still clicks.
								if (event.currentTarget.dataset.state !== 'open') onfocus(project.id);
							}}
						>
							<span class="monogram"><ProjectIcon {project} /></span>
							<span class="text">
								<strong>{label}</strong>
								<span class="path">{project.path}</span>
								{#each (epics[project.id] ?? []).filter((epic) => epic.run && !epic.archived) as epic (epic.slug)}
									{@const tally = counts(epic)}
									<span class="run">Run {epic.slug} · <span class="resolved">{tally.merged + tally.skipped} of {epic.tickets.length}</span></span>
									<Progress {epic} />
								{/each}
							</span>
							{@render badge?.(project)}
						</button>
					{/snippet}
				</Menu>
				{#if tiles.length}
					<div class="worktrees" role="group" aria-label="Worktrees of {label}">
						{#each tiles as tile (tile.id)}
							{@const name = labels.get(tile.id) ?? tile.mark}
							<Menu context items={[{ label: 'Delete worktree', onselect: () => ondelete(tile.id) }]}>
								{#snippet trigger(props)}
									<button
										{...props}
										tabindex={0}
										class="tile {tile.status ?? ''}"
										class:focused={tile.id === focused}
										aria-label={tile.status ? `${name}, ${tile.status}` : name}
										aria-current={tile.id === focused}
										onclick={(event) => {
											if (event.currentTarget.dataset.state !== 'open') onfocus(tile.id);
										}}
									>
										{tile.mark}
									</button>
								{/snippet}
							</Menu>
						{/each}
					</div>
				{/if}
			</li>
		{/each}
	</ol>
	<Button onclick={onpick}><FolderOpen /> Open Project</Button>
</nav>

<style>
	nav {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-2);
		min-height: 0;
		padding: var(--space-2);
		overflow-y: auto;
		user-select: none;
	}

	ol {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		display: flex;
		gap: var(--space-2);
	}

	.project {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-3);
		box-sizing: border-box;
		min-width: 0;
		padding: var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
		text-align: start;
		cursor: pointer;
	}

	.project.focused {
		border-color: var(--color-fg);
	}

	.monogram {
		display: grid;
		flex: none;
		place-items: center;
		width: var(--target-min);
		height: var(--target-min);
		border-radius: var(--radius-md);
		background: var(--surface-3);
		font-weight: 600;
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

	.worktrees {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-2);
	}

	.tile {
		width: var(--target-min);
		height: var(--target-min);
		padding: 0;
		border: var(--border-width) solid var(--muted-fg);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--color-fg);
		font: inherit;
		font-weight: 700;
		cursor: pointer;
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

	.tile.focused {
		outline: var(--border-width) solid var(--color-fg);
		outline-offset: var(--border-width);
	}
</style>
