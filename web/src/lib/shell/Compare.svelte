<script lang="ts">
	import { comparison } from '#lib/compare.svelte.ts';
	import type { Device } from '#lib/device.svelte.ts';
	import type { Selection } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { letters, refs, split, type Change, type Git, type Refs } from '#lib/git.ts';
	import BranchPicker from '#lib/shell/BranchPicker.svelte';
	import { treeRows } from '#lib/shell/tree.ts';
	import Dialog from '#lib/ui/Dialog.svelte';
	import { onMount, untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import CaretRight from '~icons/ph/caret-right';
	import CaretUpDown from '~icons/ph/caret-up-down';

	type Side = 'target' | 'source';
	type Props = {
		projectId: string;
		git: Git;
		device: Device;
		selection?: Selection;
		onselect(selection: Selection): void;
		onopen(change: Change, selection: Selection): void;
	};

	let { projectId, git, device, selection, onselect, onopen }: Props = $props();

	const collapsed = new SvelteSet<string>();
	let listed = $state.raw<Refs>();
	let picking = $state<Side>();
	const target = $derived(selection?.target ?? listed?.default);
	const source = $derived(selection?.source ?? (git.detached ? undefined : git.branch));
	const compared = $derived(target && source ? comparison(projectId, { target, source }) : undefined);
	const files = $derived(compared?.result?.files);
	const rows = $derived(files ? treeRows(files, 'compare', (key) => !collapsed.has(key)) : []);
	const failure = $derived.by(() => {
		const error = compared?.error;
		if (!error) return;
		return error.status === 409 ? `${target} and ${source} share no history` : error.message;
	});

	onMount(() => {
		refs(projectId).then((refs) => (listed = refs), () => {});
		return device.listen({
			message(message) {
				if (message.type === 'hello' || (message.type === 'git.changed' && message.projectId === projectId)) compared?.load();
			}
		});
	});

	$effect(() => {
		const current = compared;
		untrack(() => current?.load());
	});

	function pick(side: Side, name: string) {
		picking = undefined;
		const chosen = { target: target ?? name, source: source ?? name };
		onselect({ ...chosen, [side]: name });
	}

	function toggle(key: string) {
		if (!collapsed.delete(key)) collapsed.add(key);
	}
</script>

{#snippet picker(side: Side, label: string, name: string | undefined)}
	<button class="ref" aria-haspopup="dialog" aria-expanded={picking === side} title={name} onclick={() => (picking = side)}>
		<span class="role">{label}</span>
		<span class="branch" class:unset={!name}>{name ?? 'Choose a branch'}</span>
		<CaretUpDown aria-hidden="true" />
	</button>
{/snippet}

<section aria-label="Compare">
	<div class="refs">
		{@render picker('target', 'Target', target)}
		{@render picker('source', 'Source', source)}
	</div>
	{#if failure}
		<p class="note failure" role="alert">{failure}</p>
	{:else if files?.length === 0}
		<p class="note" role="status">No changes</p>
	{:else if files}
		<div class="heading">
			<span class="name">Changed files</span>
			<span class="count">{files.length} {files.length === 1 ? 'file' : 'files'}</span>
		</div>
		<ul class="list" aria-label="Compared files">
			{#each rows as row (row.key)}
				<li class="row">
					{#if row.kind === 'folder'}
						<button class="entry" style:--depth={row.depth} aria-expanded={row.open} title={row.folder.path} onclick={() => toggle(row.key)}>
							<span class="caret" class:open={row.open}><CaretRight /></span>
							<span class="name">{row.folder.name}</span>
						</button>
					{:else}
						{@const { change } = row}
						<button
							class="entry file"
							class:deleted={change.kind === 'deleted'}
							style:--depth={row.depth}
							style:--git="var(--git-{change.kind})"
							title={change.from ? `${change.from} → ${change.path}` : change.path}
							onclick={() => target && source && onopen(change, { target, source })}
						>
							<span class="name">{split(change.path).name}</span>
							<span class="letter" aria-hidden="true">{letters[change.kind]}</span>
						</button>
					{/if}
				</li>
			{/each}
		</ul>
	{:else if !target || !source}
		<p class="note">Choose a Target and a Source to compare.</p>
	{/if}
</section>

<Dialog
	bind:open={() => !!picking, (open) => !open && (picking = undefined)}
	full={form() === 'phone'}
	title={picking === 'target' ? 'Choose the Target' : 'Choose the Source'}
	description={picking === 'target' ? 'The branch the Source split off.' : 'The branch whose changes to show.'}
>
	{#if picking}
		{@const side = picking}
		<BranchPicker {projectId} current={side === 'target' ? target : source} origin={!!git.origin} onpick={(name) => pick(side, name)} />
	{/if}
</Dialog>

<style>
	section {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	.refs {
		display: grid;
		gap: var(--space-1);
		padding: 0 var(--space-2) var(--space-2);
	}

	.ref {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: none;
		border-radius: calc(var(--target-min) / 2);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		text-align: left;
		cursor: pointer;
	}

	.ref:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.ref:focus-visible {
		outline: var(--border-width) solid var(--status-working);
	}

	.ref :global(svg) {
		flex: none;
	}

	.role {
		flex: none;
		width: 4.5em;
	}

	.branch {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		color: var(--color-fg);
		font-family: var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.branch.unset {
		color: var(--muted-fg);
		font-family: inherit;
	}

	.note {
		margin: 0 var(--space-3);
		padding: var(--space-4) 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-align: center;
		overflow-wrap: anywhere;
	}

	.failure {
		padding: var(--space-2);
		border-left: 3px solid var(--git-deleted);
		background: var(--surface-1);
		color: var(--color-fg);
		text-align: left;
	}

	.heading {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		padding: 0 var(--space-2) 0 var(--space-3);
		border-block: var(--border-width) solid var(--border);
		background: var(--surface-2);
	}

	.heading .name {
		font-size: var(--font-size-sm);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.list {
		flex: 1;
		margin: 0;
		overflow: auto;
		padding: 0 0 var(--space-3);
		list-style: none;
	}

	.row {
		display: flex;
		align-items: center;
		min-height: var(--target-min);
	}

	.row:hover {
		background: var(--hover);
	}

	.entry {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		min-height: var(--target-min);
		padding: 0 var(--space-2) 0 calc(var(--space-2) + var(--depth) * var(--space-4));
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.entry:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: calc(-1 * var(--border-width));
	}

	.file {
		padding-left: calc(var(--space-2) + var(--depth) * var(--space-4) + 1em + var(--space-1));
	}

	.caret {
		display: inline-flex;
		flex: none;
		width: 1em;
		color: var(--muted-fg);
		transition: transform var(--dur-fast);
	}

	.caret.open {
		transform: rotate(90deg);
	}

	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.entry .name {
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
	}

	.file .name {
		color: var(--git);
	}

	.deleted .name {
		text-decoration: line-through;
	}

	.letter,
	.count {
		flex: none;
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	.letter {
		color: var(--git);
		font-weight: 700;
	}

	.count {
		color: var(--muted-fg);
	}
</style>
