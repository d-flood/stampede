<script lang="ts">
	import { comparison } from '#lib/compare.svelte.ts';
	import type { Device } from '#lib/device.svelte.ts';
	import { branches, type Reviewing, type Selection } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { letters, refs, split, type Change, type Git, type Refs } from '#lib/git.ts';
	import { deleteDraft, discardReview, openWorktree, prs, refreshReview, setViewed, viewedState, type Availability, type DraftComment, type PrReview } from '#lib/pr-reviews.ts';
	import BranchPicker from '#lib/shell/BranchPicker.svelte';
	import PrPicker from '#lib/shell/PrPicker.svelte';
	import SendReview from '#lib/shell/SendReview.svelte';
	import { treeRows } from '#lib/shell/tree.ts';
	import Button from '#lib/ui/Button.svelte';
	import Dialog from '#lib/ui/Dialog.svelte';
	import IconButton from '#lib/ui/IconButton.svelte';
	import Tooltip from '#lib/ui/Tooltip.svelte';
	import { mergeProps } from 'bits-ui';
	import { onMount, untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import ArrowSquareOut from '~icons/ph/arrow-square-out';
	import ArrowsClockwise from '~icons/ph/arrows-clockwise';
	import CaretRight from '~icons/ph/caret-right';
	import CaretUpDown from '~icons/ph/caret-up-down';
	import FolderOpen from '~icons/ph/folder-open';
	import GitPullRequest from '~icons/ph/git-pull-request';
	import Trash from '~icons/ph/trash';
	import X from '~icons/ph/x';

	type Side = 'target' | 'source';
	type Props = {
		projectId: string;
		git: Git;
		device: Device;
		selection?: Selection | Reviewing;
		reviews?: PrReview[];
		onselect(selection?: Selection | Reviewing): void;
		onopen(change: Change, against: Selection | Reviewing, draft?: string, replace?: boolean): void;
		onfocus(projectId: string): void;
	};

	let { projectId, git, device, selection, reviews, onselect, onopen, onfocus }: Props = $props();

	const collapsed = new SvelteSet<string>();
	let listed = $state.raw<Refs>();
	let availability = $state.raw<Availability>();
	let picking = $state<Side>();
	let choosing = $state(false);
	let sending = $state(false);
	let acting = $state(false);
	let actionError = $state<string>();
	const reviewing = $derived(selection && 'pr' in selection ? selection.pr : undefined);
	const plain = $derived(selection && 'target' in selection ? selection : undefined);
	const review = $derived(reviewing === undefined ? undefined : reviews?.find((review) => review.number === reviewing));
	const target = $derived(plain?.target ?? listed?.default);
	const source = $derived(plain?.source ?? (git.detached ? undefined : git.branch));
	const against = $derived<Selection | Reviewing | undefined>(reviewing !== undefined ? { pr: reviewing } : target && source ? { target, source } : undefined);
	const compared = $derived(against ? comparison(projectId, branches(against)) : undefined);
	const files = $derived(compared?.result?.files);
	const rows = $derived(files ? treeRows(files, 'compare', (key) => !collapsed.has(key)) : []);
	const viewedCount = $derived(review && files ? files.filter((change) => viewedState(review, change) === 'viewed').length : 0);
	const placed = $derived(review?.drafts.filter((draft) => !draft.displaced) ?? []);
	const displaced = $derived(review?.drafts.filter((draft) => draft.displaced) ?? []);
	const reason = $derived(availability && !availability.available && !availability.hidden ? availability.reason : undefined);
	const failure = $derived.by(() => {
		if (reviewing !== undefined && reviews && !review) return `No PR review of #${reviewing}`;
		const error = compared?.error;
		if (!error) return;
		return error.status === 409 ? `${target} and ${source} share no history` : error.message;
	});

	onMount(() => {
		refs(projectId).then((refs) => (listed = refs), () => {});
		prs(projectId).then((found) => (availability = found), () => {});
		return device.listen({
			message(message) {
				if (message.type === 'hello' || (message.type === 'git.changed' && message.projectId === projectId)) compared?.load();
				if (message.type === 'pr-reviews.changed' && message.projectId === projectId && reviewing !== undefined) compared?.load();
			}
		});
	});

	$effect(() => {
		const current = compared;
		untrack(() => current?.load());
	});

	function pick(side: Side, name: string) {
		picking = undefined;
		const chosen = { target: target ?? name, source: source ?? name };
		onselect({ ...chosen, [side]: name });
	}

	function toggle(key: string) {
		if (!collapsed.delete(key)) collapsed.add(key);
	}

	function opened(review: PrReview) {
		choosing = false;
		onselect({ pr: review.number });
	}

	async function act(action: () => Promise<unknown>) {
		if (acting) return;
		acting = true;
		actionError = undefined;
		try {
			await action();
		} catch (cause) {
			actionError = cause instanceof Error ? cause.message : String(cause);
		} finally {
			acting = false;
		}
	}

	function refresh(pr: number) {
		act(async () => {
			await refreshReview(projectId, pr);
			await compared?.load();
		});
	}

	function view(pr: number, path: string, viewed: boolean) {
		act(() => setViewed(projectId, pr, path, viewed));
	}

	function branchOff(pr: number) {
		act(async () => onfocus(await openWorktree(projectId, pr)));
	}

	function showDraft(draft: DraftComment, replace = false) {
		if (reviewing === undefined) return;
		const change = files?.find((change) => change.path === draft.path) ?? { path: draft.path, kind: 'modified' as const };
		onopen(change, { pr: reviewing }, draft.id, replace);
	}

	function removeDraft(pr: number, id: string) {
		act(() => deleteDraft(projectId, pr, id));
	}

	function discard(pr: number) {
		act(async () => {
			await discardReview(projectId, pr);
			onselect();
		});
	}
</script>

{#snippet picker(side: Side, label: string, name: string | undefined)}
	<button class="ref" aria-haspopup="dialog" aria-expanded={picking === side} title={name} onclick={() => (picking = side)}>
		<span class="role">{label}</span>
		<span class="branch" class:unset={!name}>{name ?? 'Choose a branch'}</span>
		<CaretUpDown aria-hidden="true" />
	</button>
{/snippet}

{#snippet draftList(label: string, drafts: DraftComment[])}
	<div class="heading">
		<span class="name">{label}</span>
		<span class="count">{drafts.length}</span>
	</div>
	<ul class="drafts" aria-label={label}>
		{#each drafts as draft (draft.id)}
			{@const lines = draft.startLine ? `${draft.startLine}–${draft.line}` : draft.line}
			<li class="row">
				<button class="entry draft" title="{draft.path}:{lines}" onclick={() => showDraft(draft)}>
					<span class="where">
						<span class="name">{draft.path}:{lines}</span>
						{#if draft.side === 'target'}<span class="side">Target</span>{/if}
					</span>
					<span class="body">{draft.body.trim().split('\n')[0]}</span>
				</button>
			</li>
		{/each}
	</ul>
{/snippet}

{#snippet displacedList(pr: number, drafts: DraftComment[])}
	<div class="heading">
		<span class="name">Displaced</span>
		<span class="count">{drafts.length}</span>
	</div>
	<ul class="drafts" aria-label="Displaced">
		{#each drafts as draft (draft.id)}
			{@const lines = draft.startLine ? `${draft.startLine}–${draft.line}` : draft.line}
			<li class="displaced">
				<span class="where">
					<span class="name" title="{draft.path}, was line {lines}">{draft.path}:{lines}</span>
					{#if draft.side === 'target'}<span class="side">Target</span>{/if}
				</span>
				<span class="body">{draft.body.trim().split('\n')[0]}</span>
				<span class="draft-actions">
					<Button title="Choose a new line for this draft" disabled={acting} onclick={() => showDraft(draft, true)}>Re-place</Button>
					<Button variant="danger" disabled={acting} onclick={() => removeDraft(pr, draft.id)}>Delete</Button>
				</span>
			</li>
		{/each}
	</ul>
{/snippet}

{#snippet prButton(props: Record<string, unknown>)}
	<button
		{...mergeProps(props, { onclick: () => !reason && (choosing = true) })}
		class="ref"
		aria-haspopup="dialog"
		aria-expanded={choosing}
		aria-disabled={!!reason}
	>
		<GitPullRequest aria-hidden="true" />
		<span class="label">Pull request…</span>
	</button>
{/snippet}

<section aria-label="Compare">
	{#if reviewing !== undefined}
		<div class="review">
			<h3 class="title" title={review?.title}><span class="number">#{reviewing}</span> {review?.title ?? ''}</h3>
			<div class="actions">
				{#if review}
					<a class="github" href={review.url} target="_blank" rel="noopener noreferrer"><ArrowSquareOut aria-hidden="true" />Open on GitHub</a>
				{/if}
				<IconButton label="Open as worktree" title="Check out this pull request in a worktree beside the Project" disabled={acting || !review} onclick={() => branchOff(reviewing)}>
					<FolderOpen />
				</IconButton>
				<IconButton label="Refresh" title="Fetch the pull request again" disabled={acting || !review} onclick={() => refresh(reviewing)}>
					<ArrowsClockwise />
				</IconButton>
				<IconButton label="Discard PR review" title="Discard this PR review" disabled={acting || !review} onclick={() => discard(reviewing)}>
					<Trash />
				</IconButton>
				<IconButton label="Close" title="Back to the plain Comparison" onclick={() => onselect()}><X /></IconButton>
			</div>
		</div>
		{#if review && review.state !== 'open'}
			<div class="banner" role="status">
				<span>#{review.number} was {review.state} on GitHub.</span>
				<Button variant="danger" disabled={acting} onclick={() => discard(review.number)}>Discard</Button>
			</div>
		{/if}
		{#if actionError}<p class="note failure" role="alert">{actionError}</p>{/if}
		{#if placed.length}{@render draftList('Draft comments', placed)}{/if}
		{#if displaced.length}{@render displacedList(reviewing, displaced)}{/if}
		{#if review?.state === 'open'}
			<div class="send">
				<Button variant="primary" disabled={!!displaced.length} onclick={() => (sending = true)}>
					Send review… <span class="badge">{review.drafts.length}</span>
				</Button>
				{#if displaced.length}
					<p class="reason">Re-place or delete {displaced.length === 1 ? 'the Displaced draft' : `the ${displaced.length} Displaced drafts`} to send.</p>
				{/if}
			</div>
			{#key review.number}<SendReview {projectId} {review} bind:open={sending} />{/key}
		{/if}
	{:else}
		<div class="refs">
			{@render picker('target', 'Target', target)}
			{@render picker('source', 'Source', source)}
			{#if availability && !(!availability.available && availability.hidden)}
				{#if reason && form() === 'desktop'}
					<Tooltip text={reason} delayDuration={0}>
						{#snippet children(trigger)}
							{@render prButton(trigger)}
						{/snippet}
					</Tooltip>
				{:else}
					{@render prButton({})}
				{/if}
				{#if reason && form() !== 'desktop'}<p class="reason">{reason}</p>{/if}
			{/if}
		</div>
		{#if reviews?.length}
			<div class="heading">
				<span class="name">PR reviews in progress</span>
			</div>
			<ul class="reviews" aria-label="PR reviews in progress">
				{#each reviews as listedReview (listedReview.number)}
					<li class="row">
						<button class="entry" title={listedReview.title} onclick={() => onselect({ pr: listedReview.number })}>
							<span class="number">#{listedReview.number}</span>
							<span class="name">{listedReview.title}</span>
							{#if listedReview.state !== 'open'}<span class="state">{listedReview.state}</span>{/if}
						</button>
					</li>
				{/each}
			</ul>
		{/if}
	{/if}
	{#if failure}
		<p class="note failure" role="alert">{failure}</p>
	{:else if files?.length === 0}
		<p class="note" role="status">No changes</p>
	{:else if files}
		<div class="heading">
			<span class="name">Changed files</span>
			{#if review}
				<span class="count">{viewedCount}/{files.length} viewed</span>
			{:else}
				<span class="count">{files.length} {files.length === 1 ? 'file' : 'files'}</span>
			{/if}
		</div>
		<ul class="list" aria-label="Compared files">
			{#each rows as row (row.key)}
				<li class="row">
					{#if row.kind === 'folder'}
						<button class="entry" style:--depth={row.depth} aria-expanded={row.open} title={row.folder.path} onclick={() => toggle(row.key)}>
							<span class="caret" class:open={row.open}><CaretRight /></span>
							<span class="name">{row.folder.name}</span>
						</button>
					{:else}
						{@const { change } = row}
						{@const mark = review && viewedState(review, change)}
						<button
							class="entry file"
							class:deleted={change.kind === 'deleted'}
							style:--depth={row.depth}
							style:--git="var(--git-{change.kind})"
							title={change.from ? `${change.from} → ${change.path}` : change.path}
							onclick={() => against && onopen(change, against)}
						>
							<span class="name" class:viewed={mark === 'viewed'}>{split(change.path).name}</span>
							{#if mark === 'changed'}<span class="since" role="img" aria-label="Changed since viewed" title="Changed since viewed"></span>{/if}
							<span class="letter" aria-hidden="true">{letters[change.kind]}</span>
						</button>
						{#if review}
							<label class="viewed-box" title="Viewed">
								<input
									type="checkbox"
									aria-label="Viewed"
									checked={mark === 'viewed'}
									disabled={acting}
									onchange={(event) => view(review.number, change.path, event.currentTarget.checked)}
								/>
							</label>
						{/if}
					{/if}
				</li>
			{/each}
		</ul>
	{:else if reviewing === undefined && (!target || !source)}
		<p class="note">Choose a Target and a Source to compare.</p>
	{/if}
</section>

<Dialog
	bind:open={() => !!picking, (open) => !open && (picking = undefined)}
	full={form() === 'phone'}
	title={picking === 'target' ? 'Choose the Target' : 'Choose the Source'}
	description={picking === 'target' ? 'The branch the Source split off.' : 'The branch whose changes to show.'}
>
	{#if picking}
		{@const side = picking}
		<BranchPicker {projectId} current={side === 'target' ? target : source} origin={!!git.origin} onpick={(name) => pick(side, name)} />
	{/if}
</Dialog>

<Dialog bind:open={choosing} full={form() === 'phone'} title="Open a pull request" description="Fetches it and shows what it changes.">
	{#if choosing}<PrPicker {projectId} onopen={opened} />{/if}
</Dialog>

<style>
	section {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-height: 0;
	}

	.refs {
		display: grid;
		gap: var(--space-1);
		padding: 0 var(--space-2) var(--space-2);
	}

	.ref {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		min-height: var(--target-min);
		padding: 0 var(--space-2);
		border: none;
		border-radius: calc(var(--target-min) / 2);
		background: none;
		color: var(--muted-fg);
		font: inherit;
		font-size: var(--font-size-sm);
		text-align: left;
		cursor: pointer;
	}

	.ref:hover {
		background: var(--hover);
		color: var(--color-fg);
	}

	.ref:focus-visible {
		outline: var(--border-width) solid var(--accent);
	}

	.ref :global(svg) {
		flex: none;
	}

	.role {
		flex: none;
		width: 4.5em;
	}

	.branch {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		color: var(--color-fg);
		font-family: var(--font-mono);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.branch.unset {
		color: var(--muted-fg);
		font-family: inherit;
	}

	.note {
		margin: 0 var(--space-3);
		padding: var(--space-4) 0;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-align: center;
		overflow-wrap: anywhere;
	}

	.failure {
		padding: var(--space-2);
		border-left: 3px solid var(--git-deleted);
		background: var(--surface-1);
		color: var(--color-fg);
		text-align: left;
	}

	.heading {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		padding: 0 var(--space-2) 0 var(--space-3);
		border-block: var(--border-width) solid var(--border);
		background: var(--surface-2);
	}

	.heading .name {
		font-size: var(--font-size-sm);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.list {
		flex: 1;
		margin: 0;
		overflow: auto;
		padding: 0 0 var(--space-3);
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

	.entry {
		display: flex;
		flex: 1;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		min-height: var(--target-min);
		padding: 0 var(--space-2) 0 calc(var(--space-2) + var(--depth) * var(--space-4));
		border: none;
		background: none;
		color: var(--color-fg);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.entry:focus-visible {
		outline: var(--border-width) solid var(--accent);
		outline-offset: calc(-1 * var(--border-width));
	}

	.file {
		padding-left: calc(var(--space-2) + var(--depth) * var(--space-4) + 1em + var(--space-1));
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

	.entry .name {
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
	}

	.file .name {
		color: var(--git);
	}

	.deleted .name {
		text-decoration: line-through;
	}

	.file .name.viewed {
		opacity: 0.55;
	}

	.since {
		flex: none;
		width: 0.5em;
		height: 0.5em;
		border-radius: 50%;
		background: var(--status-waiting);
	}

	.viewed-box {
		display: grid;
		flex: none;
		place-items: center;
		width: var(--target-min);
		height: var(--target-min);
		cursor: pointer;
	}

	.viewed-box input {
		margin: 0;
		accent-color: var(--accent);
		cursor: inherit;
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

	.ref[aria-disabled='true'] {
		color: var(--muted-fg);
		cursor: default;
		opacity: 0.6;
	}

	.ref[aria-disabled='true']:hover {
		background: none;
	}

	.label {
		flex: 1;
		color: var(--color-fg);
	}

	.reason {
		margin: 0 var(--space-2);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		overflow-wrap: anywhere;
	}

	.review {
		display: grid;
		gap: var(--space-1);
		padding: 0 var(--space-2) var(--space-2) var(--space-3);
	}

	.title {
		margin: 0;
		overflow: hidden;
		font-size: var(--font-size-md);
		font-weight: 600;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.number {
		flex: none;
		color: var(--muted-fg);
		font-family: var(--font-mono);
		font-weight: 400;
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}

	.github {
		display: inline-flex;
		flex: 1;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--target-min);
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-decoration: none;
	}

	.github:hover {
		color: var(--color-fg);
		text-decoration: underline;
	}

	.banner {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		margin: 0 var(--space-2) var(--space-2);
		padding: var(--space-2);
		border-left: 3px solid var(--status-waiting);
		background: var(--surface-1);
		font-size: var(--font-size-sm);
	}

	.reviews {
		margin: 0;
		padding: 0 0 var(--space-2);
		list-style: none;
	}

	.reviews .entry {
		--depth: 0;
		gap: var(--space-2);
	}

	.reviews .name {
		font-family: inherit;
		font-size: inherit;
	}

	.drafts {
		flex: none;
		max-height: 40%;
		margin: 0;
		overflow: auto;
		padding: 0 0 var(--space-2);
		list-style: none;
	}

	.draft {
		--depth: 0;
		flex-direction: column;
		align-items: stretch;
		gap: 0;
		padding-block: var(--space-1);
	}

	.where {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		min-width: 0;
	}

	.side {
		flex: none;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
	}

	.body {
		overflow: hidden;
		color: var(--muted-fg);
		font-size: var(--font-size-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.displaced {
		display: grid;
		gap: var(--space-1);
		padding: var(--space-1) var(--space-2) var(--space-1) var(--space-3);
	}

	.displaced .name {
		font-family: var(--font-mono);
		font-size: var(--font-size-code);
	}

	.draft-actions {
		display: flex;
		gap: var(--space-1);
	}

	.send {
		display: grid;
		gap: var(--space-1);
		padding: var(--space-2);
	}

	.badge {
		min-width: 1.5em;
		padding: 0 var(--space-1);
		border-radius: 999px;
		background: color-mix(in oklab, currentColor 18%, transparent);
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
	}

	.state {
		flex: none;
		color: var(--status-waiting);
		font-size: var(--font-size-sm);
	}
</style>
