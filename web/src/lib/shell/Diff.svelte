<script lang="ts">
	import { projectUrl } from '#lib/host.ts';
	import type { Device, ServerMessage } from '#lib/device.svelte.ts';
	import { diffConfig, diffCounts, diffRows, diffSigns, originalNumbers } from '#lib/editor/changes.ts';
	import { find } from '#lib/editor/find.ts';
	import { language } from '#lib/editor/languages.ts';
	import { languageServer } from '#lib/editor/lsp-editor.ts';
	import type { LanguageServers } from '#lib/editor/lsp.svelte.ts';
	import { Sync } from '#lib/editor/sync.svelte.ts';
	import { diffTheme, theme } from '#lib/editor/theme.ts';
	import { indentedWrap } from '#lib/editor/wrap.ts';
	import { draftComments, draftPosition, openPlacement, openPosition, setDrafts, setReplacing, type Render } from '#lib/editor/drafts.ts';
	import { comparison } from '#lib/compare.svelte.ts';
	import { branches, comparing, image, read, RequestError, type Contents, type OpenDiff } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { painted } from '#lib/paint.svelte.ts';
	import { baseline, index, indexUrl, show, showUrl, stage, unstage, type Git } from '#lib/git.ts';
	import { acceptDraft, createDraft, deleteDraft, editDraft, hunks, setViewed, viewedState, type Hunks, type Placement, type PrReview, type Side } from '#lib/pr-reviews.ts';
	import DraftBox from '#lib/shell/DraftBox.svelte';
	import ImageDiff, { type ImageSide } from '#lib/shell/ImageDiff.svelte';
	import Conflict from '#lib/shell/Conflict.svelte';
	import { ordered } from '#lib/shell/tree.ts';
	import ServerSwitch from '#lib/shell/ServerSwitch.svelte';
	import Button from '#lib/ui/Button.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Segmented from '#lib/ui/Segmented.svelte';
	import Toolbar from '#lib/ui/Toolbar.svelte';
	import { getChunks, getOriginalDoc, MergeView, originalDocChangeEffect, unifiedMergeView } from '@codemirror/merge';
	import { setDiagnostics } from '@codemirror/lint';
	import { ChangeSet, Compartment, EditorState, Prec, type Extension, type Text } from '@codemirror/state';
	import { EditorView, keymap } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import { mount, onMount, unmount, untrack } from 'svelte';
	import ArrowDown from '~icons/ph/arrow-down';
	import ArrowUp from '~icons/ph/arrow-up';
	import CheckSquare from '~icons/ph/check-square';
	import Columns from '~icons/ph/columns';
	import FloppyDisk from '~icons/ph/floppy-disk';
	import Minus from '~icons/ph/minus';
	import Plus from '~icons/ph/plus';
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
	/** Unsaved text by draft ID, or for a new comment by the side of the view holding it. */
	type Typed = Record<string, string>;
	type Shown = { b: EditorView; views: EditorView[]; path: string; original: Text; editable: boolean; analyzed: boolean; typed: Typed; setOriginal(content: string): void; destroy(): void };

	let { diff, git, reviews, device, servers, active, displayed, focused, onshowdiff }: Props = $props();

	let container: HTMLDivElement;
	let moving = $state(false);
	let width = $state(0);
	let columns = $state(0);
	const narrow = $derived(width > 0 && width < 720);
	const layout = $derived(narrow ? 'unified' : diff.layout);
	const wrapped = $derived(layout === 'unified' ? narrow : width > 0 && width < 2 * columns);
	let shown = $state.raw<Shown>();
	let revision = $state(0);
	const counts = $derived.by(() => {
		revision;
		const chunks = shown && getChunks(shown.b.state)?.chunks;
		if (!shown || !chunks) return;
		return diffCounts(chunks, shown.original, shown.b.state.doc);
	});
	const recount = EditorView.updateListener.of((update) => {
		// Dispatches often run inside effects, so a tracked read here would make them depend on what they write.
		if (update.docChanged || update.transactions.some((transaction) => transaction.effects.length)) untrack(() => revision++);
	});
	let syntax = $state.raw<Extension>();
	let note = $state<string>();
	let heads = 0;
	let missed = false;
	let left: string | undefined;
	let right: Contents | undefined;
	let commentable: Hunks | undefined;
	const sync = new Sync(() => diff, () => shown?.b);
	const analysis = new Compartment();
	const wrapping = new Compartment();
	const editable = $derived(diff.against === 'HEAD' || diff.against === 'index');
	const versusHead = $derived(diff.against === 'HEAD' || diff.against === 'staged');
	const loaded = $derived(!!git);
	const from = $derived(git?.changes.find((change) => change.path === diff.path)?.from);
	const head = $derived(git?.head);
	const compared = $derived(comparing(diff.against) ? comparison(diff.projectId, branches(diff.against)) : undefined);
	const analyzed = $derived(editable || (!!head && head === compared?.result?.source));
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
	const svg = $derived(/\.svg$/i.test(diff.path));
	const raster = $derived(image(diff.path) && !svg);
	let rendered = $state(true);
	const pictures = $derived.by((): ImageSide[] | undefined => {
		if (!raster && !(svg && rendered)) return;
		const { projectId, path, against } = diff;
		const disk: ImageSide = { label: 'disk', url: projectUrl(projectId, `image?${new URLSearchParams({ path })}`), live: 'disk' };
		const staged: ImageSide = { label: 'index', url: indexUrl(projectId, path), live: 'index' };
		const committed: ImageSide = { label: 'HEAD', url: head && showUrl(projectId, head, from ?? path) };
		if (against === 'HEAD') return loaded ? [committed, disk] : undefined;
		if (against === 'index') return [staged, disk];
		if (against === 'staged') return loaded ? [committed, staged] : undefined;
		if (against === 'disk') return;
		if (!comparing(against)) return [{ label: `${against.short}^`, url: showUrl(projectId, `${against.sha}^`, diff.from ?? path) }, { label: against.short, url: showUrl(projectId, against.sha, path) }];
		const result = compared?.result;
		if (!result) return;
		const renamed = result.files.find((change) => change.path === path)?.from;
		const { target, source } = 'pr' in against ? { target: 'target', source: 'source' } : against;
		return [
			since ? { label: 'viewed', url: showUrl(projectId, since, path) } : { label: target, url: showUrl(projectId, result.mergeBase, renamed ?? path) },
			{ label: source, url: showUrl(projectId, result.source, path) }
		];
	});

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
		if (raster) return;
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
		if (raster) return;
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
		if (raster) {
			untrack(() => shown?.destroy());
			shown = undefined;
			note = undefined;
			return;
		}
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
		const now = analyzed;
		untrack(() => {
			if (!shown || shown.analyzed === now) return;
			shown.analyzed = now;
			shown.b.dispatch({ effects: analysis.reconfigure(now ? languageServer(servers, diff.projectId, diff.path, false) : []) });
			if (now) servers.displayed(shown.b, displayed);
			else shown.b.dispatch(setDiagnostics(shown.b.state, []));
		});
	});

	$effect(() => {
		const current = shown?.b;
		if (current) servers.displayed(current, displayed && !pictures);
	});

	$effect(() => {
		const wrap = wrapped;
		untrack(() => {
			for (const view of shown?.views ?? []) view.dispatch({ effects: wrapping.reconfigure(wrap ? indentedWrap : []) });
		});
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
		if (focused && shown && !pictures && form() === 'desktop') shown.b.focus();
	});

	/** Both sides, but HEAD's, which follows the status. */
	async function move(staging: boolean) {
		moving = true;
		try {
			await (staging ? stage : unstage)(diff.projectId, [diff.path]);
		} catch (error) {
			console.error(error);
		} finally {
			moving = false;
		}
	}

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

	/** One column holds one new comment, so of a split view's two the one with text stays, or else the Source's. */
	function reopened({ typed, views }: Shown, unified: boolean) {
		const open = views.map(openPlacement);
		if (!unified) {
			if (open[0]?.side === 'target') {
				typed.target = typed.source;
				delete typed.source;
			}
			return open[0];
		}
		const [target, source] = open;
		if (source && (typed.source || !typed.target)) {
			delete typed.target;
			return source;
		}
		if (target) typed.source = typed.target;
		delete typed.target;
		return target;
	}

	function toggle() {
		diff.layout = diff.layout === 'split' ? 'unified' : 'split';
	}

	$effect(() => {
		const unified = layout === 'unified';
		untrack(() => {
			if (!shown || (shown.views.length === 1) === unified) return;
			const pending = reopened(shown, unified);
			const views = build(shown.original, shown.b.state.doc, { typed: shown.typed, pending }).views;
			for (const view of views) {
				const pos = openPosition(view);
				if (pos !== undefined) view.dispatch({ effects: EditorView.scrollIntoView(pos, { y: 'center' }) });
			}
		});
	});

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

	function drafting(number: number, path: string, side: Side, typed: Typed): Render {
		const { projectId } = diff;
		return (target, { placement, draft, lines, fresh, close }) => {
			const key = draft?.id ?? side;
			const body = draft?.body ?? '';
			const where = `${placement.side === 'target' ? 'deleted ' : ''}${placement.startLine ? `lines ${placement.startLine}–${placement.line}` : `line ${placement.line}`}`;
			const named = where[0].toUpperCase() + where.slice(1);
			const hint = `${matchMedia('(pointer: coarse)').matches ? 'Tap' : 'Shift-click'} another line to extend`;
			const box = mount(DraftBox, {
				target,
				props: {
					get text() {
						return typed[key] ?? body;
					},
					set text(value) {
						if (value === body) delete typed[key];
						else typed[key] = value;
					},
					body,
					saved: !!draft,
					label: `Draft comment on ${where}`,
					caption: draft ? [draft.suggested && `Suggested by ${draft.suggested.rig}`, placement.startLine && named].filter(Boolean).join(' · ') || undefined : `${named} · ${hint}`,
					autofocus: fresh && !draft,
					suggestion: lines,
					async onsave(text: string) {
						const updated = await (draft ? editDraft(projectId, number, draft.id, { body: text }) : createDraft(projectId, number, { ...placement, path, body: text }));
						delete typed[key];
						applied(updated, path);
						if (!draft) close();
					},
					oncancel() {
						delete typed[key];
						close();
					},
					ondelete: async () => draft && applied(await deleteDraft(projectId, number, draft.id), path),
					onaccept: draft?.suggested && (async () => applied(await acceptDraft(projectId, number, draft.id), path))
				}
			});
			return () => unmount(box);
		};
	}

	function build(a: string | Text, b: string | Text, kept?: { typed: Typed; pending: Placement | null }): Shown {
		shown?.destroy();
		const path = diff.path;
		const typed = $state<Typed>(kept?.typed ?? {});
		const pending = kept?.pending ?? null;
		const whole = !a.length !== !b.length;
		const drafts = review ? placed(review, path) : [];
		const commenting = (side: Side, unified: boolean) =>
			commentable && pr !== undefined ? draftComments(side, unified, commentable, drafts, drafting(pr, path, side, typed), replace, unified || pending?.side === side ? pending : null) : [];
		const walking = compared
			? Prec.highest(
					keymap.of([
						{ key: 'Alt-ArrowUp', run: () => (step(-1), true) },
						{ key: 'Alt-ArrowDown', run: () => (step(1), true) }
					])
				)
			: [];
		const shared = [walking, basicSetup, find, syntax ?? [], theme, diffTheme(whole), recount];
		const working = [
			keymap.of([{ key: 'Mod-s', run: () => (sync.write(), true) }]),
			shared,
			EditorState.readOnly.of(!editable),
			sync.listener,
			analysis.of(analyzed ? languageServer(servers, diff.projectId, diff.path, false) : [])
		];
		if (layout === 'split') {
			const view = new MergeView({
				parent: container,
				a: { doc: a, extensions: [shared, diffSigns('a'), diffRows('a'), EditorState.readOnly.of(true), commenting('target', false), wrapping.of(wrapped ? indentedWrap : [])] },
				b: { doc: b, extensions: [working, diffSigns('b'), diffRows('b'), commenting('source', false), wrapping.of(wrapped ? indentedWrap : [])] },
				revertControls: editable ? 'a-to-b' : undefined,
				highlightChanges: !whole,
				collapseUnchanged: {},
				diffConfig
			});
			return (shown = {
				b: view.b,
				views: [view.a, view.b],
				path,
				editable,
				analyzed,
				typed,
				get original() {
					return view.a.state.doc;
				},
				setOriginal(content) {
					view.a.dispatch({ changes: { from: 0, to: view.a.state.doc.length, insert: content } });
					// Folds are only computed when configured, so without this the two sides keep disagreeing ones.
					view.reconfigure({ collapseUnchanged: {} });
				},
				destroy: () => view.destroy()
			});
		} else {
			const view = new EditorView({
				parent: container,
				doc: b,
				extensions: [
					working,
					originalNumbers(),
					diffSigns('b'),
					diffRows('b'),
					commenting('source', true),
					wrapping.of(wrapped ? indentedWrap : []),
					unifiedMergeView({ original: a, mergeControls: editable && revert, highlightChanges: !whole, collapseUnchanged: {}, diffConfig })
				]
			});
			return (shown = {
				b: view,
				views: [view],
				path,
				editable,
				analyzed,
				typed,
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
	<Toolbar path={diff.from ? `${diff.from} → ${diff.path}` : diff.path} title={diff.path} deleted={diff.deleted} wrap={svg}>
		{#if diff.against === 'disk'}
			<span class="sides">Unsaved edits</span>
		{:else if diff.against === 'index'}
			<span class="sides">Unstaged</span>
		{:else if diff.against === 'staged'}
			<span class="sides">Staged</span>
		{:else if comparing(diff.against)}
			{#if form() === 'desktop'}
				{@const { target, source } = branches(diff.against)}
				<span class="sides comparison" title="{target}...{source}">{'pr' in diff.against ? `#${diff.against.pr}` : `${target} ↔ ${source}`}</span>
			{/if}
		{:else if diff.against !== 'HEAD'}
			<span class="sides" title={diff.against.subject}>{diff.against.short}^ ↔ {diff.against.short}</span>
		{/if}
		{#if counts && (counts.added || counts.removed)}<span class="counts" aria-label="{counts.added} added, {counts.removed} removed">{#if counts.added}<span class="added">+{counts.added}</span>{/if}{#if counts.removed}<span class="removed">−{counts.removed}</span>{/if}</span>{/if}
		{#if diff.against === 'index' || diff.against === 'staged'}
			{@const unstaged = diff.against === 'index'}
			{@const label = unstaged ? 'Stage' : 'Unstage'}
			{@const Icon = unstaged ? Plus : Minus}
			<Button variant="ghost" size="sm" aria-label="{label} file" disabled={moving || !counts?.added && !counts?.removed} onclick={() => move(unstaged)}><Icon />{#if form() === 'desktop'}{label}{/if}</Button>
		{/if}
		{#if compared?.error}<span class="error">{compared.error.message}</span>{/if}
		{#if compared}
			<IconButton label="Previous file" title="Previous file (Alt+↑)" disabled={at <= 0} onclick={() => step(-1)}><ArrowUp /></IconButton>
			<IconButton label="Next file" title="Next file (Alt+↓)" disabled={at >= walk.length - 1} onclick={() => step(1)}><ArrowDown /></IconButton>
		{/if}
		{#if review}
			{#if mark === 'changed'}
				<IconButton label="Since viewed" title="Since viewed" pressed={sinceViewed} onclick={() => (sinceViewed = !sinceViewed)}>Since viewed</IconButton>
			{/if}
			{#if form() === 'phone'}
				<IconButton label="Viewed & next" title="Mark Viewed, then the next file" disabled={marking || at < 0} onclick={viewedAndNext}><CheckSquare /></IconButton>
			{:else}
				<Button title="Mark Viewed, then the next file" disabled={marking || at < 0} onclick={viewedAndNext}><CheckSquare /> Viewed &amp; next</Button>
			{/if}
		{/if}
		{#if svg}
			<Segmented aria-label="View" options={[{ value: true, label: 'Rendered' }, { value: false, label: 'Source' }]} bind:value={rendered} />
		{/if}
		{#if analyzed && !pictures}<ServerSwitch {servers} projectId={diff.projectId} path={diff.path} />{/if}
		{#if shown || pictures}
			{#if layout === 'split'}
				<IconButton label="Single column" title="Single column" onclick={toggle}><Rows /></IconButton>
			{:else if !narrow}
				<IconButton label="Side by side" title="Side by side" onclick={toggle}><Columns /></IconButton>
			{/if}
			{#if shown && !pictures && form() === 'phone' && editable}
				<Button onclick={() => sync.write()} disabled={!diff.dirty}><FloppyDisk /> Save</Button>
			{/if}
		{/if}
	</Toolbar>
	{#if replacing}
		<div class="replacing" role="status">
			<span class="prompt">Choose a line to re-place <q>{replacing.body.trim().split('\n')[0]}</q></span>
			{#if replaceError}<span class="deleted">{replaceError}</span>{/if}
			<Button onclick={cancelReplacing}>Cancel</Button>
		</div>
	{/if}
	<Conflict {sync} onshowdiff={(disk) => shown && onshowdiff(disk, shown.b.state.doc.toString())} />
	<div class="content" class:wrapped bind:this={container} bind:clientWidth={width} hidden={!!pictures}>
		<span class="probe" aria-hidden="true"><span bind:clientWidth={columns}></span></span>
		{#if note}<p>{note}</p>{/if}
	</div>
	{#if pictures}
		<div class="content"><ImageDiff {diff} sides={pictures} {device} {active} /></div>
	{/if}
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



	.sides {
		flex: none;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	/* Hunk actions wait for the pointer so they don't sit over code while reading. */
	@media (hover: hover) {
		.content :global(.cm-editor:not(:hover) .cm-chunkButtons:not(:focus-within)),
		.content :global(.cm-mergeView:not(:hover) .cm-merge-revert:not(:focus-within)) {
			opacity: 0;
		}
	}

	/* Lengthens the editor's fade, an overlay rather than a mask so Draft comments can sit above it. */
	.content:not(.wrapped) > :global(.cm-editor > .cm-scroller::after),
	.content:not(.wrapped) :global(.cm-mergeViewEditor > .cm-editor > .cm-scroller::after) {
		width: calc(3 * var(--space-6));
		margin-left: calc(-3 * var(--space-6));
	}

	/* Measures a hundred columns of code, past which a side by side view stops wrapping. */
	.probe {
		position: absolute;
		width: 0;
		height: 0;
		overflow: hidden;
	}

	.probe > span {
		display: block;
		width: 100ch;
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
	}

	.counts {
		display: inline-flex;
		flex: none;
		gap: var(--space-1);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
	}

	.added {
		color: var(--git-added);
	}

	.removed {
		color: var(--git-deleted);
	}

	.comparison {
		flex: 0 1 auto;
		max-width: 40%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.error {
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

	.content :global(.cm-mergeViewEditors) {
		min-height: 100%;
	}

	.content :global(.cm-mergeViewEditor + .cm-mergeViewEditor) {
		border-left: var(--border-width) solid var(--border);
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
