<script lang="ts">
	import { editor as settings } from '#lib/config.svelte.ts';
	import { language } from '#lib/editor/languages.ts';
	import { theme } from '#lib/editor/theme.ts';
	import { read, save, type OpenFile } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import Button from '#lib/ui/Button.svelte';
	import { Compartment, EditorState, type Extension, type Text } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import { onMount } from 'svelte';
	import FloppyDisk from '~icons/ph/floppy-disk';

	let { file, active }: { file: OpenFile; active: boolean } = $props();

	let container: HTMLDivElement;
	let view = $state.raw<EditorView>();
	let note = $state<string>();
	let hash = '';
	let saved: Text;
	let saving = false;
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
		return () => {
			disposed = true;
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
		if (active && view && form() === 'desktop') view.focus();
	});

	async function keybindings(name: 'standard' | 'vim'): Promise<Extension> {
		return name === 'vim' ? (await import('@replit/codemirror-vim')).vim() : [];
	}

	async function write() {
		if (!view || !file.dirty || saving) return;
		const doc = view.state.doc;
		saving = true;
		try {
			hash = (await save(file.projectId, file.path, doc.toString(), hash)).hash;
			saved = doc;
			file.dirty = !view.state.doc.eq(saved);
		} finally {
			saving = false;
		}
	}
</script>

<div class="editor" role="tabpanel" hidden={!active}>
	<div class="toolbar">
		<span class="path" title={file.path}>{file.path}</span>
		{#if form() === 'phone' && view}
			<Button onclick={write} disabled={!file.dirty}><FloppyDisk /> Save</Button>
		{/if}
	</div>
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
