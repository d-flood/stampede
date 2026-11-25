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
	import { failures, finished, perform, progress, type Action, type Progress } from '#lib/git-busy.svelte.ts';
	import {
		commit,
		discard,
		draft,
		letters,
		pullTrunk,
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
		type Stash,
		type Trunk
	} from '#lib/git.ts';
	import { treeRows } from '#lib/shell/tree.ts';
	import Button from '#lib/ui/Button.svelte';
	import Caret from '#lib/ui/Caret.svelte';
	import Chip from '#lib/ui/Chip.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import Eyebrow from '#lib/ui/Eyebrow.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Menu from '#lib/ui/Menu.svelte';
	import ProgressLine from '#lib/ui/ProgressLine.svelte';
	import { LoadingMark } from '#lib/ui/spinners.ts';
	import TextArea from '#lib/ui/TextArea.svelte';
	import TreeRow from '#lib/ui/TreeRow.svelte';
	import { onMount, tick, type Component, type Snippet } from 'svelte';
	import { prefersReducedMotion } from 'svelte/motion';
	import { SvelteSet } from 'svelte/reactivity';
	import { fly } from 'svelte/transition';
	import Archive from '~icons/ph/archive';
	import ArrowCounterClockwise from '~icons/ph/arrow-counter-clockwise';
	import ArrowDown from '~icons/ph/arrow-down';
	import ArrowUUpLeft from '~icons/ph/arrow-u-up-left';
	import ArrowUp from '~icons/ph/arrow-up';
	import ArrowsClockwise from '~icons/ph/arrows-clockwise';
	import Check from '~icons/ph/check';
	import CheckCircle from '~icons/ph/check-circle';
	import CloudArrowUp from '~icons/ph/cloud-arrow-up';
	import Copy from '~icons/ph/copy';
	import DotsThree from '~icons/ph/dots-three';
	import GitDiff from '~icons/ph/git-diff';
	import Minus from '~icons/ph/minus';
	import Plus from '~icons/ph/plus';
	import Sparkle from '~icons/ph/sparkle';
	import Trash from '~icons/ph/trash';
	import TrayArrowUp from '~icons/ph/tray-arrow-up';

	type Section = 'conflicted' | 'staged' | 'unstaged';
	type Target = { section: Section; changes: Change[]; label: string; change?: Change };
	type Props = {
		projectId: string;
		git: Git;
		trunk?: Trunk;
		device: Device;
		onopen(path: string): void;
		ondiff(change: Change, against: 'index' | 'staged'): void;
		onchanges(): void;
		children: Snippet;
	};

	let { projectId, git, trunk, device, onopen, ondiff, onchanges, children }: Props = $props();

	const LARGE = 200;
	const collapsed = new SvelteSet<string>();
	const opened = new SvelteSet<string>();
	let drafting = $state(false);
	let now = $state(Date.now());
	let saved = $state.raw<Stash[]>([]);
	let discarding = $state<{ label: string; changes: Change[]; untracked: number }>();
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
			.map((group) => ({ ...group, rows: treeRows(group.changes, group.section, shown) }))
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
	const running = $derived(progress[projectId]);
	const busy = $derived(!!running);
	const failure = $derived(failures[projectId]);
	const pending = $derived(new Set(running?.paths));
	const committing = $derived(running?.action === 'commit');
	const elapsed = $derived(committing ? Math.floor((now - running!.since) / 1000) : 0);
	const roll = $derived({ y: prefersReducedMotion.current ? 0 : -6, duration: 200 });
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
		if (!committing) return;
		const timer = setInterval(() => (now = Date.now()), 1000);
		return () => clearInterval(timer);
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

	function shown(key: string, changes: Change[]) {
		return changes.length > LARGE ? opened.has(key) : !collapsed.has(key);
	}

	function toggle(key: string, toggled = collapsed) {
		if (!toggled.delete(key)) toggled.add(key);
	}

	async function run(action: Action, task: (progress: Progress) => Promise<unknown>, changes: Change[] = []) {
		if (!(await perform(projectId, action, task, changes.map(({ path }) => path)))) cursor = undefined;
	}

	function paths(changes: Change[]) {
		return changes.flatMap(({ path, from }) => (from ? [path, from] : [path]));
	}

	function toggleStage({ section, changes }: Target) {
		if (section === 'staged') run('unstage', () => unstage(projectId, paths(changes)), changes);
		else run('stage', () => stage(projectId, paths(changes)), changes);
	}

	function stageAll() {
		if (git.unstaged.length) run('stage', () => stage(projectId, paths(git.unstaged)), git.unstaged);
		else if (git.staged.length) run('unstage', () => unstage(projectId, paths(git.staged)), git.staged);
	}

	function askDiscard({ section, changes, label }: Target) {
		if (section !== 'unstaged' || busy) return;
		discarding = { label, changes, untracked: changes.filter(({ kind }) => kind === 'untracked').length };
	}

	function confirmDiscard() {
		const chosen = discarding;
		discarding = undefined;
		if (chosen) run('discard', () => discard(projectId, chosen.changes.map(({ path }) => path)), chosen.changes);
	}

	function openRow({ section, change }: Target) {
		if (!change) return;
		if (section === 'conflicted') onopen(change.path);
		else ondiff(change, section === 'staged' ? 'staged' : 'index');
	}

	function commitStaged() {
		if (!committable) return;
		const id = projectId;
		run(
			'commit',
			async (progress) => {
				await commit(id, message, (output) => (progress.output = output));
				messages[id] = '';
			},
			git.staged
		);
	}

	async function draftMessage() {
		const id = projectId;
		drafting = true;
		delete failures[id];
		try {
			messages[id] = await draft(id);
		} catch (error) {
			failures[id] = { action: 'draft', text: error instanceof Error ? error.message : String(error) };
		} finally {
			drafting = false;
		}
	}

	function undoCommit() {
		const id = projectId;
		run('undo', async () => restoreMessage(id, await undo(id)));
	}

	function remote(action: 'fetch' | 'pull' | 'push' | 'publish') {
		run(action, () => sync(projectId, action));
	}

	function push() {
		if (publishable) remote('publish');
		else if (git.upstream) remote('push');
	}

	function confirmDrop() {
		const index = dropping;
		dropping = undefined;
		if (index !== undefined) run('unstash', () => unstash(projectId, index, 'drop'));
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
			s: () => run('stash', () => stash(projectId)),
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

{#snippet motion(action: Action, Icon: Component)}
	{@const done = finished[projectId] === action && action !== 'fetch'}
	<span class={['motion', action, { active: running?.action === action, done }]}>
		{#if done}<Check />{:else}<Icon />{/if}
	</span>
{/snippet}

{#snippet count(value: number)}
	{#key value}<span class="count-value" in:fly={roll}>{value}</span>{/key}
{/snippet}

<section aria-label="Git">
	<header>
		{#if git.upstream}
			<span class="upstream" title="Upstream branch">{git.upstream}</span>
			<IconButton label="Fetch" title="Fetch (f)" disabled={busy && running?.action !== 'fetch'} aria-busy={running?.action === 'fetch'} onclick={() => remote('fetch')}>
				{@render motion('fetch', ArrowsClockwise)}
			</IconButton>
			<Button variant="ghost" size="sm" aria-label="Pull" title={`Pull ${git.behind} from ${git.upstream} (p)`} disabled={busy && running?.action !== 'pull'} aria-busy={running?.action === 'pull'} onclick={() => remote('pull')}>
				{@render motion('pull', ArrowDown)}{@render count(git.behind)}
			</Button>
			<Button variant="ghost" size="sm" aria-label="Push" title={`Push ${git.ahead} to ${git.upstream} (P)`} disabled={busy ? running?.action !== 'push' : !git.ahead && finished[projectId] !== 'push'} aria-busy={running?.action === 'push'} onclick={push}>
				{@render motion('push', ArrowUp)}{@render count(git.ahead)}
			</Button>
		{:else if publishable}
			<span class="upstream">Not on origin</span>
			<Button title="Publish Branch (P)" disabled={busy && running?.action !== 'publish'} busy={running?.action === 'publish'} onclick={push}><CloudArrowUp /> Publish Branch</Button>
		{:else}
			<span class="upstream">{git.detached ? 'Detached HEAD' : 'No remote'}</span>
		{/if}
		{#if trunk && trunk.branch !== git.branch}
			<Chip
				variant="ghost"
				aria-label="Pull {trunk.branch}"
				title="Pull {trunk.behind} new from origin/{trunk.branch} into {trunk.branch} without switching"
				disabled={busy && running?.action !== 'trunk'}
				aria-busy={running?.action === 'trunk'}
				onclick={() => run('trunk', () => pullTrunk(projectId))}
			>
				{trunk.branch}{@render motion('trunk', ArrowDown)}{trunk.behind}
			</Chip>
		{/if}
		{#if git.changes.length}<IconButton label="Review all changes" title="Review all changes" onclick={onchanges}><GitDiff /></IconButton>{/if}
		<Menu
			items={[
				{ label: 'Stash All Changes', icon: Archive, hint: 's', disabled: busy || !git.changes.length, onselect: () => run('stash', () => stash(projectId)) },
				{ label: 'Undo Last Commit', icon: ArrowUUpLeft, hint: 'z', disabled: busy, onselect: undoCommit }
			]}
		>
			{#snippet trigger(props)}
				<IconButton {...props} label="More Git actions"><DotsThree /></IconButton>
			{/snippet}
		</Menu>
		<ProgressLine active={busy && !committing} />
	</header>
	{#if failure && failure.action !== 'commit'}<pre class="failure" role="alert">{failure.text}</pre>{/if}
	<div class="scroller">
		{#if git.changes.length || git.operation || message}
			<div class="commit">
				<TextArea
					bind:ref={box}
					bind:value={() => message, (value) => (messages[projectId] = value)}
					aria-label="Commit message"
					placeholder="Commit message"
					rows={1}
					spellcheck="true"
					readonly={committing}
					onkeydown={commitKey}
				/>
				<div class="buttons">
					{#if elapsed >= 2}<span class="elapsed" title="Time spent committing">{Math.floor(elapsed / 60)}:{String(elapsed % 60).padStart(2, '0')}</span>{/if}
					<Button variant="ghost" title={git.staged.length ? `Draft a message with ${commitRig() ?? 'the commit Rig'}` : 'Stage changes to draft a message'} disabled={drafting || committing || !git.staged.length} onclick={draftMessage}>
						<Sparkle />{drafting ? 'Drafting…' : 'Draft'}
					</Button>
					<Button variant="primary" title="Commit (Ctrl+Enter)" disabled={!committable && !committing} busy={committing} onclick={commitStaged}>
						{#if committing}<LoadingMark />Committing{:else}<Check />{git.operation === 'merge' ? 'Commit Merge' : 'Commit'}{/if}
					</Button>
				</div>
			</div>
		{/if}
		{#if committing && running?.output}
			<pre class="log">{running.output.trimEnd()}</pre>
		{:else if failure?.action === 'commit'}
			<pre class="log failure" role="alert">{failure.text}</pre>
		{/if}
		{#if operation}<p class="operation" role="status">{operation}</p>{/if}
		<!-- svelte-ignore a11y_no_static_element_interactions (Keys act on the focused row.) -->
		<div bind:this={list} onkeydown={key} aria-busy={busy}>
			{#each groups as { section, label, changes, rows } (section)}
				{@const open = !collapsed.has(section)}
				<div class="row heading">
					<button data-key={section} class="entry" aria-expanded={open} onclick={() => toggle(section)}>
						<Caret />
						<span class="name"><Eyebrow>{label}</Eyebrow></span>
						<span class="count">{changes.length}</span>
					</button>
					{@render controls(targets.get(section))}
				</div>
				{#if open}
					<ul aria-label={label}>
						{#each rows as row (row.key)}
							{@const waiting = row.kind === 'file' ? pending.has(row.change.path) : row.changes.every(({ path }) => pending.has(path))}
							<li class="row" class:pending={waiting} aria-busy={waiting || undefined}>
								{#if row.kind === 'folder'}
									<TreeRow data-key={row.key} name={row.folder.name} depth={row.depth} open={row.open} muted title={row.folder.path} onclick={() => toggle(row.key, row.changes.length > LARGE ? opened : collapsed)} />
								{:else}
									{@const { change } = row}
									<TreeRow
										data-key={row.key}
										name={split(change.path).name}
										depth={row.depth}
										git={change.kind}
										letter={waiting ? undefined : letters[change.kind]}
										title={change.from ? `${change.from} → ${change.path}` : change.path}
										onclick={() => openRow(targets.get(row.key)!)}
									>
										{#if waiting}<span class="waiting"><LoadingMark /></span>{/if}
									</TreeRow>
								{/if}
								{@render controls(targets.get(row.key))}
							</li>
						{/each}
					</ul>
				{/if}
			{:else}
				<p class="clean"><CheckCircle />No uncommitted changes</p>
			{/each}
			{#if saved.length}
				{@const open = !collapsed.has('stashes')}
				<div class="row heading">
					<button class="entry" aria-expanded={open} onclick={() => toggle('stashes')}>
						<Caret />
						<span class="name"><Eyebrow>Stashes</Eyebrow></span>
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
									<IconButton label={`Pop ${message}`} title="Pop" disabled={busy} onclick={() => run('unstash', () => unstash(projectId, index, 'pop'))}><TrayArrowUp /></IconButton>
									<IconButton label={`Apply ${message}`} title="Apply" disabled={busy} onclick={() => run('unstash', () => unstash(projectId, index, 'apply'))}><Copy /></IconButton>
									<IconButton label={`Drop ${message}`} title="Drop" disabled={busy} onclick={() => (dropping = index)}><Trash /></IconButton>
								</span>
							</li>
						{/each}
					</ul>
				{/if}
			{/if}
		</div>
		{@render children()}
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
		position: relative;
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		padding: 0 var(--space-1) 0 var(--space-2);
	}

	.upstream {
		flex: 1;
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.motion {
		display: inline-flex;
		transition: color var(--dur-fast);
	}

	.motion.active {
		color: var(--color-fg);
	}

	.fetch.active {
		animation: spin 0.8s linear var(--pulse);
	}

	.push.active {
		animation: lift 0.9s var(--ease-out) var(--pulse);
	}

	:is(.pull, .trunk).active {
		animation: drop 0.9s var(--ease-out) var(--pulse);
	}

	.motion.done {
		color: var(--status-done);
		animation: pop var(--dur-med) var(--ease-out);
	}

	.count-value {
		display: inline-block;
		font-variant-numeric: tabular-nums;
	}

	@keyframes spin {
		to {
			rotate: 1turn;
		}
	}

	@keyframes lift {
		from {
			opacity: 0;
			translate: 0 40%;
		}
		35%,
		70% {
			opacity: 1;
		}
		to {
			opacity: 0;
			translate: 0 -40%;
		}
	}

	@keyframes drop {
		from {
			opacity: 0;
			translate: 0 -40%;
		}
		35%,
		70% {
			opacity: 1;
		}
		to {
			opacity: 0;
			translate: 0 40%;
		}
	}

	@keyframes pop {
		from {
			scale: 0.5;
		}
		60% {
			scale: 1.2;
		}
	}





	.scroller {
		flex: 1;
		min-height: 0;
		overflow: auto;
	}

	.commit {
		display: grid;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-2) var(--space-2);
	}

	.commit :global(textarea) {
		min-height: calc(1lh + 2 * var(--space-1));
		max-height: 12lh;
		resize: none;
		field-sizing: content;
	}

	.buttons {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	.buttons > :global(button:last-child) {
		flex: 1;
	}

	.elapsed {
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		font-variant-numeric: tabular-nums;
	}

	.failure,
	.log {
		max-height: 12em;
		margin: 0 var(--space-3) var(--space-2);
		overflow: auto;
		padding: var(--space-2);
		border-left: 3px solid var(--git-deleted);
		background: var(--surface-1);
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		white-space: pre-wrap;
	}

	/* Column-reverse keeps the newest output in view as it streams in. */
	.log {
		display: flex;
		flex-direction: column-reverse;
	}

	.log:not(.failure) {
		max-height: 8em;
		border-left-color: var(--accent);
		color: var(--muted-fg);
	}

	.operation {
		margin: 0 var(--space-3) var(--space-2);
		padding: var(--space-2);
		border-left: 3px solid var(--git-conflicted);
		background: var(--surface-1);
		font-size: var(--font-size-sm);
		line-height: 1.5;
	}

	ul {
		margin: 0;
		padding: 0 var(--space-2);
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

	li.row {
		border-radius: var(--radius-md);
		transition: opacity var(--dur-med);
	}

	li.pending {
		opacity: 0.55;
	}

	.waiting {
		display: inline-flex;
		flex: none;
		color: var(--muted-fg);
	}

	.heading {
		position: sticky;
		top: 0;
		z-index: 1;
		border-block: var(--border-width) solid var(--border);
		background: var(--chrome);
	}

	.heading + .heading {
		border-top: none;
	}

	.entry {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.entry:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-inset);
	}



	.name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.count {
		flex: none;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.actions {
		display: flex;
		flex: none;
	}

	/* Overlays hover actions so badges and counts stay flush right. */
	@media (hover: hover) {
		li.row {
			position: relative;
		}

		.actions {
			position: absolute;
			right: 0;
			border-radius: inherit;
			background: var(--chrome);
		}

		.row:hover .actions {
			background: var(--hover);
		}

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
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--target-min);
		margin: 0;
		padding: var(--space-1) var(--space-3);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.clean :global(svg) {
		flex: none;
		color: var(--status-done);
		font-size: var(--font-size-md);
	}
</style>
