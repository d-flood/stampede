<script lang="ts">
	import type { Device } from '#lib/device.svelte.ts';
	import { copy, create, list, remove, RequestError, type Entry } from '#lib/files.svelte.ts';
	import { letters, type Change } from '#lib/git.ts';
	import Rename from '#lib/shell/Rename.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import CaretRight from '~icons/ph/caret-right';
	import Clipboard from '~icons/ph/clipboard';
	import Copy from '~icons/ph/copy';
	import DotsThree from '~icons/ph/dots-three';
	import Eye from '~icons/ph/eye';
	import EyeSlash from '~icons/ph/eye-slash';
	import FilePlus from '~icons/ph/file-plus';
	import FolderPlus from '~icons/ph/folder-plus';
	import PencilSimple from '~icons/ph/pencil-simple';
	import Trash from '~icons/ph/trash';

	type Props = { projectId: string; device: Device; changes: Change[]; onopen(path: string): void; onrename(from: string, to: string): void };

	let { projectId, device, changes, onopen, onrename }: Props = $props();

	let includeIgnored = $state(false);
	let listings = $state.raw<Record<string, Entry[]>>({});
	let target = $state<Entry>();
	let confirming = $state(false);
	let deleting = $state(false);
	let error = $state('');
	let destination = $state({ path: '', dir: false });
	let naming = $state(false);
	let name = $state('');
	let creating = $state(false);
	let createError = $state('');
	let copied = $state<Entry>();
	let pasting = $state(false);
	let pasteError = $state('');
	let renamer: Rename;
	let treeElement: HTMLDivElement;
	let focused: Entry | undefined;
	const expanded = new SvelteSet<string>();
	const kinds = $derived(new Map(changes.map((change) => [change.path, change.kind])));
	const marked = $derived(new Set(changes.flatMap(({ path, from }) => [...parents(path), ...parents(from ?? '')])));

	onMount(() => {
		load('');
		return device.listen({
			message(message) {
				if (message.type === 'hello') return reload();
				if (message.type !== 'fs.changed' || message.projectId !== projectId) return;
				const dirs = new Set(message.paths.map((path) => path.slice(0, Math.max(path.lastIndexOf('/'), 0))));
				for (const dir of dirs) if (dir === '' || expanded.has(dir)) load(dir);
			}
		});
	});

	function parents(path: string) {
		const parts = path.split('/');
		return parts.slice(1).map((_, index) => parts.slice(0, index + 1).join('/'));
	}

	async function load(path: string) {
		const entries = await list(projectId, path, includeIgnored).catch((error) => {
			if (error instanceof RequestError && error.status === 404) return [];
			throw error;
		});
		listings = { ...listings, [path]: entries };
	}

	function toggle(path: string) {
		if (expanded.delete(path)) return;
		expanded.add(path);
		load(path);
	}

	function reload() {
		for (const path of ['', ...expanded]) load(path);
	}

	function showIgnored() {
		includeIgnored = !includeIgnored;
		reload();
	}

	function confirm(entry: Entry) {
		if (deleting) return;
		target = entry;
		error = '';
		confirming = true;
	}

	function creationItems(path: string) {
		return [
			{ label: 'New File', icon: FilePlus, onselect: () => startCreate(path, false) },
			{ label: 'New Directory', icon: FolderPlus, onselect: () => startCreate(path, true) },
			{ label: 'Paste', icon: Clipboard, disabled: !copied || pasting, onselect: () => paste(path) }
		];
	}

	function copyEntry(entry: Entry) {
		copied = entry;
		pasteError = '';
	}

	async function paste(path: string) {
		if (!copied || pasting) return;
		pasting = true;
		pasteError = '';
		try {
			await copy(projectId, copied.path, path);
			if (path) expanded.add(path);
			reload();
		} catch (cause) {
			pasteError = cause instanceof Error ? cause.message : String(cause);
		} finally {
			pasting = false;
		}
	}

	function keydown(event: KeyboardEvent) {
		if (event.defaultPrevented || event.isComposing || event.altKey || event.shiftKey) return;
		if (!(event.target instanceof HTMLElement) || !treeElement?.contains(event.target)) return;
		const entry = event.target === treeElement ? undefined : focused;
		if (event.key === 'F2' && entry && !event.ctrlKey && !event.metaKey) {
			event.preventDefault();
			renamer.start(entry.path, entry.dir);
		}
		if (event.ctrlKey === event.metaKey) return;
		const key = event.key.toLowerCase();
		if (key === 'c' && entry) {
			event.preventDefault();
			if (!event.repeat) copyEntry(entry);
		} else if (key === 'v' && copied) {
			event.preventDefault();
			if (!event.repeat) paste(entry?.dir ? entry.path : entry?.path.slice(0, Math.max(entry.path.lastIndexOf('/'), 0)) ?? '');
		}
	}

	function startCreate(path: string, dir: boolean) {
		if (creating) return;
		destination = { path, dir };
		name = '';
		createError = '';
		naming = true;
	}

	async function createEntry(event: SubmitEvent) {
		event.preventDefault();
		if (!name.trim() || creating) return;
		const path = destination.path ? `${destination.path}/${name.trim()}` : name.trim();
		creating = true;
		try {
			await create(projectId, path, destination.dir);
			if (destination.path) expanded.add(destination.path);
			reload();
			naming = false;
			if (!destination.dir) onopen(path);
		} catch (cause) {
			createError = cause instanceof RequestError && cause.status === 409
				? 'A file or directory with that name already exists.'
				: cause instanceof Error ? cause.message : String(cause);
		} finally {
			creating = false;
		}
	}

	function renamed(from: string, to: string) {
		const kept = { ...listings };
		for (const path of [...expanded]) {
			if (path === from || path.startsWith(`${from}/`)) {
				expanded.delete(path);
				expanded.add(to + path.slice(from.length));
				delete kept[path];
			}
		}
		listings = kept;
		reload();
		onrename(from, to);
	}

	async function deleteEntry() {
		if (!target || deleting) return;
		deleting = true;
		try {
			await remove(projectId, target.path);
			const kept = { ...listings };
			for (const path of expanded) {
				if (path === target.path || path.startsWith(`${target.path}/`)) {
					expanded.delete(path);
					delete kept[path];
				}
			}
			listings = kept;
			reload();
			confirming = false;
		} catch (cause) {
			error = cause instanceof Error ? cause.message : String(cause);
		} finally {
			deleting = false;
		}
	}
</script>

<svelte:window onkeydown={keydown} />

{#snippet tree(path: string, depth: number)}
	<ul>
		{#each listings[path] ?? [] as entry (entry.path)}
			{@const open = entry.dir && expanded.has(entry.path)}
			{@const kind = entry.dir ? undefined : kinds.get(entry.path)}
			{@const changed = kind ?? (entry.dir && marked.has(entry.path) ? 'modified' : undefined)}
			{@const items = [
				{ label: 'Copy', icon: Copy, onselect: () => copyEntry(entry) },
				...creationItems(entry.dir ? entry.path : path),
				{ label: 'Rename', icon: PencilSimple, onselect: () => renamer.start(entry.path, entry.dir) },
				{ label: 'Delete', icon: Trash, onselect: () => confirm(entry) }
			]}
			<li>
				<div class="row" onfocusin={() => (focused = entry)}>
					<Menu context {items}>
						{#snippet trigger(props)}
							<button
								{...props}
								tabindex={0}
								class="entry"
								class:ignored={entry.ignored}
								class:changed={changed}
								style:--depth={depth}
								style:--git={changed && `var(--git-${changed})`}
								aria-expanded={entry.dir ? open : undefined}
								title={kind ? `${entry.path} (${kind})` : entry.path}
								onclick={(event) => {
									if (event.currentTarget.dataset.state !== 'open') {
										if (entry.dir) toggle(entry.path);
										else onopen(entry.path);
									}
								}}
							>
								<span class="caret" class:open>{#if entry.dir}<CaretRight />{/if}</span>
								<span class="name">{entry.name}</span>
								{#if kind}
									<span class="mark" aria-hidden="true">{letters[kind]}</span>
								{:else if entry.dir && marked.has(entry.path)}
									<span class="mark" aria-hidden="true">•</span>
								{/if}
							</button>
						{/snippet}
					</Menu>
					<Menu {items}>
						{#snippet trigger(props)}
							<IconButton {...props} label={`Actions for ${entry.path}`}><DotsThree /></IconButton>
						{/snippet}
					</Menu>
				</div>
				{#if open}{@render tree(entry.path, depth + 1)}{/if}
			</li>
		{/each}
	</ul>
{/snippet}

<section aria-label="Files">
	<header>
		<h2>Files</h2>
		<IconButton
			label={includeIgnored ? 'Hide ignored files' : 'Show ignored files'}
			aria-pressed={includeIgnored}
			onclick={showIgnored}
		>
			{#if includeIgnored}<Eye />{:else}<EyeSlash />{/if}
		</IconButton>
	</header>
	<Menu context items={creationItems('')}>
		{#snippet trigger(props)}
			<div {...props} bind:this={treeElement} class="tree" role="group" aria-label="File tree" aria-busy={pasting}>{@render tree('', 0)}</div>
		{/snippet}
	</Menu>
	{#if pasteError}<p role="alert">{pasteError}</p>{/if}
</section>

<Dialog
	bind:open={naming}
	title={destination.dir ? 'New Directory' : 'New File'}
	description={`Create in ${destination.path || 'Project root'}`}
>
	<form id="create-entry" onsubmit={createEntry}>
		<label for="entry-name">Name</label>
		<TextField id="entry-name" bind:value={name} required pattern="[^/]+" autocomplete="off" spellcheck="false" disabled={creating} />
		{#if createError}<p role="alert">{createError}</p>{/if}
	</form>
	{#snippet actions()}
		<Button disabled={creating} onclick={() => (naming = false)}>Cancel</Button>
		<Button type="submit" form="create-entry" variant="primary" disabled={creating || !name.trim()}>{creating ? 'Creating…' : 'Create'}</Button>
	{/snippet}
</Dialog>

<Rename bind:this={renamer} {projectId} onrename={renamed} />

<Dialog
	bind:open={confirming}
	title={target?.dir ? 'Delete directory?' : 'Delete file?'}
	description={target ? `Permanently delete “${target.path}”${target.dir ? ' and all its contents' : ''}? This cannot be undone.` : ''}
>
	{#if error}<p role="alert">{error}</p>{/if}
	{#snippet actions()}
		<Button disabled={deleting} onclick={() => (confirming = false)}>Cancel</Button>
		<Button variant="danger" disabled={deleting} onclick={deleteEntry}>{deleting ? 'Deleting…' : 'Delete'}</Button>
	{/snippet}
</Dialog>

<style>
	section {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	header {
		display: flex;
		align-items: center;
		padding: 0 var(--space-1) 0 var(--space-3);
	}

	h2 {
		flex: 1;
		margin: 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-weight: 600;
	}

	.tree {
		flex: 1;
		overflow: auto;
		padding: 0 var(--space-2) var(--space-3);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	label {
		display: block;
		margin-bottom: var(--space-1);
	}

	.row {
		display: flex;
		align-items: center;
		border-radius: var(--radius-md);
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
		padding: 0 var(--space-3) 0 calc(var(--space-2) + var(--depth) * var(--space-4));
		border: none;
		border-radius: inherit;
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

	.ignored {
		color: var(--muted-fg);
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
		overflow: hidden;
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.changed .name {
		color: var(--git);
	}

	.mark {
		flex: none;
		color: var(--git, var(--muted-fg));
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		font-weight: 700;
	}
</style>
