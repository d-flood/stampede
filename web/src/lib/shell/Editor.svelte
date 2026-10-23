<script lang="ts">
	import { editor as settings } from '#lib/config.svelte.ts';
	import type { Device } from '#lib/device.svelte.ts';
	import { changeMarkers, setBaseline } from '#lib/editor/changes.ts';
	import { keybindings } from '#lib/editor/keymap.ts';
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
	import ImagePreview from '#lib/shell/ImagePreview.svelte';
	import MarkdownPreview from '#lib/shell/MarkdownPreview.svelte';
	import ServerSwitch from '#lib/shell/ServerSwitch.svelte';
	import Button from '#lib/ui/Button.svelte';
	import { Compartment, EditorState } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import { onMount, untrack } from 'svelte';
	import FloppyDisk from '~icons/ph/floppy-disk';
	import Warning from '~icons/ph/warning';

	type Props = {
		file: OpenFile;
		device: Device;
		servers: LanguageServers;
		git?: Git;
		active: boolean;
		displayed: boolean;
		focused: boolean;
		onshowdiff(disk: string, buffer: string): void;
	};

	let { file, device, servers, git, active, displayed, focused, onshowdiff }: Props = $props();

	let container: HTMLDivElement;
	let view = $state.raw<EditorView>();
	let note = $state<string>();
	let wrap = $state(false);
	const svg = $derived(/\.svg$/i.test(file.path) && !(file instanceof OpenConfig));
	const markdown = $derived(/\.(md|markdown)$/i.test(file.path) && !(file instanceof OpenConfig));
	const original = $derived(git?.isRepo ? (git.changes.find((change) => change.path === file.path)?.from ?? file.path) : undefined);
	const head = $derived(git?.head);
	let rendered = $state(false);
	let content = $state<string>();
	const sync = new Sync(() => file, () => view);
	const keys = new Compartment();
	const wrapping = new Compartment();
	const syntax = new Compartment();
	const server = new Compartment();
	let loaded: string;

	onMount(() => {
		let disposed = false;
		rendered = svg && !file.cursor;
		wrap = markdown;
		loaded = file.path;
		Promise.all([file.read(), language(loaded)]).then(
			([contents, highlighting]) => {
				if (disposed) return;
				if (contents.binary) {
					note = 'binary file';
					return;
				}
				if (svg || markdown) content = contents.content;
				view = new EditorView({
					parent: container,
					state: EditorState.create({
						doc: contents.content,
						extensions: [
							keys.of([]),
							wrapping.of(wrap ? EditorView.lineWrapping : []),
							keymap.of([{ key: 'Mod-s', run: () => (sync.write(), true) }]),
							basicSetup,
							changeMarkers,
							syntax.of(highlighting),
							theme,
							sync.listener,
							svg || markdown ? EditorView.updateListener.of((update) => {
								if (update.docChanged) content = update.state.doc.toString();
							}) : [],
							server.of(languageServer(servers, file.projectId, loaded, true))
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

	function toggleWrap() {
		if (!view) return;
		wrap = !wrap;
		view.dispatch({ effects: wrapping.reconfigure(wrap ? EditorView.lineWrapping : []) });
	}

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
		const [current, path, commit] = [view, original, head];
		if (!current || file instanceof OpenConfig) return;
		let stale = false;
		(path === undefined ? Promise.resolve(null) : baseline(file.projectId, commit, path)).then((content) => {
			if (!stale) current.dispatch({ effects: setBaseline.of(content === null ? null : current.state.toText(content)) });
		});
		return () => {
			stale = true;
		};
	});

	$effect(() => {
		const [current, path] = [view, file.path];
		if (!current || path === loaded) return;
		loaded = path;
		untrack(() => sync.refresh());
		language(path).then((extension) => {
			if (loaded === path) current.dispatch({ effects: [syntax.reconfigure(extension), server.reconfigure(languageServer(servers, file.projectId, path, true))] });
		});
	});

	$effect(() => {
		const current = view;
		if (current) servers.displayed(current, displayed && !rendered);
	});

	$effect(() => {
		if (active && (note || (!rendered && view) || (markdown && rendered && content !== undefined))) painted(file.id);
	});

	$effect(() => {
		if (focused && !rendered && view && form() === 'desktop') view.focus();
	});

	$effect(() => {
		const cursor = file.cursor;
		if (!view || !cursor) return;
		rendered = false;
		const { doc } = view.state;
		const line = doc.line(Math.min(Math.max(cursor.line, 1), doc.lines));
		const anchor = Math.min(line.from + cursor.column - 1, line.to);
		view.dispatch({ selection: { anchor }, effects: EditorView.scrollIntoView(anchor, { y: 'center' }) });
		file.cursor = undefined;
	});
</script>

<div class="editor" role="tabpanel" hidden={!active}>
	<div class="toolbar" class:previewable={svg || markdown}>
		<span class="path" title={file.path}>{file.path}</span>
		{#if file.deleted}<span class="deleted">deleted on disk</span>{/if}
		{#if svg}
			<Button aria-pressed={rendered} variant={rendered ? 'primary' : 'default'} onclick={() => rendered = true}>Rendered</Button>
			<Button aria-pressed={!rendered} variant={rendered ? 'default' : 'primary'} onclick={() => rendered = false}>Source</Button>
		{:else if markdown}
			<Button aria-pressed={!rendered} variant={rendered ? 'default' : 'primary'} onclick={() => rendered = false}>Edit</Button>
			<Button aria-pressed={rendered} variant={rendered ? 'primary' : 'default'} onclick={() => rendered = true}>Preview</Button>
		{/if}
		{#if !rendered}<ServerSwitch {servers} projectId={file.projectId} path={file.path} attached />{/if}
		{#if view && !rendered}
			<Button aria-label="Line wrapping" aria-pressed={wrap} variant={wrap ? 'primary' : 'default'} onclick={toggleWrap}>Wrap</Button>
		{/if}
		{#if (form() === 'phone' || rendered) && view}
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
	<div class="content" hidden={rendered} bind:this={container}>
		{#if note}<p>{note}</p>{/if}
	</div>
	{#if rendered}
		<div class="content">
			{#if note}<p role="alert">{note}</p>
			{:else if content !== undefined}
				{#if markdown}<MarkdownPreview text={content} />
				{:else}<ImagePreview {file} {device} {active} {content} />{/if}
			{/if}
		</div>
	{/if}
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

	.toolbar.previewable {
		flex-wrap: wrap;
	}

	.toolbar.previewable .path {
		flex-basis: 8rem;
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

	.content[hidden] {
		display: none;
	}

	p {
		margin: var(--space-6);
		color: var(--muted-fg);
		text-align: center;
	}
</style>
