<script lang="ts" module>
	import type { Git } from '#lib/git.ts';

	export type Section = 'files' | 'changes' | 'history' | 'search';

	const sections: { id: Section; label: string }[] = [
		{ id: 'files', label: 'Files' },
		{ id: 'changes', label: 'Changes' },
		{ id: 'history', label: 'History' },
		{ id: 'search', label: 'Search' }
	];

	export function available(git?: Git) {
		return sections.filter(({ id }) => (id !== 'changes' && id !== 'history') || git?.isRepo);
	}

	export function showing(section: Section, git?: Git) {
		return available(git).find(({ id }) => id === section) ?? sections[0];
	}
</script>

<script lang="ts">
	import type { Device, Project, Reading } from '#lib/device.svelte.ts';
	import type { Cursor } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import type { Change, Commit } from '#lib/git.ts';
	import Changes from '#lib/shell/Changes.svelte';
	import Files from '#lib/shell/Files.svelte';
	import History from '#lib/shell/History.svelte';
	import Search from '#lib/shell/Search.svelte';
	import Usage from '#lib/shell/Usage.svelte';

	type Props = {
		open: boolean;
		project?: Project;
		label?: string;
		git?: Git;
		device: Device;
		section: Section;
		usage: Reading[];
		onopen(path: string, cursor?: Cursor): void;
		ondiff(change: Change, commit?: Commit): void;
	};

	let { open, project, label, git, device, section = $bindable(), usage, onopen, ondiff }: Props = $props();

	const shown = $derived(available(git));
	const current = $derived(showing(section, git).id);
</script>

<aside aria-label="Sidebar" hidden={!open} class:phone={form() === 'phone'}>
	{#if project}
		{#if form() === 'desktop'}<h1 title={project.path}>{label}</h1>{/if}
		<div class="sections" role="group" aria-label="Sections">
			{#each shown as { id, label } (id)}
				<button aria-pressed={current === id} onclick={() => (section = id)}>{label}</button>
			{/each}
		</div>
		{#if current === 'files'}
			{#key project.id}
				<Files projectId={project.id} {device} changes={git?.changes ?? []} {onopen} />
			{/key}
		{:else if current === 'changes'}
			<Changes changes={git?.changes ?? []} onopen={ondiff} />
		{:else if current === 'history'}
			{#key project.id}
				<History projectId={project.id} {device} onopen={ondiff} />
			{/key}
		{/if}
		{#key project.id}
			<Search projectId={project.id} {device} hidden={current !== 'search'} {onopen} />
		{/key}
	{/if}
	{#if form() === 'desktop'}<Usage readings={usage} />{/if}
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

	.phone {
		flex: 1;
		width: auto;
		min-height: 0;
		padding-top: var(--space-2);
		border-right: none;
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
