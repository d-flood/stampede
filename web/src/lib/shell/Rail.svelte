<script lang="ts">
	import type { Epic, Project } from '#lib/device.svelte.ts';
	import type { Tile } from '#lib/projects.ts';
	import ProjectRow from '#lib/shell/ProjectRow.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import type { Snippet } from 'svelte';
	import CaretDoubleLeft from '~icons/ph/caret-double-left';
	import CaretDoubleRight from '~icons/ph/caret-double-right';
	import Plus from '~icons/ph/plus';

	type Props = {
		open?: boolean;
		projects: Project[];
		labels: Map<string, string>;
		worktrees: Map<string, Tile[]>;
		epics: Record<string, Epic[]>;
		roots: Set<string>;
		working: Set<string>;
		needs: Map<string, number>;
		focused?: string;
		onfocus(id: string): void;
		onreorder(order: string[]): void;
		onclose(id: string): void;
		ondelete(id: string): void;
		onworktree(project: Project): void;
		onmenu(id: string): void;
		onpick(): void;
		onicon(project: Project): void;
		attention?: Snippet;
	};

	let { open = $bindable(false), projects, labels, worktrees, epics, roots, working, needs, focused, onfocus, onreorder, onclose, ondelete, onworktree, onmenu, onpick, onicon, attention }: Props = $props();

	const gutter = $derived(projects.some((project) => worktrees.get(project.id)?.length));

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

<nav aria-label="Projects" class:open>
	<div class="top">
		<IconButton label={open ? 'Collapse project rail' : 'Expand project rail'} aria-expanded={open} onclick={() => (open = !open)}>
			{#if open}<CaretDoubleLeft />{:else}<CaretDoubleRight />{/if}
		</IconButton>
	</div>
	<ol bind:this={list} onpointerdown={pointerdown}>
		{#each shown as project (project.id)}
			<ProjectRow
				{project}
				label={labels.get(project.id) ?? project.path}
				{labels}
				tiles={worktrees.get(project.id) ?? []}
				epics={epics[project.id] ?? []}
				root={roots.has(project.id)}
				working={working.has(project.id)}
				needs={needs.get(project.id) ?? 0}
				compact={!open}
				{gutter}
				{focused}
				dragging={!!drag?.moved && drag.id === project.id}
				onfocus={(id) => {
					if (!dragged) onfocus(id);
				}}
				{onclose}
				{ondelete}
				{onworktree}
				{onmenu}
				{onicon}
			/>
		{/each}
	</ol>
	<button class="add squircle" title={open ? undefined : 'Open Project'} onclick={onpick}>
		<Plus /><span class="label">Open Project</span>
	</button>
	<span class="fill"></span>
	<div class="foot">{@render attention?.()}</div>
</nav>

<style>
	nav {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-2);
		box-sizing: border-box;
		width: calc(var(--tile) + var(--space-3) + var(--space-1));
		padding: var(--space-2) var(--space-1) var(--space-2) var(--space-3);
		user-select: none;
		transition: width var(--dur-med) var(--ease-out);
	}

	nav.open {
		width: 17rem;
		overflow-y: auto;
	}

	.top {
		display: flex;
		align-self: flex-end;
		justify-content: center;
		width: var(--tile);
	}

	ol {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.add {
		display: inline-flex;
		flex: none;
		align-self: flex-start;
		align-items: center;
		justify-content: center;
		gap: 0;
		box-sizing: border-box;
		width: var(--tile);
		height: var(--tile);
		padding: 0;
		border: var(--border-width) dashed var(--border-strong);
		border-radius: var(--radius-tile);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		white-space: nowrap;
		cursor: pointer;
		transition:
			width var(--dur-med) var(--ease-out),
			height var(--dur-med) var(--ease-out),
			gap var(--dur-med) var(--ease-out),
			border-radius var(--dur-med) var(--ease-out),
			background var(--dur-fast),
			color var(--dur-fast);
	}

	.open .add {
		gap: var(--space-2);
		width: 100%;
		height: calc(var(--target-min) + var(--space-2));
		border-radius: var(--radius-lg);
	}

	.add :global(svg) {
		flex: none;
		font-size: var(--font-size-lg);
	}

	.label {
		max-width: 0;
		overflow: hidden;
		opacity: 0;
		transition:
			max-width var(--dur-med) var(--ease-out),
			opacity calc(var(--dur-fast) / 2) linear;
	}

	.open .label {
		max-width: 12em;
		opacity: 1;
		transition:
			max-width var(--dur-med) var(--ease-out),
			opacity var(--dur-med) var(--dur-fast);
	}

	.add:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.add:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: var(--border-width);
	}

	@supports (corner-shape: squircle) {
		nav:not(.open) .add {
			border-radius: calc(var(--tile) * 0.42);
		}
	}

	.fill {
		flex: 1;
	}

	.foot {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
		width: var(--tile);
	}
</style>
