<script lang="ts">
	import type { Device, ServerMessage } from '#lib/device.svelte.ts';
	import { language } from '#lib/editor/languages.ts';
	import { languageServer } from '#lib/editor/lsp-editor.ts';
	import type { LanguageServers } from '#lib/editor/lsp.svelte.ts';
	import { Sync } from '#lib/editor/sync.svelte.ts';
	import { diffTheme, theme } from '#lib/editor/theme.ts';
	import { read, RequestError, type Contents, type OpenDiff } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import { baseline, index, show, type Git } from '#lib/git.ts';
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
		git?: Git;
		device: Device;
		servers: LanguageServers;
		active: boolean;
		displayed: boolean;
		focused: boolean;
		onshowdiff(disk: string, buffer: string): void;
	};
	type Shown = { b: EditorView; original: Text; editable: boolean; setOriginal(content: string): void; destroy(): void };

	let { diff, git, device, servers, active, displayed, focused, onshowdiff }: Props = $props();

	let container: HTMLDivElement;
	let shown = $state.raw<Shown>();
	let syntax = $state.raw<Extension>();
	let note = $state<string>();
	let heads = 0;
	let missed = false;
	let left: string | undefined;
	let right: Contents | undefined;
	const sync = new Sync(() => diff, () => shown?.b);
	const editable = $derived(diff.against === 'HEAD' || diff.against === 'index');
	const versusHead = $derived(diff.against === 'HEAD' || diff.against === 'staged');
	const loaded = $derived(!!git);
	const from = $derived(git?.changes.find((change) => change.path === diff.path)?.from);
	const head = $derived(git?.head);

	onMount(() => {
		let disposed = false;
		Promise.all([language(diff.path), sides()]).then(
			([extension, sides]) => {
				if (disposed) return;
				syntax = extension;
				if (!sides) return;
				left ??= sides[0];
				right = sides[1];
				start();
			},
			(error: Error) => (note = error.message)
		);
		const unlisten = editable ? [sync.listen(device)] : [];
		if (diff.against === 'index' || diff.against === 'staged') unlisten.push(device.listen({ message }));
		return () => {
			disposed = true;
			for (const stop of unlisten) stop();
			shown?.destroy();
		};
	});

	$effect(() => {
		if (!versusHead || !loaded) return;
		const [path, commit] = [from ?? diff.path, head];
		diff.from = from;
		let stale = false;
		baseline(diff.projectId, commit, path).then(
			(content) => {
				if (stale) return;
				left = content;
				if (shown) update(content, shown.b.state.doc.toString());
				else start();
			},
			(error: Error) => {
				if (!stale && !shown) note = error.message;
			}
		);
		return () => {
			stale = true;
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
		const current = shown?.b;
		if (current) servers.displayed(current, displayed);
	});

	$effect(() => {
		if (active && (shown || note)) painted(diff.id);
	});

	$effect(() => {
		if (focused && shown && form() === 'desktop') shown.b.focus();
	});

	/** Both sides, but HEAD's, which follows the status. */
	function sides(): Promise<[string | undefined, Contents]> | undefined {
		if (diff.against === 'HEAD') return Promise.all([undefined, current()]);
		if (diff.against === 'index') return Promise.all([index(diff.projectId, diff.path), current()]);
		if (diff.against === 'staged') return Promise.all([undefined, staged()]);
		if (diff.against !== 'disk') return committed(diff.against.sha);
	}

	function start() {
		if (shown || note || left === undefined || !right) return;
		if (right.binary) {
			note = 'binary file';
			return;
		}
		const doc = build(left, right.content).b.state.doc;
		if (editable) sync.start(right.hash, doc);
	}

	async function committed(sha: string): Promise<[string, Contents]> {
		const parent = show(diff.projectId, `${sha}^`, diff.from ?? diff.path);
		const [a, b] = await Promise.all([parent, show(diff.projectId, sha, diff.path)]);
		return [a, a.includes('\0') || b.includes('\0') ? { binary: true, hash: '' } : { content: b, hash: '' }];
	}

	async function staged(): Promise<Contents> {
		const content = await index(diff.projectId, diff.path);
		return content.includes('\0') ? { binary: true, hash: '' } : { content, hash: '' };
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
		if (message.type !== 'hello' && !(message.type === 'git.changed' && message.projectId === diff.projectId)) return;
		if (document.hidden) missed = true;
		else refresh();
	}

	function catchUp() {
		if (document.hidden || !missed) return;
		missed = false;
		refresh();
	}

	/** Reads the index side again, which can change while the status stays the same. */
	async function refresh() {
		if (!shown) return;
		const generation = ++heads;
		if (diff.against === 'index') {
			const content = await index(diff.projectId, diff.path);
			if (generation === heads && shown) update(content, shown.b.state.doc.toString());
		} else {
			const side = await staged();
			if (generation === heads && shown && !side.binary) update(shown.original.toString(), side.content);
		}
	}

	function update(content: string, side: string) {
		if (!shown) return;
		if (side !== shown.b.state.doc.toString() || !content !== !shown.original.length) build(content, side);
		else if (content !== shown.original.toString()) shown.setOriginal(content);
	}

	function toggle() {
		diff.layout = diff.layout === 'split' ? 'unified' : 'split';
		if (shown) build(shown.original, shown.b.state.doc);
	}

	function build(a: string | Text, b: string | Text): Shown {
		shown?.destroy();
		const whole = !a.length !== !b.length;
		const shared = [basicSetup, syntax ?? [], theme, diffTheme(whole)];
		const working = [
			keymap.of([{ key: 'Mod-s', run: () => (sync.write(), true) }]),
			shared,
			EditorState.readOnly.of(!editable),
			sync.listener,
			editable ? languageServer(servers, diff.projectId, diff.path, false) : []
		];
		if (diff.layout === 'split') {
			const view = new MergeView({
				parent: container,
				a: { doc: a, extensions: [shared, EditorState.readOnly.of(true)] },
				b: { doc: b, extensions: working },
				revertControls: editable ? 'a-to-b' : undefined,
				highlightChanges: !whole,
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
				extensions: [working, unifiedMergeView({ original: a, mergeControls: editable && revert, highlightChanges: !whole, collapseUnchanged: {} })]
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

<svelte:document onvisibilitychange={catchUp} />

<div class="diff" role="tabpanel" hidden={!active}>
	<div class="toolbar">
		<span class="path" title={diff.path}>{diff.from ? `${diff.from} → ${diff.path}` : diff.path}</span>
		{#if diff.against === 'disk'}
			<span class="sides">disk ↔ unsaved edits</span>
		{:else if diff.against === 'index'}
			<span class="sides">index ↔ disk</span>
		{:else if diff.against === 'staged'}
			<span class="sides">HEAD ↔ index</span>
		{:else if diff.against !== 'HEAD'}
			<span class="sides" title={diff.against.subject}>{diff.against.short}^ ↔ {diff.against.short}</span>
		{/if}
		{#if diff.deleted}<span class="deleted">deleted on disk</span>{/if}
		<ServerSwitch {servers} projectId={diff.projectId} path={diff.path} attached={editable} />
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
