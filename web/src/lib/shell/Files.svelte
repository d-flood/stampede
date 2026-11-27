<script lang="ts">
	import type { Device } from '#lib/device.svelte.ts';
	import { copy, create, download, list, remove, upload, RequestError, type Cursor, type Entry } from '#lib/files.svelte.ts';
	import { letters, type Change } from '#lib/git.ts';
	import Rename from '#lib/shell/Rename.svelte';
	import Search from '#lib/shell/Search.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import TreeRow from '#lib/ui/TreeRow.svelte';
	import { onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import Clipboard from '~icons/ph/clipboard';
	import Copy from '~icons/ph/copy';
	import DotsThree from '~icons/ph/dots-three';
	import DownloadSimple from '~icons/ph/download-simple';
	import FilePlus from '~icons/ph/file-plus';
	import FolderPlus from '~icons/ph/folder-plus';
	import PencilSimple from '~icons/ph/pencil-simple';
	import Trash from '~icons/ph/trash';
	import UploadSimple from '~icons/ph/upload-simple';

	type Props = { projectId: string; device: Device; changes: Change[]; hidden: boolean; onopen(path: string, cursor?: Cursor): void; onrename(from: string, to: string): void };

	let { projectId, device, changes, hidden, onopen, onrename }: Props = $props();

	let query = $state('');
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
	let picker: HTMLInputElement;
	let uploadDirectory = '';
	let uploading = $state(false);
	let uploadStatus = $state('');
	let uploadError = $state('');
	let dropDirectory = $state<string>();
	let renamer: Rename;
	let search: Search;
	let treeElement = $state<HTMLDivElement>();
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

	export function focusSearch() {
		search.focus();
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
			{ label: 'Upload Files…', icon: UploadSimple, disabled: uploading, onselect: () => chooseUpload(path) },
			{ label: 'Paste', icon: Clipboard, disabled: !copied || pasting, onselect: () => paste(path) }
		];
	}

	function chooseUpload(path: string) {
		uploadDirectory = path;
		picker.click();
	}

	function picked() {
		const files = Array.from(picker.files ?? []);
		picker.value = '';
		uploadFiles(uploadDirectory, files);
	}

	async function uploadFiles(directory: string, files: File[], errors: string[] = []) {
		if (uploading || (!files.length && !errors.length)) return;
		uploading = true;
		uploadError = '';
		try {
			for (const [index, file] of files.entries()) {
				uploadStatus = `Uploading ${index + 1} of ${files.length}…`;
				try {
					await upload(projectId, directory ? `${directory}/${file.name}` : file.name, file);
				} catch (cause) {
					errors.push(`${file.name}: ${cause instanceof Error ? cause.message : String(cause)}`);
				}
			}
			if (directory) expanded.add(directory);
			reload();
		} finally {
			uploadError = errors.join('\n');
			uploadStatus = '';
			uploading = false;
		}
	}

	function dropDestination(event: DragEvent) {
		return event.target instanceof Element ? event.target.closest<HTMLElement>('[data-upload-directory]')?.dataset.uploadDirectory ?? '' : '';
	}

	function dragover(event: DragEvent) {
		if (!event.dataTransfer?.types.includes('Files')) return;
		event.preventDefault();
		const allowed = !uploading && !query;
		event.dataTransfer.dropEffect = allowed ? 'copy' : 'none';
		dropDirectory = allowed ? dropDestination(event) : undefined;
	}

	function dragleave(event: DragEvent & { currentTarget: HTMLElement }) {
		if (!(event.relatedTarget instanceof Node) || !event.currentTarget.contains(event.relatedTarget)) dropDirectory = undefined;
	}

	function drop(event: DragEvent) {
		if (!event.dataTransfer?.types.includes('Files')) return;
		event.preventDefault();
		dropDirectory = undefined;
		if (query) return;
		const files: File[] = [];
		const errors: string[] = [];
		for (const item of event.dataTransfer.items) {
			const file = item.getAsFile();
			if (!file) continue;
			if (item.webkitGetAsEntry()?.isDirectory) errors.push(`${file.name}: Directories cannot be uploaded`);
			else files.push(file);
		}
		uploadFiles(dropDestination(event), files, errors);
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
				...(!entry.dir ? [{ label: 'Download', icon: DownloadSimple, onselect: () => download(projectId, entry.path) }] : []),
				...creationItems(entry.dir ? entry.path : path),
				{ label: 'Rename', icon: PencilSimple, onselect: () => renamer.start(entry.path, entry.dir) },
				{ label: 'Delete', icon: Trash, onselect: () => confirm(entry) }
			]}
			<li>
				<div class="row" class:dropTarget={entry.dir && dropDirectory === entry.path} data-upload-directory={entry.dir ? entry.path : path} onfocusin={() => (focused = entry)}>
					<Menu context {items}>
						{#snippet trigger(props)}
							<TreeRow
								{...props}
								tabindex={0}
								name={entry.name}
								{depth}
								open={entry.dir ? open : undefined}
								git={changed}
								letter={kind ? letters[kind] : changed ? '•' : undefined}
								muted={entry.ignored}
								title={kind ? `${entry.path} (${kind})` : entry.path}
								onclick={(event) => {
									if (event.currentTarget.dataset.state !== 'open') {
										if (entry.dir) toggle(entry.path);
										else onopen(entry.path);
									}
								}}
							/>
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

<section aria-label="Files" {hidden} ondragover={dragover} ondragleave={dragleave} ondrop={drop}>
	<input type="file" multiple hidden bind:this={picker} onchange={picked} />
	<Search bind:this={search} {projectId} {device} bind:query {includeIgnored} {uploading} onupload={() => chooseUpload('')} onignored={showIgnored} {onopen} />
	{#if !query}
		<Menu context items={creationItems('')}>
			{#snippet trigger(props)}
				<div {...props} bind:this={treeElement} class="tree" class:dropTarget={dropDirectory === ''} role="group" aria-label="File tree" aria-busy={pasting || uploading}>{@render tree('', 0)}</div>
			{/snippet}
		</Menu>
		{#if pasteError}<p class="notice" role="alert">{pasteError}</p>{/if}
		{#if uploadStatus}<p class="notice" role="status">{uploadStatus}</p>{/if}
		{#if uploadError}<p class="notice" role="alert">{uploadError}</p>{/if}
	{/if}
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

	section[hidden] {
		display: none;
	}

	.tree {
		flex: 1;
		overflow: auto;
		padding: 0 var(--space-2) var(--space-3);
	}

	.notice {
		margin: 0;
		padding: var(--space-1) var(--space-4);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.notice[role='alert'] {
		color: var(--ansi-1);
	}

	.tree.dropTarget, .row.dropTarget {
		background: var(--selected-bg);
		box-shadow: inset 0 0 0 var(--border-width) var(--selected-border);
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

	/* Shows row actions where a pointer can hover; touch keeps them visible. */
	@media (hover: none) {
		.row > :global(button:last-child:not(:first-child)) {
			opacity: 0.45;
		}
	}

	@media (hover: hover) {
		.row {
			position: relative;
		}

		.row > :global(button:last-child:not(:first-child)) {
			position: absolute;
			inset-block: 0;
			right: 0;
			min-height: 0;
			background: var(--chrome);
			opacity: 0;
			transition: opacity var(--dur-fast);
		}

		.row:hover > :global(button:last-child:not(:first-child)),
		.row:focus-within > :global(button:last-child:not(:first-child)),
		.row > :global(button[data-state='open']) {
			opacity: 1;
		}

		.row:hover > :global(button:last-child:not(:first-child)),
		.row > :global(button[data-state='open']) {
			background: var(--hover);
		}
	}
</style>
