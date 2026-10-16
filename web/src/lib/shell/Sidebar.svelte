<script lang="ts" module>
	export type Section = 'files' | 'changes' | 'history' | 'search';
</script>

<script lang="ts">
	import type { Device, Project } from '#lib/device.ts';
	import type { Change, Git } from '#lib/git.ts';
	import Changes from '#lib/shell/Changes.svelte';
	import Files from '#lib/shell/Files.svelte';

	type Props = {
		open: boolean;
		project?: Project;
		label?: string;
		git?: Git;
		device: Device;
		section: Section;
		onopen(path: string): void;
		ondiff(change: Change): void;
	};

	let { open, project, label, git, device, section = $bindable(), onopen, ondiff }: Props = $props();

	const sections: { id: Section; label: string }[] = [
		{ id: 'files', label: 'Files' },
		{ id: 'changes', label: 'Changes' },
		{ id: 'history', label: 'History' },
		{ id: 'search', label: 'Search' }
	];

	const shown = $derived(sections.filter(({ id }) => id !== 'changes' || git?.isRepo));
	const current = $derived(shown.some(({ id }) => id === section) ? section : 'files');
</script>

<aside aria-label="Sidebar" hidden={!open}>
	{#if project}
		<h1 title={project.path}>{label}</h1>
		<div class="sections" role="group" aria-label="Sections">
			{#each shown as { id, label } (id)}
				<button aria-pressed={current === id} disabled={id === 'history' || id === 'search'} onclick={() => (section = id)}>{label}</button>
			{/each}
		</div>
		{#if current === 'files'}
			{#key project.id}
				<Files projectId={project.id} {device} changes={git?.changes ?? []} {onopen} />
			{/key}
		{:else if current === 'changes'}
			<Changes changes={git?.changes ?? []} onopen={ondiff} />
		{/if}
	{/if}
</aside>

<style>
	aside {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-2);
		width: 20rem;
		border-right: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	aside[hidden] {
		display: none;
	}

	h1 {
		margin: 0;
		padding: var(--space-3) var(--space-4) 0;
		overflow: hidden;
		font-size: var(--font-size-lg);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.sections {
		display: flex;
		margin: 0 var(--space-2);
		padding: var(--space-1);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
	}

	.sections button {
		flex: 1;
		min-height: var(--target-min);
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		cursor: pointer;
	}

	.sections button:hover:not(:disabled) {
		color: var(--color-fg);
	}

	.sections button:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.sections [aria-pressed='true'] {
		background: var(--color-bg);
		color: var(--color-fg);
		font-weight: 600;
	}
</style>
