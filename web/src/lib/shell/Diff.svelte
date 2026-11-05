<script lang="ts">
	import type { Device, ServerMessage } from '#lib/device.svelte.ts';
	import { find } from '#lib/editor/find.ts';
	import { language } from '#lib/editor/languages.ts';
	import { languageServer } from '#lib/editor/lsp-editor.ts';
	import type { LanguageServers } from '#lib/editor/lsp.svelte.ts';
	import { Sync } from '#lib/editor/sync.svelte.ts';
	import { diffTheme, theme } from '#lib/editor/theme.ts';
	import { draftComments, draftPosition, setDrafts, setReplacing, type Render } from '#lib/editor/drafts.ts';
	import { comparison } from '#lib/compare.svelte.ts';
	import { branches, comparing, read, RequestError, type Contents, type OpenDiff } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import { baseline, index, show, type Git } from '#lib/git.ts';
	import { createDraft, deleteDraft, editDraft, hunks, setViewed, viewedState, type Hunks, type Placement, type PrReview } from '#lib/pr-reviews.ts';
	import DraftBox from '#lib/shell/DraftBox.svelte';
	import Conflict from '#lib/shell/Conflict.svelte';
	import { ordered } from '#lib/shell/tree.ts';
	import ServerSwitch from '#lib/shell/ServerSwitch.svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Toggle from '#lib/ui/Toggle.svelte';
	import { getOriginalDoc, MergeView, originalDocChangeEffect, unifiedMergeView } from '@codemirror/merge';
	import { ChangeSet, EditorState, Prec, type Extension, type Text } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import { mount, onMount, unmount, untrack } from 'svelte';
	import ArrowDown from '~icons/ph/arrow-down';
	import ArrowUp from '~icons/ph/arrow-up';
	import CheckSquare from '~icons/ph/check-square';
	import Columns from '~icons/ph/columns';
	import FloppyDisk from '~icons/ph/floppy-disk';
	import Rows from '~icons/ph/rows';

	type Props = {
		diff: OpenDiff;
		git?: Git;
		reviews?: PrReview[];
		device: Device;
		servers: LanguageServers;
		active: boolean;
		displayed: boolean;
		focused: boolean;
		onshowdiff(disk: string, buffer: string): void;
	};
	type Shown = { b: EditorView; views: EditorView[]; path: string; original: Text; editable: boolean; setOriginal(content: string): void; destroy(): void };

	let { diff, git, reviews, device, servers, active, displayed, focused, onshowdiff }: Props = $props();

	let container: HTMLDivElement;
	let shown = $state.raw<Shown>();
	let syntax = $state.raw<Extension>();
	let note = $state<string>();
	let heads = 0;
	let missed = false;
	let left: string | undefined;
	let right: Contents | undefined;
	let commentable: Hunks | undefined;
	const sync = new Sync(() => diff, () => shown?.b);
	const editable = $derived(diff.against === 'HEAD' || diff.against === 'index');
	const versusHead = $derived(diff.against === 'HEAD' || diff.against === 'staged');
	const loaded = $derived(!!git);
	const from = $derived(git?.changes.find((change) => change.path === diff.path)?.from);
	const head = $derived(git?.head);
	const compared = $derived(comparing(diff.against) ? comparison(diff.projectId, branches(diff.against)) : undefined);
	const walk = $derived(compared?.result ? ordered(compared.result.files) : []);
	const at = $derived(walk.findIndex((change) => change.path === diff.path));
	const pr = $derived(comparing(diff.against) && 'pr' in diff.against ? diff.against.pr : undefined);
	const review = $derived(pr === undefined ? undefined : reviews?.find(({ number }) => number === pr));
	const mark = $derived(review && walk[at] ? viewedState(review, walk[at]) : undefined);
	let sinceViewed = $state(false);
	let marking = $state(false);
	const since = $derived(sinceViewed && mark === 'changed' ? review?.viewed[diff.path]?.commit : undefined);
	const replacing = $derived(review?.drafts.find((draft) => draft.id === diff.replacing && draft.displaced && draft.path === diff.path));
	let replaceError = $state<string>();

	onMount(() => {
		let disposed = false;
		const following = compared;
		if (following) {
			following.load();
			const unlisten = device.listen({
				message(message) {
					const changed = message.type === 'git.changed' || message.type === 'pr-reviews.changed';
					if (message.type === 'hello' || (changed && message.projectId === diff.projectId)) following.load();
				}
			});
			return () => {
				unlisten();
				shown?.destroy();
			};
		}
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
		const result = compared?.result;
		if (!result) return;
		const [path, viewed] = [diff.path, since];
		const from = result.files.find((change) => change.path === path)?.from;
		diff.from = from;
		let stale = false;
		const left = viewed ? show(diff.projectId, viewed, path) : show(diff.projectId, result.mergeBase, from ?? path);
		const lines = pr === undefined || viewed ? undefined : hunks(diff.projectId, pr, path).catch(() => undefined);
		Promise.all([language(path), left, show(diff.projectId, result.source, path), lines]).then(
			([extension, a, b, ranges]) => {
				if (stale) return;
				syntax = extension;
				commentable = ranges;
				shown?.destroy();
				shown = undefined;
				note = undefined;
				if (a.includes('\0') || b.includes('\0')) note = 'binary file';
				else build(a, b);
			},
			(error: Error) => {
				if (!stale) note = error.message;
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
		const current = shown;
		if (!current || !review) return;
		const drafts = placed(review, current.path);
		for (const view of current.views) view.dispatch({ effects: setDrafts.of(drafts) });
	});

	$effect(() => {
		const [current, on] = [shown, !!replacing];
		if (current) for (const view of current.views) view.dispatch({ effects: setReplacing.of(on) });
	});

	$effect(() => {
		const [id, current] = [diff.draft, shown];
		if (!id || !current) return;
		for (const view of current.views) {
			const pos = draftPosition(view, id);
			if (pos === undefined) continue;
			view.dispatch({ effects: EditorView.scrollIntoView(pos, { y: 'center' }) });
			diff.draft = undefined;
		}
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
		if (diff.against !== 'disk' && !comparing(diff.against)) return committed(diff.against.sha);
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

	/** Shows the Comparison's file `by` places along its tree, stopping at either end. */
	function step(by: number) {
		const next = walk[at + by];
		if (!next) return;
		diff.path = next.path;
		diff.from = next.from;
	}

	async function viewedAndNext() {
		if (!review || marking) return;
		marking = true;
		try {
			await setViewed(diff.projectId, review.number, diff.path, true);
			step(1);
		} catch (error) {
			note = error instanceof Error ? error.message : String(error);
		} finally {
			marking = false;
		}
	}

	function stepKey(event: KeyboardEvent) {
		if (!compared || event.defaultPrevented || !event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
		if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
		event.preventDefault();
		step(event.key === 'ArrowUp' ? -1 : 1);
	}

	function toggle() {
		diff.layout = diff.layout === 'split' ? 'unified' : 'split';
		if (shown) build(shown.original, shown.b.state.doc);
	}

	/** The drafts drawn inline on `path`; a Displaced draft's lines no longer apply. */
	function placed(review: PrReview, path: string) {
		return review.drafts.filter((draft) => draft.path === path && !draft.displaced);
	}

	function applied(updated: PrReview, path: string) {
		const drafts = placed(updated, path);
		for (const view of shown?.views ?? []) view.dispatch({ effects: setDrafts.of(drafts) });
	}

	async function replace(placement: Placement) {
		const draft = replacing;
		if (!draft || pr === undefined) return;
		replaceError = undefined;
		try {
			applied(await editDraft(diff.projectId, pr, draft.id, placement), draft.path);
			diff.replacing = undefined;
			diff.draft = draft.id;
		} catch (error) {
			replaceError = error instanceof Error ? error.message : String(error);
		}
	}

	function cancelReplacing() {
		diff.replacing = undefined;
		replaceError = undefined;
	}

	function drafting(number: number, path: string): Render {
		const { projectId } = diff;
		return (target, { placement, draft, lines, close }) => {
			const label = `Draft comment on ${placement.side === 'target' ? 'deleted ' : ''}line ${placement.line}`;
			const box = mount(DraftBox, {
				target,
				props: {
					body: draft?.body ?? '',
					saved: !!draft,
					label,
					suggestion: lines,
					async onsave(body: string) {
						if (draft) return applied(await editDraft(projectId, number, draft.id, { body }), path);
						applied(await createDraft(projectId, number, { ...placement, path, body }), path);
						close();
					},
					oncancel: close,
					ondelete: async () => draft && applied(await deleteDraft(projectId, number, draft.id), path)
				}
			});
			return () => unmount(box);
		};
	}

	function build(a: string | Text, b: string | Text): Shown {
		shown?.destroy();
		const path = diff.path;
		const whole = !a.length !== !b.length;
		const drafts = review ? placed(review, path) : [];
		const commenting = (side: 'target' | 'source', unified: boolean) =>
			commentable && pr !== undefined ? draftComments(side, unified, commentable, drafts, drafting(pr, path), replace) : [];
		const walking = compared
			? Prec.highest(
					keymap.of([
						{ key: 'Alt-ArrowUp', run: () => (step(-1), true) },
						{ key: 'Alt-ArrowDown', run: () => (step(1), true) }
					])
				)
			: [];
		const shared = [walking, basicSetup, find, syntax ?? [], theme, diffTheme(whole)];
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
				a: { doc: a, extensions: [shared, EditorState.readOnly.of(true), commenting('target', false)] },
				b: { doc: b, extensions: [working, commenting('source', false)] },
				revertControls: editable ? 'a-to-b' : undefined,
				highlightChanges: !whole,
				collapseUnchanged: {}
			});
			return (shown = {
				b: view.b,
				views: [view.a, view.b],
				path,
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
				extensions: [
					working,
					commenting('source', true),
					unifiedMergeView({ original: a, mergeControls: editable && revert, highlightChanges: !whole, collapseUnchanged: {} })
				]
			});
			return (shown = {
				b: view,
				views: [view],
				path,
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

<!-- svelte-ignore a11y_interactive_supports_focus (Alt-arrows walk a Comparison from anywhere in its tab.) -->
<div class="diff" role="tabpanel" hidden={!active} onkeydown={stepKey}>
	<div class="toolbar">
		<span class="path" title={diff.path}>{diff.from ? `${diff.from} → ${diff.path}` : diff.path}</span>
		{#if diff.against === 'disk'}
			<span class="sides">disk ↔ unsaved edits</span>
		{:else if diff.against === 'index'}
			<span class="sides">index ↔ disk</span>
		{:else if diff.against === 'staged'}
			<span class="sides">HEAD ↔ index</span>
		{:else if comparing(diff.against)}
			{#if form() === 'desktop'}
				{@const { target, source } = branches(diff.against)}
				<span class="sides comparison" title="{target}...{source}">{'pr' in diff.against ? `#${diff.against.pr}` : `${target} ↔ ${source}`}</span>
			{/if}
		{:else if diff.against !== 'HEAD'}
			<span class="sides" title={diff.against.subject}>{diff.against.short}^ ↔ {diff.against.short}</span>
		{/if}
		{#if diff.deleted}<span class="deleted">deleted on disk</span>{/if}
		{#if compared?.error}<span class="deleted">{compared.error.message}</span>{/if}
		{#if compared}
			<IconButton label="Previous file" title="Previous file (Alt+↑)" disabled={at <= 0} onclick={() => step(-1)}><ArrowUp /></IconButton>
			<IconButton label="Next file" title="Next file (Alt+↓)" disabled={at >= walk.length - 1} onclick={() => step(1)}><ArrowDown /></IconButton>
		{/if}
		{#if review}
			{#if mark === 'changed'}
				<Toggle label="Since viewed" pressed={sinceViewed} onclick={() => (sinceViewed = !sinceViewed)}>Since viewed</Toggle>
			{/if}
			{#if form() === 'phone'}
				<IconButton label="Viewed & next" title="Mark Viewed, then the next file" disabled={marking || at < 0} onclick={viewedAndNext}><CheckSquare /></IconButton>
			{:else}
				<Button title="Mark Viewed, then the next file" disabled={marking || at < 0} onclick={viewedAndNext}><CheckSquare /> Viewed &amp; next</Button>
			{/if}
		{/if}
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
	{#if replacing}
		<div class="replacing" role="status">
			<span class="prompt">Choose a line to re-place <q>{replacing.body.trim().split('\n')[0]}</q></span>
			{#if replaceError}<span class="deleted">{replaceError}</span>{/if}
			<Button onclick={cancelReplacing}>Cancel</Button>
		</div>
	{/if}
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

	.comparison {
		flex: 0 1 auto;
		max-width: 40%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.deleted {
		flex: none;
		color: var(--status-waiting);
		font-size: var(--font-size-sm);
	}

	.replacing {
		display: flex;
		flex: none;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-1) var(--space-1) var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
		border-left: 3px solid var(--accent);
		background: var(--surface-1);
		font-size: var(--font-size-sm);
	}

	.prompt {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
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
