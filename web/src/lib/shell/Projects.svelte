<script lang="ts">
	import type { Epic, Project } from '#lib/device.svelte.ts';
	import type { Tile } from '#lib/projects.ts';
	import ProjectRow from '#lib/shell/ProjectRow.svelte';
	import Button from '#lib/ui/Button.svelte';
	import FolderOpen from '~icons/ph/folder-open';

	type Props = {
		projects: Project[];
		labels: Map<string, string>;
		worktrees: Map<string, Tile[]>;
		epics: Record<string, Epic[]>;
		roots: Set<string>;
		working: Set<string>;
		needs: Map<string, number>;
		focused?: string;
		onfocus(id: string): void;
		onclose(id: string): void;
		ondelete(id: string): void;
		onworktree(project: Project): void;
		onmenu(id: string): void;
		onpick(): void;
		onicon(project: Project): void;
	};

	let { projects, labels, worktrees, epics, roots, working, needs, focused, onfocus, onclose, ondelete, onworktree, onmenu, onpick, onicon }: Props = $props();

	const gutter = $derived(projects.some((project) => worktrees.get(project.id)?.length));
</script>

<nav aria-label="Projects">
	<ol>
		{#each projects as project (project.id)}
			<ProjectRow
				{project}
				label={labels.get(project.id) ?? project.path}
				{labels}
				tiles={worktrees.get(project.id) ?? []}
				epics={epics[project.id] ?? []}
				root={roots.has(project.id)}
				working={working.has(project.id)}
				needs={needs.get(project.id) ?? 0}
				{gutter}
				{focused}
				{onfocus}
				{onclose}
				{ondelete}
				{onworktree}
				{onmenu}
				{onicon}
			/>
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
</style>
