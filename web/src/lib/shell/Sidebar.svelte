<script lang="ts" module>
	import type { Git } from '#lib/git.ts';

	export type Section = 'files' | 'changes' | 'history' | 'search' | 'epics';

	const sections: { id: Section; label: string }[] = [
		{ id: 'files', label: 'Files' },
		{ id: 'changes', label: 'Changes' },
		{ id: 'history', label: 'History' },
		{ id: 'search', label: 'Search' },
		{ id: 'epics', label: 'Epics' }
	];

	export function available(git?: Git) {
		return sections.filter(({ id }) => id === 'files' || id === 'search' || git?.isRepo);
	}

	export function showing(section: Section, git?: Git) {
		return available(git).find(({ id }) => id === section) ?? sections[0];
	}
</script>

<script lang="ts">
	import type { Device, Epic, Project, Reading, ResetCredits } from '#lib/device.svelte.ts';
	import type { Cursor } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import type { Change, Commit } from '#lib/git.ts';
	import Changes from '#lib/shell/Changes.svelte';
	import Epics from '#lib/shell/Epics.svelte';
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
		codexResets?: ResetCredits | null;
		epics?: Epic[];
		onopen(path: string, cursor?: Cursor): void;
		ondiff(change: Change, commit?: Commit): void;
		onepic(slug: string): void;
	};

	let { open, project, label, git, device, section = $bindable(), usage, codexResets, epics, onopen, ondiff, onepic }: Props = $props();

	const shown = $derived(available(git));
	const current = $derived(showing(section, git).id);
	const id = $props.id();
	let viewport = $state(window.innerWidth);
	let width = $state(320);
	let drag = $state<{ x: number; width: number }>();
	const maximum = $derived(Math.min(640, viewport * 0.6));
	const minimum = $derived(Math.min(160, maximum));
	const size = $derived(Math.min(maximum, Math.max(minimum, width)));

	function resize(value: number) {
		width = Math.min(maximum, Math.max(minimum, value));
	}

	function resizeKey(event: KeyboardEvent) {
		if (event.key === 'ArrowLeft') resize(size - 16);
		else if (event.key === 'ArrowRight') resize(size + 16);
		else if (event.key === 'Home') resize(minimum);
		else if (event.key === 'End') resize(maximum);
		else return;
		event.preventDefault();
	}
</script>

<svelte:window bind:innerWidth={viewport} />

<aside {id} aria-label="Sidebar" hidden={!open} class:phone={form() === 'phone'} style:width={form() === 'desktop' ? `${size}px` : undefined}>
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
		{:else if current === 'epics'}
			<Epics epics={epics ?? []} onopen={onepic} />
		{/if}
		{#key project.id}
			<Search projectId={project.id} {device} hidden={current !== 'search'} {onopen} />
		{/key}
	{/if}
	{#if form() === 'desktop'}<Usage readings={usage} {codexResets} />{/if}
	{#if form() === 'desktop'}
		<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (Focusable separators support keyboard resizing.) -->
		<div
			class="resizer"
			class:dragging={!!drag}
			role="separator"
			aria-label="Resize sidebar"
			aria-controls={id}
			aria-orientation="vertical"
			aria-valuemin={Math.round(minimum)}
			aria-valuemax={Math.round(maximum)}
			aria-valuenow={Math.round(size)}
			tabindex="0"
			onkeydown={resizeKey}
			onpointerdown={(event) => {
				if (event.button !== 0) return;
				event.preventDefault();
				drag = { x: event.clientX, width: size };
				event.currentTarget.setPointerCapture(event.pointerId);
			}}
			onpointermove={(event) => {
				if (drag && event.currentTarget.hasPointerCapture(event.pointerId)) resize(drag.width + event.clientX - drag.x);
			}}
			onpointerup={(event) => event.currentTarget.releasePointerCapture(event.pointerId)}
			onlostpointercapture={() => (drag = undefined)}
		></div>
	{/if}
</aside>

<style>
	aside {
		position: relative;
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-2);
		width: 20rem;
		border-right: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	.resizer {
		position: absolute;
		z-index: 1;
		inset: 0 -4px 0 auto;
		width: 8px;
		cursor: col-resize;
		touch-action: none;
		user-select: none;
	}

	.resizer:hover,
	.resizer:focus-visible,
	.resizer.dragging {
		background: var(--border);
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
		flex-wrap: wrap;
		margin: 0 var(--space-2);
		padding: var(--space-1);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--surface-2);
	}

	.sections button {
		flex: 1 0 auto;
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
