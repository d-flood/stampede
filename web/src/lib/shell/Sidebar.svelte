<script lang="ts" module>
	import type { Git, Trunk } from '#lib/git.ts';
	import { lazy, warmUp } from '#lib/paint.svelte.ts';

	const picker = lazy(() => import('#lib/shell/BranchPicker.svelte'));

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
	import { sidebarPosition } from '#lib/sidebar.svelte.ts';
	import type { Against } from '#lib/files.svelte.ts';
	import type { Change } from '#lib/git.ts';
	import Changes from '#lib/shell/Changes.svelte';
	import Epics from '#lib/shell/Epics.svelte';
	import Files from '#lib/shell/Files.svelte';
	import History from '#lib/shell/History.svelte';
	import Search from '#lib/shell/Search.svelte';
	import Usage from '#lib/shell/Usage.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import type { Snippet } from 'svelte';
	import CaretUpDown from '~icons/ph/caret-up-down';
	import GitBranch from '~icons/ph/git-branch';

	type Props = {
		open: boolean;
		project?: Project;
		label?: string;
		git?: Git;
		trunk?: Trunk;
		device: Device;
		section: Section;
		usage: Reading[];
		codexResets?: ResetCredits | null;
		epics?: Epic[];
		onopen(path: string, cursor?: Cursor): void;
		onrename(from: string, to: string): void;
		ondiff(change: Change, against: Against): void;
		onepic(slug: string): void;
		toggle?: Snippet;
	};

	let { open, project, label, git, trunk, device, section = $bindable(), usage, codexResets, epics, onopen, onrename, ondiff, onepic, toggle }: Props = $props();

	let switching = $state(false);
	const Picker = $derived(picker());
	const shown = $derived(available(git));
	const current = $derived(showing(section, git).id);
	const id = $props.id();
	let viewport = $state(window.innerWidth);
	let width = $state(320);
	let drag = $state<{ x: number; width: number }>();
	const maximum = $derived(Math.min(640, viewport * 0.6));
	const minimum = $derived(Math.min(160, maximum));
	const size = $derived(Math.min(maximum, Math.max(minimum, width)));
	const direction = $derived(sidebarPosition.side === 'right' ? -1 : 1);

	function resize(value: number) {
		width = Math.min(maximum, Math.max(minimum, value));
	}

	function resizeKey(event: KeyboardEvent) {
		if (event.key === 'ArrowLeft') resize(size - 16 * direction);
		else if (event.key === 'ArrowRight') resize(size + 16 * direction);
		else if (event.key === 'Home') resize(minimum);
		else if (event.key === 'End') resize(maximum);
		else return;
		event.preventDefault();
	}
</script>

<svelte:window bind:innerWidth={viewport} />

<aside
	{id}
	aria-label="Sidebar"
	class:closed={!open}
	class:phone={form() === 'phone'}
	class:right={form() === 'desktop' && sidebarPosition.side === 'right'}
	class:dragging={!!drag}
	style:width={form() === 'desktop' ? `${open ? size : 0}px` : undefined}
>
	<div class="clip">
		<div class="content" style:width={form() === 'desktop' ? `${size}px` : undefined}>
			{#if form() === 'desktop'}
				<header>
					{#if project}<h1 title={project.path}>{label}</h1>{/if}
					{@render toggle?.()}
				</header>
			{/if}
			{#if project}
				{#if git?.branch && project.worktree}
					<p class="branch" title={git.branch}><GitBranch role="img" aria-label="Branch" /><span>{git.branch}</span></p>
				{:else if git?.branch}
					<button class="branch" title="Switch branch" aria-haspopup="dialog" aria-expanded={switching} onclick={() => ((switching = true), warmUp())}>
						<GitBranch role="img" aria-label="Branch" /><span>{git.branch}</span><CaretUpDown />
					</button>
					<Dialog bind:open={switching} full={form() === 'phone'} title="Switch branch" description="Uncommitted changes come along unless they clash with the branch.">
						{#if Picker}<Picker projectId={project.id} current={git.branch} origin={!!git.origin} onswitch={() => (switching = false)} />{/if}
					</Dialog>
				{/if}
				<div class="sections" role="group" aria-label="Sections">
					{#each shown as { id, label } (id)}
						<button aria-pressed={current === id} onclick={() => (section = id)}>{label}</button>
					{/each}
				</div>
				{#if current === 'files'}
					{#key project.id}
						<Files projectId={project.id} {device} changes={git?.changes ?? []} {onopen} {onrename} />
					{/key}
				{:else if current === 'changes'}
					{#key project.id}
						{#if git}<Changes projectId={project.id} {git} {trunk} {device} onopen={(path) => onopen(path)} {ondiff} />{/if}
					{/key}
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
		</div>
	</div>
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
				if (drag && event.currentTarget.hasPointerCapture(event.pointerId)) resize(drag.width + (event.clientX - drag.x) * direction);
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
		width: 20rem;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-xl);
		background: var(--surface-1);
		transition:
			width var(--dur-med) var(--ease-out),
			margin var(--dur-med) var(--ease-out);
	}

	.dragging {
		transition: none;
	}

	.closed {
		margin-right: calc(-1 * var(--gutter) - 2 * var(--border-width));
		visibility: hidden;
		transition:
			width var(--dur-med) var(--ease-out),
			margin var(--dur-med) var(--ease-out),
			visibility var(--dur-med);
	}

	.right.closed {
		margin-right: 0;
		margin-left: calc(-1 * var(--gutter) - 2 * var(--border-width));
	}

	.clip {
		display: flex;
		flex: 1;
		min-width: 0;
		border-radius: calc(var(--radius-xl) - var(--border-width));
		overflow: clip;
	}

	.right .clip {
		justify-content: flex-end;
	}

	.content {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-2);
		min-height: 0;
	}

	.resizer {
		position: absolute;
		z-index: 1;
		inset: var(--radius-xl) calc(-1 * var(--gutter) - var(--border-width) - 3px) var(--radius-xl) auto;
		width: calc(var(--gutter) + 6px);
		cursor: col-resize;
		touch-action: none;
		user-select: none;
	}

	.resizer::after {
		content: '';
		position: absolute;
		inset: 0 calc(50% - 1px);
		border-radius: 1px;
	}

	.resizer:hover::after,
	.resizer:focus-visible::after,
	.resizer.dragging::after {
		background: var(--border-strong);
	}

	.resizer:focus-visible {
		outline: none;
	}

	.right {
		order: 1;
	}

	.right .resizer {
		inset: var(--radius-xl) auto var(--radius-xl) calc(-1 * var(--gutter) - var(--border-width) - 3px);
	}

	.phone {
		flex: 1;
		width: auto;
		min-height: 0;
		padding-top: var(--space-2);
		border: none;
		border-radius: 0;
	}

	.phone .content {
		flex: 1;
		min-width: 0;
	}

	header {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-2) 0 var(--space-4);
	}

	.right header {
		flex-direction: row-reverse;
		padding: var(--space-2) var(--space-4) 0 var(--space-2);
	}

	h1 {
		flex: 1;
		min-width: 0;
		margin: 0;
		overflow: hidden;
		font-size: var(--font-size-lg);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.branch {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		margin: 0;
		padding: 0 var(--space-4);
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	button.branch {
		align-self: start;
		max-width: calc(100% - 2 * var(--space-2));
		min-height: var(--target-min);
		margin: 0 var(--space-2);
		padding: 0 var(--space-2);
		border: none;
		border-radius: calc(var(--target-min) / 2);
		background: none;
		cursor: pointer;
	}

	button.branch:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	button.branch:focus-visible {
		outline: var(--border-width) solid var(--status-working);
	}

	.branch :global(svg) {
		flex: none;
	}

	.branch span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.sections {
		display: flex;
		flex-wrap: wrap;
		margin: 0 var(--space-2);
		padding: var(--space-1);
		border-radius: calc(var(--radius-md) + var(--space-1));
		background: var(--surface-2);
		box-shadow: inset 0 0 0 var(--border-width) var(--border);
	}

	.sections button {
		flex: 1 0 auto;
		min-height: var(--target-min);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		cursor: pointer;
		transition:
			background var(--dur-fast),
			color var(--dur-fast);
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
		box-shadow:
			var(--shadow-lift),
			0 0 0 var(--border-width) var(--border);
		color: var(--color-fg);
		font-weight: 600;
	}
</style>
