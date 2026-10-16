<script lang="ts">
	import { editor as settings } from '#lib/config.svelte.ts';
	import type { Device } from '#lib/device.ts';
	import { language } from '#lib/editor/languages.ts';
	import { Sync } from '#lib/editor/sync.svelte.ts';
	import { theme } from '#lib/editor/theme.ts';
	import { read, type OpenFile } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import Conflict from '#lib/shell/Conflict.svelte';
	import Button from '#lib/ui/Button.svelte';
	import { Compartment, EditorState, type Extension } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import { onMount } from 'svelte';
	import FloppyDisk from '~icons/ph/floppy-disk';

	type Props = { file: OpenFile; device: Device; active: boolean; onshowdiff(disk: string, buffer: string): void };

	let { file, device, active, onshowdiff }: Props = $props();

	let container: HTMLDivElement;
	let view = $state.raw<EditorView>();
	let note = $state<string>();
	const sync = new Sync(() => file, () => view);
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
				view = new EditorView({
					parent: container,
					state: EditorState.create({
						doc: contents.content,
						extensions: [
							keys.of([]),
							keymap.of([{ key: 'Mod-s', run: () => (sync.write(), true) }]),
							basicSetup,
							syntax,
							theme,
							sync.listener
						]
					})
				});
				sync.start(contents.hash, view.state.doc);
			},
			(error: Error) => (note = error.message)
		);
		const unlisten = sync.listen(device);
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
		if (active && view && form() === 'desktop') view.focus();
	});

	async function keybindings(name: 'standard' | 'vim'): Promise<Extension> {
		return name === 'vim' ? (await import('@replit/codemirror-vim')).vim() : [];
	}
</script>

<div class="editor" role="tabpanel" hidden={!active}>
	<div class="toolbar">
		<span class="path" title={file.path}>{file.path}</span>
		{#if file.deleted}<span class="deleted">deleted on disk</span>{/if}
		{#if form() === 'phone' && view}
			<Button onclick={() => sync.write()} disabled={!file.dirty}><FloppyDisk /> Save</Button>
		{/if}
	</div>
	<Conflict {sync} onshowdiff={(disk) => view && onshowdiff(disk, view.state.doc.toString())} />
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
