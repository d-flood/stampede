<script lang="ts" module>
	const messages = $state<Record<string, string>>({});

	/** Offers a commit message back to the commit box, unless one is already being written. */
	export function restoreMessage(projectId: string, message: string) {
		if (!messages[projectId]?.trim()) messages[projectId] = message;
	}
</script>

<script lang="ts">
	import { commitRig } from '#lib/config.svelte.ts';
	import type { Device } from '#lib/device.svelte.ts';
	import {
		commit,
		discard,
		draft,
		letters,
		split,
		stage,
		stash,
		stashes,
		sync,
		undo,
		unstage,
		unstash,
		type Change,
		type Git,
		type Stash
	} from '#lib/git.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import { onMount, tick } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import Archive from '~icons/ph/archive';
	import ArrowCounterClockwise from '~icons/ph/arrow-counter-clockwise';
	import ArrowDown from '~icons/ph/arrow-down';
	import ArrowUUpLeft from '~icons/ph/arrow-u-up-left';
	import ArrowUp from '~icons/ph/arrow-up';
	import ArrowsClockwise from '~icons/ph/arrows-clockwise';
	import CaretRight from '~icons/ph/caret-right';
	import Check from '~icons/ph/check';
	import CheckCircle from '~icons/ph/check-circle';
	import CloudArrowUp from '~icons/ph/cloud-arrow-up';
	import Copy from '~icons/ph/copy';
	import DotsThree from '~icons/ph/dots-three';
	import Minus from '~icons/ph/minus';
	import Plus from '~icons/ph/plus';
	import Sparkle from '~icons/ph/sparkle';
	import Trash from '~icons/ph/trash';
	import TrayArrowUp from '~icons/ph/tray-arrow-up';

	type Section = 'conflicted' | 'staged' | 'unstaged';
	type Folder = { name: string; path: string; folders: Folder[]; files: Change[] };
	type Row =
		| { kind: 'folder'; key: string; folder: Folder; depth: number; changes: Change[]; open: boolean }
		| { kind: 'file'; key: string; change: Change; depth: number };
	type Target = { section: Section; changes: Change[]; label: string; change?: Change };
	type Props = {
		projectId: string;
		git: Git;
		device: Device;
		onopen(path: string): void;
		ondiff(change: Change, against: 'index' | 'staged'): void;
	};

	let { projectId, git, device, onopen, ondiff }: Props = $props();

	const LARGE = 200;
	const collapsed = new SvelteSet<string>();
	const opened = new SvelteSet<string>();
	let busy = $state(false);
	let failure = $state<string>();
	let drafting = $state(false);
	let saved = $state.raw<Stash[]>([]);
	let discarding = $state<{ label: string; paths: string[]; untracked: number }>();
	let dropping = $state<number>();
	let list = $state<HTMLDivElement>();
	let box = $state<HTMLTextAreaElement>();
	let cursor: number | undefined;
	let loads = 0;

	const groups = $derived(
		(
			[
				{ section: 'conflicted', label: 'Merge Changes', changes: git.conflicted },
				{ section: 'staged', label: 'Staged Changes', changes: git.staged },
				{ section: 'unstaged', label: 'Changes', changes: git.unstaged }
			] as const
		)
			.filter(({ changes }) => changes.length)
			.map((group) => ({ ...group, rows: flatten(tree(group.changes), group.section, 0) }))
	);
	const targets = $derived(
		new Map<string, Target>(
			groups.flatMap(({ section, label, changes, rows }) => [
				[section, { section, changes, label: label === 'Changes' ? 'All Changes' : `All ${label}` }],
				...rows.map((row): [string, Target] =>
					row.kind === 'file'
						? [row.key, { section, changes: [row.change], label: row.change.path, change: row.change }]
						: [row.key, { section, changes: row.changes, label: `${row.folder.path}/` }]
				)
			])
		)
	);
	const message = $derived(messages[projectId] ?? '');
	const committable = $derived(
		!busy && !!message.trim() && !git.conflicted.length && git.operation !== 'rebase' && (!!git.staged.length || !!git.operation)
	);
	const publishable = $derived(!git.upstream && !!git.origin && !!git.branch && !git.detached);
	const operation = $derived.by(() => {
		if (!git.operation) return undefined;
		if (git.operation === 'rebase') return 'A rebase is in progress. Continue or abort it in a terminal with git rebase --continue or --abort.';
		const steps = git.conflicted.length ? 'Resolve each conflict in its file, stage it, then commit' : 'Commit';
		return `A ${git.operation} is in progress. ${steps} to finish it, or run git ${git.operation} --abort in a terminal.`;
	});

	onMount(() => {
		load();
		return device.listen({
			message(message) {
				if (message.type === 'hello' || (message.type === 'git.changed' && message.projectId === projectId)) load();
			}
		});
	});

	$effect(() => {
		void groups;
		if (cursor === undefined || !list || (document.activeElement && document.activeElement !== document.body)) return;
		const rows = [...list.querySelectorAll<HTMLElement>('[data-key]')];
		rows[Math.min(cursor, rows.length - 1)]?.focus();
		cursor = undefined;
	});

	async function load() {
		const generation = ++loads;
		const listed = await stashes(projectId).catch(() => undefined);
		if (listed && generation === loads) saved = listed;
	}

	function tree(changes: Change[]): Folder {
		const root: Folder = { name: '', path: '', folders: [], files: [] };
		for (const change of changes) {
			let folder = root;
			for (const name of change.path.split('/').slice(0, -1)) {
				const path = folder.path ? `${folder.path}/${name}` : name;
				let child = folder.folders.find((candidate) => candidate.path === path);
				if (!child) folder.folders.push((child = { name, path, folders: [], files: [] }));
				folder = child;
			}
			folder.files.push(change);
		}
		return root;
	}

	function compact(folder: Folder): Folder {
		while (!folder.files.length && folder.folders.length === 1) {
			const [only] = folder.folders;
			folder = { ...only, name: `${folder.name}/${only.name}` };
		}
		return folder;
	}

	function under(folder: Folder): Change[] {
		return [...folder.files, ...folder.folders.flatMap(under)];
	}

	function flatten(folder: Folder, section: Section, depth: number): Row[] {
		const folders = folder.folders.map(compact).sort((a, b) => a.name.localeCompare(b.name));
		const files = folder.files.toSorted((a, b) => split(a.path).name.localeCompare(split(b.path).name));
		return [
			...folders.flatMap((child): Row[] => {
				const key = `${section}:${child.path}/`;
				const changes = under(child);
				const open = changes.length > LARGE ? opened.has(key) : !collapsed.has(key);
				const row: Row = { kind: 'folder', key, folder: child, depth, changes, open };
				return open ? [row, ...flatten(child, section, depth + 1)] : [row];
			}),
			...files.map((change): Row => ({ kind: 'file', key: `${section}:${change.path}`, change, depth }))
		];
	}

	function toggle(key: string, toggled = collapsed) {
		if (!toggled.delete(key)) toggled.add(key);
	}

	async function run(action: () => Promise<unknown>) {
		if (busy) return;
		busy = true;
		failure = undefined;
		try {
			await action();
		} catch (error) {
			cursor = undefined;
			failure = error instanceof Error ? error.message : String(error);
		} finally {
			busy = false;
		}
	}

	function paths(changes: Change[]) {
		return changes.flatMap(({ path, from }) => (from ? [path, from] : [path]));
	}

	function toggleStage({ section, changes }: Target) {
		run(() => (section === 'staged' ? unstage(projectId, paths(changes)) : stage(projectId, paths(changes))));
	}

	function stageAll() {
		if (git.unstaged.length) run(() => stage(projectId, paths(git.unstaged)));
		else if (git.staged.length) run(() => unstage(projectId, paths(git.staged)));
	}

	function askDiscard({ section, changes, label }: Target) {
		if (section !== 'unstaged' || busy) return;
		discarding = { label, paths: changes.map(({ path }) => path), untracked: changes.filter(({ kind }) => kind === 'untracked').length };
	}

	function confirmDiscard() {
		const chosen = discarding;
		discarding = undefined;
		if (chosen) run(() => discard(projectId, chosen.paths));
	}

	function openRow({ section, change }: Target) {
		if (!change) return;
		if (section === 'conflicted') onopen(change.path);
		else ondiff(change, section === 'staged' ? 'staged' : 'index');
	}

	function commitStaged() {
		if (!committable) return;
		const id = projectId;
		run(async () => {
			await commit(id, message);
			messages[id] = '';
		});
	}

	async function draftMessage() {
		const id = projectId;
		drafting = true;
		failure = undefined;
		try {
			messages[id] = await draft(id);
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		} finally {
			drafting = false;
		}
	}

	function undoCommit() {
		const id = projectId;
		run(async () => restoreMessage(id, await undo(id)));
	}

	function remote(action: 'fetch' | 'pull' | 'push' | 'publish') {
		run(() => sync(projectId, action));
	}

	function push() {
		if (publishable) remote('publish');
		else if (git.upstream) remote('push');
	}

	function confirmDrop() {
		const index = dropping;
		dropping = undefined;
		if (index !== undefined) run(() => unstash(projectId, index, 'drop'));
	}

	function move(from: HTMLElement | null, step: number) {
		const rows = [...(list?.querySelectorAll<HTMLElement>('[data-key]') ?? [])];
		const at = from ? rows.indexOf(from) : -1;
		rows[Math.max(0, Math.min(rows.length - 1, at + step))]?.focus();
	}

	function key(event: KeyboardEvent) {
		if (event.defaultPrevented || event.isComposing || event.ctrlKey || event.metaKey || event.altKey) return;
		const row = event.target instanceof HTMLElement ? event.target.closest<HTMLElement>('[data-key]') : null;
		const target = row ? targets.get(row.dataset.key!) : undefined;
		const rowActions: Record<string, () => void> = target
			? {
					j: () => move(row, 1),
					ArrowDown: () => move(row, 1),
					k: () => move(row, -1),
					ArrowUp: () => move(row, -1),
					' ': () => toggleStage(target),
					d: () => askDiscard(target),
					e: () => target.change && target.change.kind !== 'deleted' && onopen(target.change.path)
				}
			: {};
		const actions: Record<string, () => void> = {
			...rowActions,
			a: stageAll,
			c: () => box?.focus(),
			s: () => run(() => stash(projectId)),
			z: undoCommit,
			f: () => remote('fetch'),
			p: () => git.upstream && remote('pull'),
			P: push
		};
		const action = actions[event.key];
		if (!action) return;
		event.preventDefault();
		if (row && [' ', 'a', 'd'].includes(event.key)) cursor = [...list!.querySelectorAll('[data-key]')].indexOf(row);
		action();
	}

	function commitKey(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			commitStaged();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			tick().then(() => move(null, 0));
		}
	}
</script>

{#snippet controls(target: Target | undefined)}
	{#if target}
		<span class="actions">
			{#if target.section === 'unstaged'}
				<IconButton label={`Discard ${target.label}`} title="Discard (d)" disabled={busy} onclick={() => askDiscard(target)}><ArrowCounterClockwise /></IconButton>
			{/if}
			{#if target.section === 'staged'}
				<IconButton label={`Unstage ${target.label}`} title="Unstage (space)" disabled={busy} onclick={() => toggleStage(target)}><Minus /></IconButton>
			{:else}
				<IconButton label={`Stage ${target.label}`} title="Stage (space)" disabled={busy} onclick={() => toggleStage(target)}><Plus /></IconButton>
			{/if}
		</span>
	{/if}
{/snippet}

<section aria-label="Changes">
	<header>
		{#if git.upstream}
			<span class="upstream" title="Upstream branch">{git.upstream}</span>
			<IconButton label="Fetch" title="Fetch (f)" disabled={busy} onclick={() => remote('fetch')}><ArrowsClockwise /></IconButton>
			<button class="sync" aria-label="Pull" title={`Pull ${git.behind} from ${git.upstream} (p)`} disabled={busy} onclick={() => remote('pull')}>
				<ArrowDown />{git.behind}
			</button>
			<button class="sync" aria-label="Push" title={`Push ${git.ahead} to ${git.upstream} (P)`} disabled={busy || !git.ahead} onclick={push}>
				<ArrowUp />{git.ahead}
			</button>
		{:else if publishable}
			<span class="upstream">Not on origin</span>
			<Button title="Publish Branch (P)" disabled={busy} onclick={push}><CloudArrowUp /> Publish Branch</Button>
		{:else}
			<span class="upstream">{git.detached ? 'Detached HEAD' : 'Local branch'}</span>
		{/if}
		<Menu
			items={[
				{ label: 'Stash All Changes', icon: Archive, hint: 's', disabled: busy || !git.changes.length, onselect: () => run(() => stash(projectId)) },
				{ label: 'Undo Last Commit', icon: ArrowUUpLeft, hint: 'z', disabled: busy, onselect: undoCommit }
			]}
		>
			{#snippet trigger(props)}
				<IconButton {...props} label="More Git actions"><DotsThree /></IconButton>
			{/snippet}
		</Menu>
	</header>
	{#if git.changes.length || git.operation || message || failure}
		<div class="commit">
			<textarea
				bind:this={box}
				bind:value={() => message, (value) => (messages[projectId] = value)}
				aria-label="Commit message"
				placeholder="Commit message"
				rows="3"
				spellcheck="true"
				onkeydown={commitKey}
			></textarea>
			<div class="buttons">
				<Button title={`Draft a message with ${commitRig() ?? 'the commit Rig'}`} disabled={drafting || !git.staged.length} onclick={draftMessage}>
					<Sparkle />{drafting ? 'Drafting…' : 'Draft'}
				</Button>
				<Button variant="primary" title="Commit (Ctrl+Enter)" disabled={!committable} onclick={commitStaged}>
					<Check />{git.operation === 'merge' ? 'Commit Merge' : 'Commit'}
				</Button>
			</div>
			{#if failure}<pre class="failure" role="alert">{failure}</pre>{/if}
		</div>
	{/if}
	{#if operation}<p class="operation" role="status">{operation}</p>{/if}
	<!-- svelte-ignore a11y_no_static_element_interactions (Keys act on the focused row.) -->
	<div class="list" bind:this={list} onkeydown={key} aria-busy={busy}>
		{#each groups as { section, label, changes, rows } (section)}
			{@const open = !collapsed.has(section)}
			<div class="row heading">
				<button data-key={section} class="entry" aria-expanded={open} onclick={() => toggle(section)}>
					<span class="caret" class:open><CaretRight /></span>
					<span class="name">{label}</span>
					<span class="count">{changes.length}</span>
				</button>
				{@render controls(targets.get(section))}
			</div>
			{#if open}
				<ul aria-label={label}>
					{#each rows as row (row.key)}
						<li class="row">
							{#if row.kind === 'folder'}
								<button data-key={row.key} class="entry" style:--depth={row.depth} aria-expanded={row.open} title={row.folder.path} onclick={() => toggle(row.key, row.changes.length > LARGE ? opened : collapsed)}>
									<span class="caret" class:open={row.open}><CaretRight /></span>
									<span class="name">{row.folder.name}</span>
								</button>
							{:else}
								{@const { change } = row}
								<button
									data-key={row.key}
									class="entry file"
									class:deleted={change.kind === 'deleted'}
									style:--depth={row.depth}
									style:--git="var(--git-{change.kind})"
									title={change.from ? `${change.from} → ${change.path}` : change.path}
									onclick={() => openRow(targets.get(row.key)!)}
								>
									<span class="name">{split(change.path).name}</span>
									<span class="letter" aria-hidden="true">{letters[change.kind]}</span>
								</button>
							{/if}
							{@render controls(targets.get(row.key))}
						</li>
					{/each}
				</ul>
			{/if}
		{:else}
			<div class="clean">
				<span class="mark"><CheckCircle /></span>
				<p>No uncommitted changes</p>
				<small>The working tree matches {git.branch ?? 'HEAD'}.</small>
			</div>
		{/each}
		{#if saved.length}
			{@const open = !collapsed.has('stashes')}
			<div class="row heading">
				<button class="entry" aria-expanded={open} onclick={() => toggle('stashes')}>
					<span class="caret" class:open><CaretRight /></span>
					<span class="name">Stashes</span>
					<span class="count">{saved.length}</span>
				</button>
			</div>
			{#if open}
				<ul aria-label="Stashes">
					{#each saved as { message, date }, index (`${index}:${message}`)}
						<li class="row stash" title={message}>
							<span class="name">{message}</span>
							<span class="date">{date}</span>
							<span class="actions">
								<IconButton label={`Pop ${message}`} title="Pop" disabled={busy} onclick={() => run(() => unstash(projectId, index, 'pop'))}><TrayArrowUp /></IconButton>
								<IconButton label={`Apply ${message}`} title="Apply" disabled={busy} onclick={() => run(() => unstash(projectId, index, 'apply'))}><Copy /></IconButton>
								<IconButton label={`Drop ${message}`} title="Drop" disabled={busy} onclick={() => (dropping = index)}><Trash /></IconButton>
							</span>
						</li>
					{/each}
				</ul>
			{/if}
		{/if}
	</div>
</section>

<Dialog
	bind:open={() => !!discarding, (open) => !open && (discarding = undefined)}
	title="Discard changes?"
	description={discarding
		? `Discard changes to “${discarding.label}”?${discarding.untracked ? ` This deletes ${discarding.untracked} untracked ${discarding.untracked === 1 ? 'file' : 'files'}.` : ''} This cannot be undone.`
		: ''}
>
	{#snippet actions()}
		<Button onclick={() => (discarding = undefined)}>Cancel</Button>
		<Button variant="danger" onclick={confirmDiscard}>Discard</Button>
	{/snippet}
</Dialog>

<Dialog
	bind:open={() => dropping !== undefined, (open) => !open && (dropping = undefined)}
	title="Drop stash?"
	description={dropping !== undefined ? `Drop “${saved[dropping]?.message}”? This cannot be undone.` : ''}
>
	{#snippet actions()}
		<Button onclick={() => (dropping = undefined)}>Cancel</Button>
		<Button variant="danger" onclick={confirmDrop}>Drop</Button>
	{/snippet}
</Dialog>

<style>
	section {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	header {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		padding: 0 var(--space-1) 0 var(--space-3);
	}

	.upstream {
		flex: 1;
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.sync {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		cursor: pointer;
	}

	.sync:hover:not(:disabled) {
		background: var(--hover);
		color: var(--color-fg);
	}

	.sync:focus-visible {
		outline: var(--border-width) solid var(--status-working);
	}

	.sync:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.commit {
		display: grid;
		gap: var(--space-2);
		padding: 0 var(--space-3) var(--space-2);
	}

	textarea {
		box-sizing: border-box;
		width: 100%;
		min-height: calc(3 * 1.5em);
		padding: var(--space-2);
		border: var(--border-width) solid var(--border);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font: inherit;
		line-height: 1.5;
		resize: vertical;
	}

	textarea:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: calc(-1 * var(--border-width));
	}

	.buttons {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	.failure {
		max-height: 12em;
		margin: 0;
		overflow: auto;
		padding: var(--space-2);
		border-left: 3px solid var(--git-deleted);
		background: var(--surface-1);
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		white-space: pre-wrap;
	}

	.operation {
		margin: 0 var(--space-3) var(--space-2);
		padding: var(--space-2);
		border-left: 3px solid var(--git-conflicted);
		background: var(--surface-1);
		font-size: var(--font-size-sm);
		line-height: 1.5;
	}

	.list {
		flex: 1;
		overflow: auto;
		padding-bottom: var(--space-3);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.row {
		display: flex;
		align-items: center;
		min-height: var(--target-min);
	}

	.row:hover {
		background: var(--hover);
	}

	.heading {
		position: sticky;
		top: 0;
		z-index: 1;
		border-block: var(--border-width) solid var(--border);
		background: var(--surface-2);
	}

	.heading + .heading {
		border-top: none;
	}

	.heading .name {
		font-size: var(--font-size-sm);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.entry {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		min-height: var(--target-min);
		padding: 0 var(--space-2) 0 calc(var(--space-2) + (var(--depth, -1) + 1) * var(--space-4));
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.entry:focus-visible {
		outline: var(--border-width) solid var(--status-working);
		outline-offset: calc(-1 * var(--border-width));
	}

	.file {
		padding-left: calc(var(--space-2) + (var(--depth) + 1) * var(--space-4) + 1em);
	}

	.caret {
		display: inline-flex;
		flex: none;
		width: 1em;
		color: var(--muted-fg);
		transition: transform var(--dur-fast);
	}

	.caret.open {
		transform: rotate(90deg);
	}

	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	ul .entry .name {
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
	}

	.file .name {
		color: var(--git);
	}

	.deleted .name {
		text-decoration: line-through;
	}

	.letter,
	.count {
		flex: none;
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	.letter {
		color: var(--git);
		font-weight: 700;
	}

	.count {
		color: var(--muted-fg);
	}

	.actions {
		display: flex;
		flex: none;
	}

	@media (hover: hover) {
		.row:not(:hover, :focus-within) .actions {
			opacity: 0;
		}
	}

	.stash {
		gap: var(--space-2);
		padding-left: calc(var(--space-2) + var(--space-4));
	}

	.date {
		flex: none;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.clean {
		display: grid;
		justify-items: center;
		gap: var(--space-1);
		padding: var(--space-6) var(--space-4);
		color: var(--muted-fg);
		text-align: center;
	}

	.clean p {
		margin: 0;
		color: var(--color-fg);
		font-weight: 500;
	}

	.clean small {
		font-size: var(--font-size-sm);
	}

	.mark {
		display: grid;
		place-items: center;
		width: 2.5em;
		height: 2.5em;
		margin-bottom: var(--space-2);
		border-radius: 50%;
		background: color-mix(in oklab, var(--status-done) 14%, transparent);
		color: var(--status-done);
		font-size: var(--font-size-lg);
	}
</style>
