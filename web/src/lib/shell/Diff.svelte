<script lang="ts">
	import type { Device, ServerMessage } from '#lib/device.ts';
	import { language } from '#lib/editor/languages.ts';
	import { languageServer, type LanguageServers } from '#lib/editor/lsp.svelte.ts';
	import { Sync } from '#lib/editor/sync.svelte.ts';
	import { diffTheme, theme } from '#lib/editor/theme.ts';
	import { read, RequestError, type Contents, type OpenDiff } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { head, show } from '#lib/git.ts';
	import Conflict from '#lib/shell/Conflict.svelte';
	import ServerSwitch from '#lib/shell/ServerSwitch.svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import { getOriginalDoc, MergeView, originalDocChangeEffect, unifiedMergeView } from '@codemirror/merge';
	import { ChangeSet, EditorState, type Extension, type Text } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import { onMount, untrack } from 'svelte';
	import Columns from '~icons/ph/columns';
	import FloppyDisk from '~icons/ph/floppy-disk';
	import Rows from '~icons/ph/rows';

	type Props = {
		diff: OpenDiff;
		device: Device;
		servers: LanguageServers;
		active: boolean;
		onshowdiff(disk: string, buffer: string): void;
	};
	type Shown = { b: EditorView; original: Text; editable: boolean; setOriginal(content: string): void; destroy(): void };

	let { diff, device, servers, active, onshowdiff }: Props = $props();

	let container: HTMLDivElement;
	let shown = $state.raw<Shown>();
	let syntax = $state.raw<Extension>();
	let note = $state<string>();
	let heads = 0;
	const sync = new Sync(() => diff, () => shown?.b);
	const editable = $derived(diff.against === 'HEAD' && !diff.deleted);

	onMount(() => {
		let disposed = false;
		Promise.all([language(diff.path), sides()]).then(
			([extension, sides]) => {
				if (disposed) return;
				syntax = extension;
				if (!sides) return;
				const [a, b] = sides;
				if (b.binary) {
					note = 'binary file';
					return;
				}
				const doc = build(a, b.content).b.state.doc;
				if (diff.against === 'HEAD') sync.start(b.hash, doc);
			},
			(error: Error) => (note = error.message)
		);
		const unlisten = diff.against === 'HEAD' ? [sync.listen(device), device.listen({ message })] : [];
		return () => {
			disposed = true;
			for (const stop of unlisten) stop();
			shown?.destroy();
		};
	});

	$effect(() => {
		const snapshot = diff.snapshot;
		if (snapshot && syntax) untrack(() => build(snapshot.disk, snapshot.buffer));
	});

	$effect(() => {
		const now = editable;
		untrack(() => {
			if (shown && shown.editable !== now) build(shown.original, shown.b.state.doc);
		});
	});

	$effect(() => {
		if (active && shown && form() === 'desktop') shown.b.focus();
	});

	function sides(): Promise<[string, Contents]> | undefined {
		if (diff.against === 'HEAD') return Promise.all([headContent(), current()]);
		if (diff.against !== 'disk') return committed(diff.against.sha);
	}

	function headContent() {
		return head(diff.projectId, diff.from ?? diff.path);
	}

	async function committed(sha: string): Promise<[string, Contents]> {
		const parent = show(diff.projectId, `${sha}^`, diff.from ?? diff.path);
		const [a, b] = await Promise.all([parent, show(diff.projectId, sha, diff.path)]);
		return [a, a.includes('\0') || b.includes('\0') ? { binary: true, hash: '' } : { content: b, hash: '' }];
	}

	async function current(): Promise<Contents> {
		try {
			return await read(diff.projectId, diff.path);
		} catch (error) {
			if (!(error instanceof RequestError && error.status === 404)) throw error;
			diff.deleted = true;
			return { content: '', hash: '' };
		}
	}

	function message(message: ServerMessage) {
		if (message.type === 'git.changed' && message.projectId === diff.projectId) refreshHead();
	}

	async function refreshHead() {
		if (!shown) return;
		const generation = ++heads;
		const content = await headContent();
		if (generation === heads && shown && content !== shown.original.toString()) shown.setOriginal(content);
	}

	function toggle() {
		diff.layout = diff.layout === 'split' ? 'unified' : 'split';
		if (shown) build(shown.original, shown.b.state.doc);
	}

	function build(a: string | Text, b: string | Text): Shown {
		shown?.destroy();
		const shared = [basicSetup, syntax ?? [], theme, diffTheme];
		const working = [
			keymap.of([{ key: 'Mod-s', run: () => (sync.write(), true) }]),
			shared,
			EditorState.readOnly.of(!editable),
			sync.listener,
			diff.against === 'HEAD' ? languageServer(servers, diff.projectId, diff.path, false) : []
		];
		if (diff.layout === 'split') {
			const view = new MergeView({
				parent: container,
				a: { doc: a, extensions: [shared, EditorState.readOnly.of(true)] },
				b: { doc: b, extensions: working },
				revertControls: editable ? 'a-to-b' : undefined,
				collapseUnchanged: {}
			});
			return (shown = {
				b: view.b,
				editable,
				get original() {
					return view.a.state.doc;
				},
				setOriginal: (content) => view.a.dispatch({ changes: { from: 0, to: view.a.state.doc.length, insert: content } }),
				destroy: () => view.destroy()
			});
		} else {
			const view = new EditorView({
				parent: container,
				doc: b,
				extensions: [working, unifiedMergeView({ original: a, mergeControls: editable && revert, collapseUnchanged: {} })]
			});
			return (shown = {
				b: view,
				editable,
				get original() {
					return getOriginalDoc(view.state);
				},
				setOriginal(content) {
					const length = getOriginalDoc(view.state).length;
					const changes = ChangeSet.of({ from: 0, to: length, insert: content }, length);
					view.dispatch({ effects: originalDocChangeEffect(view.state, changes) });
				},
				destroy: () => view.destroy()
			});
		}
	}

	function revert(type: 'accept' | 'reject', action: (event: MouseEvent) => void) {
		// Accepting would change HEAD, which is read-only.
		if (type === 'accept') return document.createElement('span');
		const button = document.createElement('button');
		button.textContent = 'Revert';
		button.onclick = action;
		return button;
	}
</script>

<div class="diff" role="tabpanel" hidden={!active}>
	<div class="toolbar">
		<span class="path" title={diff.path}>{diff.from ? `${diff.from} → ${diff.path}` : diff.path}</span>
		{#if diff.against === 'disk'}
			<span class="sides">disk ↔ unsaved edits</span>
		{:else if diff.against !== 'HEAD'}
			<span class="sides" title={diff.against.subject}>{diff.against.short}^ ↔ {diff.against.short}</span>
		{/if}
		{#if diff.deleted}<span class="deleted">deleted on disk</span>{/if}
		<ServerSwitch {servers} projectId={diff.projectId} path={diff.path} attached={diff.against === 'HEAD'} />
		{#if shown}
			{#if diff.layout === 'split'}
				<IconButton label="Single column" title="Single column" onclick={toggle}><Rows /></IconButton>
			{:else}
				<IconButton label="Side by side" title="Side by side" onclick={toggle}><Columns /></IconButton>
			{/if}
			{#if form() === 'phone' && editable}
				<Button onclick={() => sync.write()} disabled={!diff.dirty}><FloppyDisk /> Save</Button>
			{/if}
		{/if}
	</div>
	<Conflict {sync} onshowdiff={(disk) => shown && onshowdiff(disk, shown.b.state.doc.toString())} />
	<div class="content" bind:this={container}>
		{#if note}<p>{note}</p>{/if}
	</div>
</div>

<style>
	.diff {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.diff[hidden] {
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

	.sides {
		flex: none;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
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

	.content :global(.cm-mergeView) {
		height: 100%;
	}

	.content :global(.cm-merge-revert) {
		width: var(--target-min);
		border-inline: var(--border-width) solid var(--border);
		background: var(--surface-1);
	}

	.content :global(.cm-merge-revert button) {
		min-height: var(--target-min);
		color: var(--muted-fg);
		font-family: var(--font-mono);
	}

	.content :global(.cm-merge-revert button:hover) {
		background: var(--hover);
		color: var(--color-fg);
	}

	p {
		margin: var(--space-6);
		color: var(--muted-fg);
		text-align: center;
	}
</style>
