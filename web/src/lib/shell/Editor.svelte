<script lang="ts">
	import { editor as settings } from '#lib/config.svelte.ts';
	import type { Device } from '#lib/device.ts';
	import { changeMarkers, setBaseline } from '#lib/editor/changes.ts';
	import { language } from '#lib/editor/languages.ts';
	import { theme } from '#lib/editor/theme.ts';
	import { read, RequestError, save, type OpenFile } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { baseline, type Git } from '#lib/git.ts';
	import Button from '#lib/ui/Button.svelte';
	import { Compartment, EditorState, type Extension, type Text } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import { onMount } from 'svelte';
	import FloppyDisk from '~icons/ph/floppy-disk';
	import Warning from '~icons/ph/warning';

	type Disk = { content: string; hash: string };

	let { file, device, git, active }: { file: OpenFile; device: Device; git?: Git; active: boolean } = $props();

	let container: HTMLDivElement;
	let view = $state.raw<EditorView>();
	let note = $state<string>();
	let hash = '';
	let saved: Text;
	let saving = false;
	let stale = false;
	let generation = 0;
	let conflict = $state<Disk>();
	const keys = new Compartment();

	onMount(() => {
		let disposed = false;
		Promise.all([read(file.projectId, file.path), language(file.path)]).then(
			([contents, syntax]) => {
				if (disposed) return;
				if (contents.binary) {
					note = 'binary file';
					return;
				}
				hash = contents.hash;
				view = new EditorView({
					parent: container,
					state: EditorState.create({
						doc: contents.content,
						extensions: [
							keys.of([]),
							keymap.of([{ key: 'Mod-s', run: () => (write(), true) }]),
							basicSetup,
							changeMarkers,
							syntax,
							theme,
							EditorView.updateListener.of((update) => {
								if (update.docChanged) file.dirty = !update.state.doc.eq(saved);
							})
						]
					})
				});
				saved = view.state.doc;
			},
			(error: Error) => (note = error.message)
		);
		const unlisten = device.listen({
			message(message) {
				if (message.type !== 'fs.changed' || message.projectId !== file.projectId) return;
				if (message.paths.some((path) => file.path === path || file.path.startsWith(`${path}/`))) refresh();
			}
		});
		return () => {
			disposed = true;
			unlisten();
			view?.destroy();
		};
	});

	$effect(() => {
		const current = view;
		if (!current) return;
		let stale = false;
		keybindings(settings.keymap).then((extension) => {
			if (!stale) current.dispatch({ effects: keys.reconfigure(extension) });
		});
		return () => {
			stale = true;
		};
	});

	$effect(() => {
		const current = view;
		if (!current) return;
		let stale = false;
		baseline(file.projectId, file.path, git).then((content) => {
			if (!stale) current.dispatch({ effects: setBaseline.of(content === null ? null : current.state.toText(content)) });
		});
		return () => {
			stale = true;
		};
	});

	$effect(() => {
		if (active && view && form() === 'desktop') view.focus();
	});

	async function keybindings(name: 'standard' | 'vim'): Promise<Extension> {
		return name === 'vim' ? (await import('@replit/codemirror-vim')).vim() : [];
	}

	async function write() {
		if (!view || !file.dirty || saving) return;
		const doc = view.state.doc;
		saving = true;
		generation++;
		try {
			const result = await save(file.projectId, file.path, doc.toString(), hash);
			if (result.conflict) {
				if (!result.conflict.binary) conflict = result.conflict;
				return;
			}
			hash = result.hash;
			saved = doc;
			conflict = undefined;
			file.dirty = !view.state.doc.eq(saved);
		} finally {
			saving = false;
			if (stale) {
				stale = false;
				refresh();
			}
		}
	}

	async function refresh() {
		if (!view) return;
		if (saving) {
			stale = true;
			return;
		}
		const current = ++generation;
		try {
			const contents = await read(file.projectId, file.path);
			if (current !== generation) return;
			file.deleted = false;
			if (contents.binary) return;
			if (contents.hash === hash) conflict = undefined;
			else if (file.dirty) conflict = contents;
			else load(contents);
		} catch (error) {
			if (current === generation && error instanceof RequestError && error.status === 404) file.deleted = true;
		}
	}

	function load(disk: Disk) {
		if (!view) return;
		const doc = view.state.doc.toString();
		const next = disk.content;
		let from = 0;
		while (from < doc.length && from < next.length && doc[from] === next[from]) from++;
		let end = 0;
		while (end < doc.length - from && end < next.length - from && doc.at(-1 - end) === next.at(-1 - end)) end++;
		view.dispatch({ changes: { from, to: doc.length - end, insert: next.slice(from, next.length - end) } });
		saved = view.state.doc;
		hash = disk.hash;
		conflict = undefined;
		file.dirty = false;
	}

	function keepMine() {
		if (!view || !conflict) return;
		saved = view.state.toText(conflict.content);
		hash = conflict.hash;
		conflict = undefined;
		file.dirty = !view.state.doc.eq(saved);
	}
</script>

<div class="editor" role="tabpanel" hidden={!active}>
	<div class="toolbar">
		<span class="path" title={file.path}>{file.path}</span>
		{#if file.deleted}<span class="deleted">deleted on disk</span>{/if}
		{#if form() === 'phone' && view}
			<Button onclick={write} disabled={!file.dirty}><FloppyDisk /> Save</Button>
		{/if}
	</div>
	{#if conflict}
		<div class="banner" role="alert">
			<span class="message"><Warning /> Changed on disk while you have unsaved edits.</span>
			<div class="choices">
				<Button onclick={keepMine}>Keep mine</Button>
				<Button onclick={() => conflict && load(conflict)}>Load disk</Button>
				<Button disabled title="Diff tabs are not available yet">Show diff</Button>
			</div>
		</div>
	{/if}
	<div class="content" bind:this={container}>
		{#if note}<p>{note}</p>{/if}
	</div>
</div>

<style>
	.editor {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.editor[hidden] {
		display: none;
	}

	.toolbar {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-1) 0 var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
	}

	.path {
		flex: 1;
		overflow: hidden;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.deleted {
		flex: none;
		color: var(--status-waiting);
		font-size: var(--font-size-sm);
	}

	.banner {
		display: flex;
		flex: none;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		padding: var(--space-2) var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
		background: color-mix(in oklab, var(--status-waiting) 16%, var(--color-bg));
	}

	.message {
		display: inline-flex;
		flex: 1 1 16rem;
		align-items: center;
		gap: var(--space-2);
	}

	.message :global(svg) {
		flex: none;
		color: var(--status-waiting);
	}

	.choices {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.content {
		flex: 1;
		min-height: 0;
	}

	p {
		margin: var(--space-6);
		color: var(--muted-fg);
		text-align: center;
	}
</style>
