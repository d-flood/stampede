<script lang="ts">
	import { diffConfig, diffCounts, diffRows, diffSigns, originalNumbers } from '#lib/editor/changes.ts';
	import { language } from '#lib/editor/languages.ts';
	import { diffTheme, theme } from '#lib/editor/theme.ts';
	import { indentedWrap } from '#lib/editor/wrap.ts';
	import { read, type OpenChanges } from '#lib/files.svelte.ts';
	import { baseline, split, stage, type Change, type Git } from '#lib/git.ts';
	import Button from '#lib/ui/Button.svelte';
	import Caret from '#lib/ui/Caret.svelte';
	import FileIcon from '#lib/ui/FileIcon.svelte';
	import { getChunks, getOriginalDoc, unifiedMergeView } from '@codemirror/merge';
	import { EditorState, type Extension } from '@codemirror/state';
	import { EditorView } from '@codemirror/view';
	import { basicSetup } from 'codemirror';
	import Plus from '~icons/ph/plus';

	type Side = { text: string; binary?: undefined } | { binary: true };
	type Loaded = { change: Change; before: Side; after: Side; syntax: Extension };

	let { tab, git, active, onopen }: { tab: OpenChanges; git?: Git; active: boolean; onopen(path: string): void } = $props();

	let loaded = $state.raw<Loaded[]>([]);
	let counts = $state<Record<string, { added: number; removed: number }>>({});
	let staging = $state<string>();
	let folded = $state<Record<string, boolean>>({});
	const unstaged = $derived((git?.unstaged ?? []).map(({ path }) => path));
	const changes = $derived(git?.changes ?? []);
	const key = $derived(changes.map(({ path, kind }) => `${kind}:${path}`).join('\n'));
	const totals = $derived(Object.values(counts).reduce((sum, { added, removed }) => ({ added: sum.added + added, removed: sum.removed + removed }), { added: 0, removed: 0 }));

	async function before(change: Change): Promise<Side> {
		if (change.kind === 'added' || change.kind === 'untracked') return { text: '' };
		return { text: await baseline(tab.projectId, git?.head, change.from ?? change.path).catch(() => '') };
	}

	async function after(change: Change): Promise<Side> {
		if (change.kind === 'deleted') return { text: '' };
		const contents = await read(tab.projectId, change.path).catch(() => undefined);
		return contents?.binary ? { binary: true } : { text: contents?.content ?? '' };
	}

	$effect(() => {
		void key;
		if (!active) return;
		const listed = changes;
		let stale = false;
		Promise.all(listed.map(async (change) => ({ change, before: await before(change), after: await after(change), syntax: await language(change.path) }))).then((all) => {
			if (!stale) loaded = all;
		});
		return () => (stale = true);
	});

	function build(node: HTMLElement, { change, before, after, syntax }: Loaded) {
		if (before.binary || after.binary) return;
		const whole = !before.text.length !== !after.text.length;
		const view = new EditorView({
			parent: node,
			doc: after.text,
			extensions: [
				basicSetup,
				syntax,
				theme,
				diffTheme(whole),
				EditorState.readOnly.of(true),
				EditorView.editable.of(false),
				originalNumbers(),
				diffSigns('b'),
				diffRows('b'),
				indentedWrap,
				unifiedMergeView({ original: before.text, mergeControls: false, highlightChanges: !whole, collapseUnchanged: {}, diffConfig })
			]
		});
		counts[change.path] = diffCounts(getChunks(view.state)?.chunks ?? [], getOriginalDoc(view.state), view.state.doc);
		return () => view.destroy();
	}


	async function stageFiles(paths: string[]) {
		staging = paths.length === 1 ? paths[0] : '';
		try {
			await stage(tab.projectId, paths);
		} finally {
			staging = undefined;
		}
	}
</script>

<section class="stack" aria-label="Changes">
	<header class="summary">
		<h2>{changes.length} changed {changes.length === 1 ? 'file' : 'files'}</h2>
		{#if totals.added}<span class="added">+{totals.added}</span>{/if}
		{#if totals.removed}<span class="removed">−{totals.removed}</span>{/if}
		<span class="fill"></span>
		{#if unstaged.length}<Button variant="ghost" size="sm" disabled={staging !== undefined} onclick={() => stageFiles(unstaged)}><Plus />Stage all</Button>{/if}
	</header>
	{#each loaded as item (item.change.path)}
		{@const { dir, name } = split(item.change.path)}
		{@const count = counts[item.change.path]}
		<article class="file" aria-label={item.change.path}>
			<header>
				<button class="fold" aria-expanded={!folded[item.change.path]} aria-label="{folded[item.change.path] ? 'Expand' : 'Collapse'} {item.change.path}" onclick={() => (folded[item.change.path] = !folded[item.change.path])}><Caret /></button>
				<FileIcon path={item.change.path} />
				<button class="path" title="Open {item.change.path}" onclick={() => onopen(item.change.path)}>{dir}<span class="name">{name}</span></button>
				{#if count?.added}<span class="added">+{count.added}</span>{/if}
				{#if count?.removed}<span class="removed">−{count.removed}</span>{/if}
				<span class="fill"></span>
				{#if unstaged.includes(item.change.path)}
					<Button variant="ghost" size="sm" aria-label="Stage {item.change.path}" disabled={staging !== undefined} onclick={() => stageFiles([item.change.path])}><Plus />Stage</Button>
				{/if}
			</header>
			{#if item.before.binary || item.after.binary}
				<p class="binary" hidden={folded[item.change.path]}>Binary file</p>
			{:else if !folded[item.change.path]}
				<div class="body" {@attach (node) => build(node, item)}></div>
			{/if}
		</article>
	{:else}
		<p class="empty">No uncommitted changes</p>
	{/each}
</section>

<style>
	.stack {
		height: 100%;
		overflow-y: auto;
		padding: var(--space-4) var(--space-6) var(--space-6);
		box-sizing: border-box;
	}

	/* Last baselines line up the title with a button's label rather than its leading icon. */
	.summary {
		display: flex;
		align-items: last baseline;
		gap: var(--space-2);
		max-width: var(--measure);
		margin: 0 auto var(--space-6);
		padding-right: calc(var(--space-1) + var(--border-width));
		box-sizing: border-box;
	}

	h2 {
		margin: 0;
		font-size: var(--font-size-xl);
		font-weight: 600;
	}

	.file {
		box-sizing: border-box;
		max-width: var(--measure);
		margin: 0 auto var(--space-4);
		overflow: clip;
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-lg);
		background: var(--color-bg);
	}

	.file header {
		position: sticky;
		top: 0;
		z-index: 1;
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		padding: 0 var(--space-1) 0 var(--space-3);
		border-bottom: var(--border-width) solid var(--border);
		background: var(--chrome);
	}

	.fold {
		display: inline-grid;
		flex: none;
		place-items: center;
		width: var(--target-min);
		height: var(--target-min);
		margin-left: calc(-1 * var(--space-2));
		padding: 0;
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		cursor: pointer;
	}

	.fold:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	.path {
		min-width: 0;
		overflow: hidden;
		padding: 0;
		border: none;
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		text-align: start;
		text-overflow: ellipsis;
		white-space: nowrap;
		cursor: pointer;
	}

	.path:hover .name {
		text-decoration: underline;
	}

	.path:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-offset);
	}

	.name {
		color: var(--color-fg);
		font-weight: 500;
	}

	.added,
	.removed {
		flex: none;
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
	}

	.added {
		color: var(--git-added);
	}

	.removed {
		color: var(--git-deleted);
	}

	.fill {
		flex: 1;
	}

	.body :global(.cm-content) {
		padding-block: 0;
	}

	.body :global(.cm-foldGutter) {
		display: none !important;
	}

	.binary,
	.empty {
		margin: 0;
		padding: var(--space-3);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.empty {
		max-width: var(--measure);
		margin: 0 auto;
	}
</style>
