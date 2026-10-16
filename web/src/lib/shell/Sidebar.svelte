<script lang="ts" module>
	export type Section = 'files' | 'changes' | 'history' | 'search';
</script>

<script lang="ts">
	import type { Project } from '#lib/device.ts';
	import Files from '#lib/shell/Files.svelte';

	type Props = {
		open: boolean;
		project?: Project;
		label?: string;
		section: Section;
		onopen(path: string): void;
	};

	let { open, project, label, section = $bindable(), onopen }: Props = $props();

	const sections: { id: Section; label: string }[] = [
		{ id: 'files', label: 'Files' },
		{ id: 'changes', label: 'Changes' },
		{ id: 'history', label: 'History' },
		{ id: 'search', label: 'Search' }
	];
</script>

<aside aria-label="Sidebar" hidden={!open}>
	{#if project}
		<h1 title={project.path}>{label}</h1>
		<div class="sections" role="group" aria-label="Sections">
			{#each sections as { id, label } (id)}
				<button aria-pressed={section === id} disabled={id !== 'files'} onclick={() => (section = id)}>{label}</button>
			{/each}
		</div>
		{#if section === 'files'}
			{#key project.id}
				<Files projectId={project.id} {onopen} />
			{/key}
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
