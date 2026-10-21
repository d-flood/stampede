<script lang="ts">
	import type { Epic, Project } from '#lib/device.svelte.ts';
	import type { Tile } from '#lib/projects.ts';
	import ProjectRow from '#lib/shell/ProjectRow.svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import type { Snippet } from 'svelte';
	import CaretDoubleLeft from '~icons/ph/caret-double-left';
	import CaretDoubleRight from '~icons/ph/caret-double-right';
	import FolderOpen from '~icons/ph/folder-open';
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
		onpick(): void;
		onicon(project: Project): void;
		attention?: Snippet;
	};

	let { open = $bindable(false), projects, labels, worktrees, epics, roots, working, needs, focused, onfocus, onreorder, onclose, ondelete, onworktree, onpick, onicon, attention }: Props = $props();

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
				{onicon}
			/>
		{/each}
	</ol>
	{#if open}
		<Button onclick={onpick}><FolderOpen /> Open Project</Button>
	{:else}
		<IconButton label="Open Project" onclick={onpick}><Plus /></IconButton>
	{/if}
	<span class="fill"></span>
	<div class="foot">{@render attention?.()}</div>
</nav>

<style>
	nav {
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

	nav.open {
		width: 17rem;
		align-items: stretch;
		overflow-y: auto;
	}

	.top {
		display: flex;
		justify-content: center;
	}

	.open .top {
		justify-content: flex-end;
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

	.fill {
		flex: 1;
	}

	.foot {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-2);
	}

	.open .foot {
		flex-direction: row;
	}
</style>
