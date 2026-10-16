<script lang="ts">
	import { editor as settings } from '#lib/config.svelte.ts';
	import type { Device } from '#lib/device.ts';
	import { changeMarkers, setBaseline } from '#lib/editor/changes.ts';
	import { language } from '#lib/editor/languages.ts';
	import { languageServer } from '#lib/editor/lsp-editor.ts';
	import type { LanguageServers } from '#lib/editor/lsp.svelte.ts';
	import { Sync } from '#lib/editor/sync.svelte.ts';
	import { theme } from '#lib/editor/theme.ts';
	import { OpenConfig, type OpenFile } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { baseline, type Git } from '#lib/git.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import Conflict from '#lib/shell/Conflict.svelte';
	import ServerSwitch from '#lib/shell/ServerSwitch.svelte';
	import Button from '#lib/ui/Button.svelte';
	import { Compartment, EditorState, type Extension } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import { onMount } from 'svelte';
	import FloppyDisk from '~icons/ph/floppy-disk';
	import Warning from '~icons/ph/warning';

	type Props = {
		file: OpenFile;
		device: Device;
		servers: LanguageServers;
		git?: Git;
		active: boolean;
		focused: boolean;
		onshowdiff(disk: string, buffer: string): void;
	};

	let { file, device, servers, git, active, focused, onshowdiff }: Props = $props();

	let container: HTMLDivElement;
	let view = $state.raw<EditorView>();
	let note = $state<string>();
	const sync = new Sync(() => file, () => view);
	const keys = new Compartment();

	onMount(() => {
		let disposed = false;
		Promise.all([file.read(), language(file.path)]).then(
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
							changeMarkers,
							syntax,
							theme,
							sync.listener,
							languageServer(servers, file.projectId, file.path, true)
						]
					})
				});
				sync.start(contents.hash, view.state.doc);
			},
			(error: Error) => (note = error.message)
		);
		const unlisten = file instanceof OpenConfig ? undefined : sync.listen(device);
		return () => {
			disposed = true;
			unlisten?.();
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
		if (!current || file instanceof OpenConfig) return;
		let stale = false;
		baseline(file.projectId, file.path, git).then((content) => {
			if (!stale) current.dispatch({ effects: setBaseline.of(content === null ? null : current.state.toText(content)) });
		});
		return () => {
			stale = true;
		};
	});

	$effect(() => {
		if (active && (view || note)) painted(file.id);
	});

	$effect(() => {
		if (focused && view && form() === 'desktop') view.focus();
	});

	$effect(() => {
		const cursor = file.cursor;
		if (!view || !cursor) return;
		const { doc } = view.state;
		const line = doc.line(Math.min(Math.max(cursor.line, 1), doc.lines));
		const anchor = Math.min(line.from + cursor.column - 1, line.to);
		view.dispatch({ selection: { anchor }, effects: EditorView.scrollIntoView(anchor, { y: 'center' }) });
		file.cursor = undefined;
	});

	async function keybindings(name: 'standard' | 'vim'): Promise<Extension> {
		return name === 'vim' ? (await import('@replit/codemirror-vim')).vim() : [];
	}
</script>

<div class="editor" role="tabpanel" hidden={!active}>
	<div class="toolbar">
		<span class="path" title={file.path}>{file.path}</span>
		{#if file.deleted}<span class="deleted">deleted on disk</span>{/if}
		<ServerSwitch {servers} projectId={file.projectId} path={file.path} attached />
		{#if form() === 'phone' && view}
			<Button onclick={() => sync.write()} disabled={!file.dirty}><FloppyDisk /> Save</Button>
		{/if}
	</div>
	<Conflict {sync} onshowdiff={(disk) => view && onshowdiff(disk, view.state.doc.toString())} />
	{#if file instanceof OpenConfig && file.invalid}
		{@const { line, message } = file.invalid}
		<div class="invalid" role="alert">
			<span class="message"><Warning /> Not applied; the last good config stays in force.</span>
			<code>{line ? `Line ${line}: ` : ''}{message}</code>
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

	.invalid {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-2) var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
		background: color-mix(in oklab, var(--status-waiting) 16%, var(--color-bg));
	}

	.message {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}

	.message :global(svg) {
		flex: none;
		color: var(--status-waiting);
	}

	code {
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		white-space: pre-wrap;
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
