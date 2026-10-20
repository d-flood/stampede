<script lang="ts">
	import type { Epic, Project } from '#lib/device.svelte.ts';
	import { counts } from '#lib/epics.svelte.ts';
	import type { Tile } from '#lib/projects.ts';
	import ProjectIcon from '#lib/shell/ProjectIcon.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import type { Snippet } from 'svelte';
	import Plus from '~icons/ph/plus';

	type Props = {
		projects: Project[];
		labels: Map<string, string>;
		worktrees: Map<string, Tile[]>;
		epics: Record<string, Epic[]>;
		focused?: string;
		onfocus(id: string): void;
		onreorder(order: string[]): void;
		onclose(id: string): void;
		onpick(): void;
		onicon(project: Project): void;
		badge?: Snippet<[Project]>;
		attention?: Snippet;
	};

	let { projects, labels, worktrees, epics, focused, onfocus, onreorder, onclose, onpick, onicon, badge, attention }: Props = $props();

	let list: HTMLOListElement;
	let drag = $state<{ id: string; y: number; order: string[]; moved: boolean }>();
	let dragged = false;

	const shown = $derived(
		drag ? drag.order.flatMap((id) => projects.filter((project) => project.id === id)) : projects
	);

	function pointerdown(event: PointerEvent) {
		const id = (event.target as HTMLElement).closest<HTMLElement>('[data-project]')?.dataset.project;
		if (!id || event.button !== 0) return;
		drag = { id, y: event.clientY, order: projects.map((project) => project.id), moved: false };
		dragged = false;
		window.addEventListener('pointermove', pointermove);
		window.addEventListener('pointerup', end);
		window.addEventListener('pointercancel', end);
	}

	function pointermove(event: PointerEvent) {
		if (!drag || (!drag.moved && Math.abs(event.clientY - drag.y) < 4)) return;
		drag.moved = true;
		const id = drag.id;
		const others = drag.order.filter((other) => other !== id);
		const above = [...list.querySelectorAll<HTMLElement>('[data-project]')].filter((row) => {
			const { top, height } = row.getBoundingClientRect();
			return row.dataset.project !== id && event.clientY > top + height / 2;
		}).length;
		drag.order = [...others.slice(0, above), id, ...others.slice(above)];
	}

	function end(event: PointerEvent) {
		window.removeEventListener('pointermove', pointermove);
		window.removeEventListener('pointerup', end);
		window.removeEventListener('pointercancel', end);
		if (drag?.moved) {
			dragged = true;
			setTimeout(() => (dragged = false));
			if (event.type === 'pointerup') onreorder(drag.order);
		}
		drag = undefined;
	}
</script>

<nav aria-label="Projects">
	<ol bind:this={list} onpointerdown={pointerdown}>
		{#each shown as project (project.id)}
			{@const label = labels.get(project.id) ?? project.path}
			{@const tiles = worktrees.get(project.id) ?? []}
			<li>
				<span class="own">
					<Menu context items={[{ label: 'Change project icon…', onselect: () => onicon(project) }, { label: 'Close', onselect: () => onclose(project.id) }]}>
						{#snippet trigger(props)}
							<button
								{...props}
								tabindex={0}
								class="project"
								class:focused={project.id === focused}
								class:dragging={drag?.moved && drag.id === project.id}
								aria-label={label}
								aria-current={project.id === focused}
								title={label}
								data-project={project.id}
								onclick={() => {
									if (!dragged) onfocus(project.id);
								}}
							>
								<ProjectIcon {project} />
								{#if badge}<span class="badge">{@render badge(project)}</span>{/if}
							</button>
						{/snippet}
					</Menu>
					{#each (epics[project.id] ?? []).filter((epic) => epic.run && !epic.archived) as epic (epic.slug)}
						{@const tally = counts(epic)}
						<span class="progress" title="Run {epic.slug}: {tally.merged + tally.skipped} of {epic.tickets.length} resolved">
							{tally.merged + tally.skipped}/{epic.tickets.length}
						</span>
					{/each}
				</span>
				{#if tiles.length}
					<div class="worktrees" role="group" aria-label="Worktrees of {label}">
						{#each tiles as tile (tile.id)}
							{@const name = labels.get(tile.id) ?? tile.mark}
							<button
								class="tile {tile.status ?? ''}"
								class:focused={tile.id === focused}
								aria-label={tile.status ? `${name}, ${tile.status}` : name}
								aria-current={tile.id === focused}
								title={tile.status ? `${name} ${tile.status}` : name}
								onclick={() => onfocus(tile.id)}
							>
								{tile.mark}
							</button>
						{/each}
					</div>
				{/if}
			</li>
		{/each}
	</ol>
	<IconButton label="Open Project" onclick={onpick}><Plus /></IconButton>
	<span class="fill"></span>
	{@render attention?.()}
</nav>

<style>
	nav {
		--monogram: calc(var(--target-min) + var(--space-2));

		display: flex;
		flex: none;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2);
		border-right: var(--border-width) solid var(--border);
		background: var(--surface-2);
		user-select: none;
	}

	ol {
		display: flex;
		flex-direction: column;
		align-self: stretch;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		display: flex;
		gap: var(--space-2);
	}

	.own {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-1);
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
		padding-left: var(--space-2);
		border-left: var(--border-width) solid var(--border);
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
		font-size: var(--font-size-sm);
		font-weight: 700;
		cursor: pointer;
	}

	.tile:hover {
		background: var(--hover);
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

	.tile:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	.project {
		position: relative;
		width: var(--monogram);
		height: var(--monogram);
		padding: 0;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-1);
		color: var(--muted-fg);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
		touch-action: none;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
	}

	.project:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.project:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	.project.focused {
		border-color: var(--color-fg);
		background: var(--color-bg);
		color: var(--color-fg);
	}

	.project.focused::before {
		content: '';
		position: absolute;
		top: 20%;
		bottom: 20%;
		left: calc(-1 * var(--space-2) - var(--border-width));
		width: 3px;
		border-radius: 0 3px 3px 0;
		background: var(--color-fg);
	}

	.project.dragging {
		box-shadow: var(--shadow-float);
		cursor: grabbing;
	}

	.badge {
		position: absolute;
		top: calc(-1 * var(--space-1));
		right: calc(-1 * var(--space-1));
	}

	.fill {
		flex: 1;
	}
</style>
