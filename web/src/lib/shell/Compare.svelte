<script lang="ts">
	import { comparison } from '#lib/compare.svelte.ts';
	import type { Device } from '#lib/device.svelte.ts';
	import { branches, type Reviewing, type Selection } from '#lib/files.svelte.ts';
	import { form } from '#lib/form.ts';
	import { letters, refs, split, type Change, type Git, type Refs } from '#lib/git.ts';
	import { discardReview, prs, refreshReview, type Availability, type PrReview } from '#lib/pr-reviews.ts';
	import BranchPicker from '#lib/shell/BranchPicker.svelte';
	import PrPicker from '#lib/shell/PrPicker.svelte';
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
		onopen(change: Change, against: Selection | Reviewing): void;
	};

	let { projectId, git, device, selection, reviews, onselect, onopen }: Props = $props();

	const collapsed = new SvelteSet<string>();
	let listed = $state.raw<Refs>();
	let availability = $state.raw<Availability>();
	let picking = $state<Side>();
	let choosing = $state(false);
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
			<span class="count">{files.length} {files.length === 1 ? 'file' : 'files'}</span>
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
						<button
							class="entry file"
							class:deleted={change.kind === 'deleted'}
							style:--depth={row.depth}
							style:--git="var(--git-{change.kind})"
							title={change.from ? `${change.from} → ${change.path}` : change.path}
							onclick={() => against && onopen(change, against)}
						>
							<span class="name">{split(change.path).name}</span>
							<span class="letter" aria-hidden="true">{letters[change.kind]}</span>
						</button>
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
		outline: var(--border-width) solid var(--status-working);
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
		outline: var(--border-width) solid var(--status-working);
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

	.state {
		flex: none;
		color: var(--status-waiting);
		font-size: var(--font-size-sm);
	}
</style>
